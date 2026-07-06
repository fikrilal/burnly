#![allow(
    dead_code,
    reason = "Antigravity runtime client is introduced before collection mapping in chunk 4"
)]

use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::{json, Value};
use thiserror::Error;

use super::RuntimeEndpoint;

const QUOTA_PATH: &str = "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary";
const TRAJECTORIES_PATH: &str =
    "/exa.language_server_pb.LanguageServerService/GetAllCascadeTrajectories";
const GENERATOR_METADATA_PATH: &str =
    "/exa.language_server_pb.LanguageServerService/GetCascadeTrajectoryGeneratorMetadata";
const STREAM_PATH: &str = "/exa.language_server_pb.LanguageServerService/StreamAgentStateUpdates";
const TIMEOUT: Duration = Duration::from_secs(3);
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_millis(750);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_CONNECT_FRAME_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeClient;

impl RuntimeClient {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn retrieve_user_quota_summary(
        &self,
        endpoint: &RuntimeEndpoint,
    ) -> Result<Value, RuntimeClientError> {
        let response = post_json(endpoint, QUOTA_PATH, b"{}", ContentType::Json)?;
        serde_json::from_slice(&response).map_err(|_| RuntimeClientError::InvalidJson)
    }

    pub(crate) fn get_all_cascade_trajectories(
        &self,
        endpoint: &RuntimeEndpoint,
    ) -> Result<Value, RuntimeClientError> {
        let response = post_json(endpoint, TRAJECTORIES_PATH, b"{}", ContentType::Json)?;
        serde_json::from_slice(&response).map_err(|_| RuntimeClientError::InvalidJson)
    }

    pub(crate) fn get_cascade_trajectory_generator_metadata(
        &self,
        endpoint: &RuntimeEndpoint,
        cascade_id: &str,
    ) -> Result<Value, RuntimeClientError> {
        if cascade_id.trim().is_empty() {
            return Err(RuntimeClientError::InvalidCascadeId);
        }
        let request = json!({ "cascadeId": cascade_id });
        let body = serde_json::to_vec(&request).map_err(|_| RuntimeClientError::InvalidJson)?;
        let response = post_json(endpoint, GENERATOR_METADATA_PATH, &body, ContentType::Json)?;
        serde_json::from_slice(&response).map_err(|_| RuntimeClientError::InvalidJson)
    }

    pub(crate) fn probe_identity(
        &self,
        endpoint: &RuntimeEndpoint,
    ) -> Result<(), RuntimeClientError> {
        if let Ok(value) = self.retrieve_user_quota_summary(endpoint) {
            if quota_response_is_valid(&value) {
                return Ok(());
            }
        }
        let value = self.get_all_cascade_trajectories(endpoint)?;
        if trajectories_response_is_valid(&value) {
            Ok(())
        } else {
            Err(RuntimeClientError::IdentityProbeFailed)
        }
    }

    pub(crate) fn stream_agent_state_updates(
        &self,
        endpoint: &RuntimeEndpoint,
        conversation_id: &str,
    ) -> Result<Vec<Value>, RuntimeClientError> {
        if conversation_id.trim().is_empty() {
            return Err(RuntimeClientError::InvalidConversationId);
        }
        let request = json!({
            "conversationId": conversation_id,
            "subscriberId": "burnly-readonly-inspection",
            "initialStepsPageBounds": { "startIndex": -500 },
            "initialGeneratorMetadatasPageBounds": { "startIndex": -500 },
            "initialExecutorMetadatasPageBounds": { "startIndex": -500 },
            "trajectoryVerbosity": 3
        });
        let body = encode_connect_frame(
            &serde_json::to_vec(&request).map_err(|_| RuntimeClientError::InvalidJson)?,
        )?;
        let response = post_connect_stream(endpoint, STREAM_PATH, &body)?;
        decode_connect_frames(&response)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentType {
    Json,
    ConnectJson,
}

impl ContentType {
    const fn as_header(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::ConnectJson => "application/connect+json",
        }
    }
}

fn post_json(
    endpoint: &RuntimeEndpoint,
    path: &str,
    body: &[u8],
    content_type: ContentType,
) -> Result<Vec<u8>, RuntimeClientError> {
    match post_json_http(endpoint, path, body, content_type) {
        Ok(response) => Ok(response),
        Err(http_error)
            if matches!(
                http_error,
                RuntimeClientError::ConnectionFailed | RuntimeClientError::MalformedHttp
            ) =>
        {
            match post_json_https(endpoint, path, body, content_type) {
                Ok(response) => Ok(response),
                Err(_) => Err(http_error),
            }
        }
        Err(http_error) => Err(http_error),
    }
}

fn post_json_http(
    endpoint: &RuntimeEndpoint,
    path: &str,
    body: &[u8],
    content_type: ContentType,
) -> Result<Vec<u8>, RuntimeClientError> {
    let mut stream = connect(endpoint)?;
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        host_header(endpoint.host, endpoint.port),
        content_type.as_header(),
        body.len(),
    );
    if content_type == ContentType::ConnectJson {
        request.push_str("Connect-Protocol-Version: 1\r\n");
    }
    if let Some(token) = endpoint
        .csrf_token
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        request.push_str("x-codeium-csrf-token: ");
        request.push_str(token);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    write_request_body(&mut stream, &request, body)?;
    read_http_response(stream)
}

fn post_json_https(
    endpoint: &RuntimeEndpoint,
    path: &str,
    body: &[u8],
    content_type: ContentType,
) -> Result<Vec<u8>, RuntimeClientError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .tls_danger_accept_invalid_certs(true)
        .build()
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    let url = format!(
        "https://{}{}",
        host_header(endpoint.host, endpoint.port),
        path
    );
    let mut request = client
        .post(url)
        .header("Content-Type", content_type.as_header())
        .body(body.to_vec());
    if content_type == ContentType::ConnectJson {
        request = request.header("Connect-Protocol-Version", "1");
    }
    if let Some(token) = endpoint
        .csrf_token
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        request = request.header("x-codeium-csrf-token", token);
    }
    let response = request
        .send()
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    if !response.status().is_success() {
        return Err(RuntimeClientError::HttpStatus);
    }
    let bytes = response
        .bytes()
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(RuntimeClientError::ResponseTooLarge);
    }
    Ok(bytes.to_vec())
}

fn post_connect_stream(
    endpoint: &RuntimeEndpoint,
    path: &str,
    body: &[u8],
) -> Result<Vec<u8>, RuntimeClientError> {
    let mut stream = connect(endpoint)?;
    let request = connect_request(endpoint, path, body.len());
    write_request_body(&mut stream, &request, body)?;
    read_connect_stream_response(stream)
}

fn connect(endpoint: &RuntimeEndpoint) -> Result<TcpStream, RuntimeClientError> {
    let stream =
        TcpStream::connect_timeout(&SocketAddr::new(endpoint.host, endpoint.port), TIMEOUT)
            .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    Ok(stream)
}

fn write_request_body(
    stream: &mut TcpStream,
    request: &str,
    body: &[u8],
) -> Result<(), RuntimeClientError> {
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|_| RuntimeClientError::ConnectionFailed)
}

fn connect_request(endpoint: &RuntimeEndpoint, path: &str, content_length: usize) -> String {
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nConnect-Protocol-Version: 1\r\n",
        host_header(endpoint.host, endpoint.port),
        ContentType::ConnectJson.as_header(),
        content_length,
    );
    if let Some(token) = endpoint
        .csrf_token
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        request.push_str("x-codeium-csrf-token: ");
        request.push_str(token);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    request
}

fn host_header(host: IpAddr, port: u16) -> String {
    match host {
        IpAddr::V4(value) => format!("{value}:{port}"),
        IpAddr::V6(value) => format!("[{value}]:{port}"),
    }
}

fn read_http_response(stream: TcpStream) -> Result<Vec<u8>, RuntimeClientError> {
    let mut raw = Vec::new();
    stream
        .take((MAX_RESPONSE_BYTES + 8192) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    if raw.len() > MAX_RESPONSE_BYTES {
        return Err(RuntimeClientError::ResponseTooLarge);
    }

    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(RuntimeClientError::MalformedHttp)?;
    let headers =
        std::str::from_utf8(&raw[..header_end]).map_err(|_| RuntimeClientError::MalformedHttp)?;
    let mut lines = headers.lines();
    let status = lines.next().ok_or(RuntimeClientError::MalformedHttp)?;
    if !status.contains(" 200 ") {
        return Err(RuntimeClientError::HttpStatus);
    }
    let chunked = lines.any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    });
    let body = &raw[header_end + 4..];
    if chunked {
        decode_chunked(body)
    } else {
        Ok(body.to_vec())
    }
}

fn read_connect_stream_response(mut stream: TcpStream) -> Result<Vec<u8>, RuntimeClientError> {
    let (headers, body_prefix) = read_response_head(&mut stream)?;
    validate_success_status(&headers)?;
    let chunked = is_chunked(&headers);
    if !chunked {
        return read_non_chunked_stream_body(stream, body_prefix);
    }
    stream
        .set_read_timeout(Some(STREAM_IDLE_TIMEOUT))
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    read_chunked_stream_body(stream, body_prefix)
}

fn read_response_head(stream: &mut TcpStream) -> Result<(String, Vec<u8>), RuntimeClientError> {
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|_| RuntimeClientError::ConnectionFailed)?;
        if read == 0 {
            return Err(RuntimeClientError::MalformedHttp);
        }
        raw.extend_from_slice(&buffer[..read]);
        if raw.len() > MAX_RESPONSE_BYTES {
            return Err(RuntimeClientError::ResponseTooLarge);
        }
        if let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&raw[..header_end])
                .map_err(|_| RuntimeClientError::MalformedHttp)?
                .to_owned();
            return Ok((headers, raw[header_end + 4..].to_vec()));
        }
    }
}

fn validate_success_status(headers: &str) -> Result<(), RuntimeClientError> {
    let status = headers
        .lines()
        .next()
        .ok_or(RuntimeClientError::MalformedHttp)?;
    if status.contains(" 200 ") {
        Ok(())
    } else {
        Err(RuntimeClientError::HttpStatus)
    }
}

fn is_chunked(headers: &str) -> bool {
    headers.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    })
}

fn read_non_chunked_stream_body(
    mut stream: TcpStream,
    mut body: Vec<u8>,
) -> Result<Vec<u8>, RuntimeClientError> {
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(body),
            Ok(read) => {
                body.extend_from_slice(&buffer[..read]);
                if body.len() > MAX_RESPONSE_BYTES {
                    return Err(RuntimeClientError::ResponseTooLarge);
                }
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if body.is_empty() {
                    return Err(RuntimeClientError::ConnectionFailed);
                }
                return Ok(body);
            }
            Err(_) => return Err(RuntimeClientError::ConnectionFailed),
        }
    }
}

fn read_chunked_stream_body(
    mut stream: TcpStream,
    mut input: Vec<u8>,
) -> Result<Vec<u8>, RuntimeClientError> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match drain_available_chunks(&mut input, &mut output)? {
            ChunkDrain::Complete => return Ok(output),
            ChunkDrain::NeedMore => {}
        }
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(output),
            Ok(read) => {
                input.extend_from_slice(&buffer[..read]);
                if input.len() + output.len() > MAX_RESPONSE_BYTES {
                    return Err(RuntimeClientError::ResponseTooLarge);
                }
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if output.is_empty() {
                    return Err(RuntimeClientError::ConnectionFailed);
                }
                return Ok(output);
            }
            Err(_) => return Err(RuntimeClientError::ConnectionFailed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkDrain {
    Complete,
    NeedMore,
}

fn drain_available_chunks(
    input: &mut Vec<u8>,
    output: &mut Vec<u8>,
) -> Result<ChunkDrain, RuntimeClientError> {
    loop {
        let Some(line_end) = find_crlf(input) else {
            return Ok(ChunkDrain::NeedMore);
        };
        let size_line = std::str::from_utf8(&input[..line_end])
            .map_err(|_| RuntimeClientError::MalformedHttp)?;
        let size_hex = size_line.split(';').next().unwrap_or(size_line);
        let size = usize::from_str_radix(size_hex.trim(), 16)
            .map_err(|_| RuntimeClientError::MalformedHttp)?;
        let chunk_start = line_end + 2;
        if size == 0 {
            return Ok(ChunkDrain::Complete);
        }
        let chunk_end = chunk_start
            .checked_add(size)
            .ok_or(RuntimeClientError::ResponseTooLarge)?;
        let next_chunk = chunk_end + 2;
        if input.len() < next_chunk {
            return Ok(ChunkDrain::NeedMore);
        }
        if &input[chunk_end..next_chunk] != b"\r\n" {
            return Err(RuntimeClientError::MalformedHttp);
        }
        output.extend_from_slice(&input[chunk_start..chunk_end]);
        if output.len() > MAX_RESPONSE_BYTES {
            return Err(RuntimeClientError::ResponseTooLarge);
        }
        input.drain(..next_chunk);
    }
}

fn decode_chunked(body: &[u8]) -> Result<Vec<u8>, RuntimeClientError> {
    let mut output = Vec::new();
    let mut offset = 0;
    loop {
        let line_end = find_crlf(&body[offset..]).ok_or(RuntimeClientError::MalformedHttp)?;
        let size_line = std::str::from_utf8(&body[offset..offset + line_end])
            .map_err(|_| RuntimeClientError::MalformedHttp)?;
        let size_hex = size_line.split(';').next().unwrap_or(size_line);
        let size = usize::from_str_radix(size_hex.trim(), 16)
            .map_err(|_| RuntimeClientError::MalformedHttp)?;
        offset += line_end + 2;
        if size == 0 {
            return Ok(output);
        }
        let chunk_end = offset
            .checked_add(size)
            .ok_or(RuntimeClientError::ResponseTooLarge)?;
        if chunk_end + 2 > body.len() || &body[chunk_end..chunk_end + 2] != b"\r\n" {
            return Err(RuntimeClientError::MalformedHttp);
        }
        output.extend_from_slice(&body[offset..chunk_end]);
        if output.len() > MAX_RESPONSE_BYTES {
            return Err(RuntimeClientError::ResponseTooLarge);
        }
        offset = chunk_end + 2;
    }
}

fn find_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|window| window == b"\r\n")
}

fn encode_connect_frame(payload: &[u8]) -> Result<Vec<u8>, RuntimeClientError> {
    let length = u32::try_from(payload.len()).map_err(|_| RuntimeClientError::ResponseTooLarge)?;
    let mut frame = Vec::with_capacity(payload.len() + 5);
    frame.push(0);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

fn quota_response_is_valid(value: &Value) -> bool {
    value
        .get("response")
        .and_then(Value::as_object)
        .and_then(|response| response.get("groups"))
        .and_then(Value::as_array)
        .is_some()
}

fn trajectories_response_is_valid(value: &Value) -> bool {
    value
        .get("trajectorySummaries")
        .and_then(Value::as_object)
        .is_some()
}

fn decode_connect_frames(body: &[u8]) -> Result<Vec<Value>, RuntimeClientError> {
    let mut frames = Vec::new();
    let mut offset = 0;
    while offset < body.len() {
        if body.len() - offset < 5 {
            return Err(RuntimeClientError::MalformedConnectFrame);
        }
        let flags = body[offset];
        let length = u32::from_be_bytes([
            body[offset + 1],
            body[offset + 2],
            body[offset + 3],
            body[offset + 4],
        ]) as usize;
        offset += 5;
        if length > MAX_CONNECT_FRAME_BYTES || offset + length > body.len() {
            return Err(RuntimeClientError::MalformedConnectFrame);
        }
        let payload = &body[offset..offset + length];
        offset += length;
        let value = serde_json::from_slice::<Value>(payload)
            .map_err(|_| RuntimeClientError::InvalidJson)?;
        if flags & 0b1000_0000 == 0 {
            frames.push(value);
        }
    }
    Ok(frames)
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeClientError {
    #[error("antigravity runtime connection failed")]
    ConnectionFailed,
    #[error("antigravity runtime returned a non-success status")]
    HttpStatus,
    #[error("antigravity runtime returned invalid json")]
    InvalidJson,
    #[error("antigravity conversation id is invalid")]
    InvalidConversationId,
    #[error("antigravity cascade id is invalid")]
    InvalidCascadeId,
    #[error("antigravity runtime returned malformed connect frames")]
    MalformedConnectFrame,
    #[error("antigravity runtime returned malformed http")]
    MalformedHttp,
    #[error("antigravity runtime response is too large")]
    ResponseTooLarge,
    #[error("antigravity runtime identity probe failed")]
    IdentityProbeFailed,
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::sync::{Mutex, MutexGuard};
    use std::thread;

    use serde_json::json;

    use super::*;
    use crate::infrastructure::collectors::antigravity::product_variant::AntigravityProductVariant;

    #[test]
    fn retrieves_quota_summary_with_optional_csrf_header() {
        let _guard = runtime_client_test_lock();
        let server = TestServer::start(|request| {
            assert!(request.contains(
                "POST /exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary HTTP/1.1"
            ));
            assert!(request.contains("x-codeium-csrf-token: local-token"));
            http_response(r#"{"response":{"groups":[]}}"#.as_bytes())
        });

        let client = RuntimeClient::new();
        let summary = client
            .retrieve_user_quota_summary(&endpoint(server.port(), Some("local-token")))
            .expect("quota summary");

        assert_eq!(summary["response"]["groups"], json!([]));
    }

    #[test]
    fn streams_agent_state_updates_with_connect_framed_request() {
        let _guard = runtime_client_test_lock();
        let server = TestServer::start(|request| {
            assert!(request.contains("Content-Type: application/connect+json"));
            assert!(request.contains("Connect-Protocol-Version: 1"));
            let body = http_request_body(&request);
            let frames = decode_connect_frames(body).expect("framed request");
            assert_eq!(
                frames[0]["subscriberId"],
                json!("burnly-readonly-inspection")
            );
            let payload = encode_connect_frame(br#"{"response":{"ok":true}}"#).expect("frame");
            chunked_response(&payload)
        });

        let client = RuntimeClient::new();
        let frames = client
            .stream_agent_state_updates(&endpoint(server.port(), None), "conversation-1")
            .expect("agent state");

        assert_eq!(frames, vec![json!({"response": {"ok": true}})]);
    }

    #[test]
    fn streams_agent_state_updates_from_open_chunked_stream() {
        let _guard = runtime_client_test_lock();
        let server = TestServer::start_streaming(
            |request, stream| {
                assert!(request.contains("Content-Type: application/connect+json"));
                let payload =
                    encode_connect_frame(br#"{"response":{"open":true}}"#).expect("frame");
                let response = [
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec(),
                    format!("{:x}\r\n", payload.len()).into_bytes(),
                    payload,
                    b"\r\n".to_vec(),
                ]
                .concat();
                stream.write_all(&response).expect("write response");
                thread::sleep(STREAM_IDLE_TIMEOUT + Duration::from_millis(250));
            },
            1,
        );

        let client = RuntimeClient::new();
        let frames = client
            .stream_agent_state_updates(&endpoint(server.port(), None), "conversation-1")
            .expect("agent state");

        assert_eq!(frames, vec![json!({"response": {"open": true}})]);
    }

    #[test]
    fn lists_trajectory_summaries_from_endpoint() {
        use crate::infrastructure::collectors::antigravity::runtime_metadata_client::list_trajectory_summaries;

        let _guard = runtime_client_test_lock();
        let server = TestServer::start(|request| {
            assert!(request.contains("GetAllCascadeTrajectories"));
            http_response(include_str!("fixtures/trajectory_list.json").as_bytes())
        });

        let client = RuntimeClient::new();
        let summaries = list_trajectory_summaries(&client, &endpoint(server.port(), None))
            .expect("trajectory summaries");

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].cascade_id, "conversation-a");
        assert_eq!(summaries[0].step_count, Some(12));
        assert_eq!(summaries[1].cascade_id, "conversation-b");
        assert_eq!(summaries[1].step_count, Some(7));
    }

    #[test]
    fn fetches_generator_metadata_with_cascade_id() {
        let _guard = runtime_client_test_lock();
        let server = TestServer::start(|request| {
            assert!(request.contains("GetCascadeTrajectoryGeneratorMetadata"));
            assert!(request.contains(r#""cascadeId":"conversation-1""#));
            http_response(br#"{"generatorMetadata":[{"chatModel":{"model":"gemini","usage":{"inputTokens":"10","outputTokens":"2","model":"gemini"}}}]}"#)
        });

        let client = RuntimeClient::new();
        let response = client
            .get_cascade_trajectory_generator_metadata(
                &endpoint(server.port(), None),
                "conversation-1",
            )
            .expect("generator metadata");

        assert!(response.get("generatorMetadata").is_some());
    }

    #[test]
    fn probes_identity_with_quota_response() {
        let _guard = runtime_client_test_lock();
        let server = TestServer::start(|request| {
            assert!(request.contains(
                "POST /exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary HTTP/1.1"
            ));
            http_response(
                r#"{"response":{"groups":[{"displayName":"Gemini Models","buckets":[]}]}}"#
                    .as_bytes(),
            )
        });

        let client = RuntimeClient::new();
        client
            .probe_identity(&endpoint(server.port(), Some("local-token")))
            .expect("identity probe");
    }

    #[test]
    fn probes_identity_with_trajectory_list_when_quota_is_invalid() {
        let _guard = runtime_client_test_lock();
        let server = TestServer::start_with_connections(
            |request| {
                if request.contains("RetrieveUserQuotaSummary") {
                    return http_response(br#"{"code":"unknown","message":"invalid"}"#);
                }
                assert!(request.contains("GetAllCascadeTrajectories"));
                http_response(br#"{"trajectorySummaries":{"session-1":{"stepCount":1}}}"#)
            },
            2,
        );

        let client = RuntimeClient::new();
        client
            .probe_identity(&endpoint(server.port(), None))
            .expect("identity probe");
    }

    #[test]
    fn rejects_identity_probe_when_rpc_responses_are_unusable() {
        let _guard = runtime_client_test_lock();
        let server = TestServer::start_with_connections(
            |request| {
                if request.contains("RetrieveUserQuotaSummary") {
                    return http_response(br#"{"code":"unknown","message":"invalid"}"#);
                }
                http_response(br#"{"code":"unknown","message":"invalid"}"#)
            },
            2,
        );

        let client = RuntimeClient::new();
        let error = client
            .probe_identity(&endpoint(server.port(), None))
            .expect_err("identity probe");

        assert_eq!(error, RuntimeClientError::IdentityProbeFailed);
    }

    #[test]
    fn rejects_malformed_connect_frames() {
        let error = decode_connect_frames(&[0, 0, 0, 0, 10, b'{']).expect_err("malformed frame");

        assert_eq!(error, RuntimeClientError::MalformedConnectFrame);
    }

    fn endpoint(port: u16, csrf_token: Option<&str>) -> RuntimeEndpoint {
        RuntimeEndpoint {
            variant: AntigravityProductVariant::Ide,
            process_id: 42,
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            csrf_token: csrf_token.map(str::to_owned),
        }
    }

    fn runtime_client_test_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().expect("runtime client test lock")
    }

    fn http_response(body: &[u8]) -> Vec<u8> {
        [
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes(),
            body.to_vec(),
        ]
        .concat()
    }

    fn chunked_response(body: &[u8]) -> Vec<u8> {
        [
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec(),
            format!("{:x}\r\n", body.len()).into_bytes(),
            body.to_vec(),
            b"\r\n0\r\n\r\n".to_vec(),
        ]
        .concat()
    }

    struct TestServer {
        port: u16,
        handle: thread::JoinHandle<()>,
    }

    impl TestServer {
        fn start(handler: fn(String) -> Vec<u8>) -> Self {
            Self::start_with_connections(handler, 1)
        }

        fn start_with_connections(handler: fn(String) -> Vec<u8>, connections: usize) -> Self {
            Self::start_streaming(
                move |request, stream| {
                    let response = handler(request);
                    stream.write_all(&response).expect("write response");
                },
                connections,
            )
        }

        fn start_streaming(
            handler: impl Fn(String, &mut TcpStream) + Send + 'static,
            connections: usize,
        ) -> Self {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("listener");
            let port = listener.local_addr().expect("addr").port();
            let handle = thread::spawn(move || {
                for _ in 0..connections {
                    let Ok((mut stream, _)) = listener.accept() else {
                        break;
                    };
                    let request = read_http_request(&mut stream);
                    let request = String::from_utf8_lossy(&request).into_owned();
                    handler(request, &mut stream);
                }
            });
            Self { port, handle }
        }

        const fn port(&self) -> u16 {
            self.port
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            let handle = std::mem::replace(&mut self.handle, thread::spawn(|| {}));
            handle.join().expect("server thread");
        }
    }

    fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        let mut content_length = None;
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            if content_length.is_none() {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                content_length = headers.lines().find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                });
            }
            let expected_length = header_end + 4 + content_length.unwrap_or(0);
            if request.len() >= expected_length {
                break;
            }
        }
        request
    }

    fn http_request_body(request: &str) -> &[u8] {
        let bytes = request.as_bytes();
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("header end");
        &bytes[header_end + 4..]
    }
}
