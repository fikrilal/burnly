#![allow(
    dead_code,
    reason = "Antigravity runtime client is introduced before collection mapping in chunk 4"
)]

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::{json, Value};
use thiserror::Error;

use super::RuntimeEndpoint;

const QUOTA_PATH: &str = "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary";
const STREAM_PATH: &str = "/exa.language_server_pb.LanguageServerService/StreamAgentStateUpdates";
const TIMEOUT: Duration = Duration::from_secs(3);
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
        let response = post_json(endpoint, STREAM_PATH, &body, ContentType::ConnectJson)?;
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
    let mut stream =
        TcpStream::connect_timeout(&SocketAddr::new(endpoint.host, endpoint.port), TIMEOUT)
            .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;

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

    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|_| RuntimeClientError::ConnectionFailed)?;
    read_http_response(stream)
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
    #[error("antigravity runtime returned malformed connect frames")]
    MalformedConnectFrame,
    #[error("antigravity runtime returned malformed http")]
    MalformedHttp,
    #[error("antigravity runtime response is too large")]
    ResponseTooLarge,
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::thread;

    use serde_json::json;

    use super::*;
    use crate::infrastructure::collectors::antigravity::product_variant::AntigravityProductVariant;

    #[test]
    fn retrieves_quota_summary_with_optional_csrf_header() {
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
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("listener");
            let port = listener.local_addr().expect("addr").port();
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept");
                let request = read_http_request(&mut stream);
                let request = String::from_utf8_lossy(&request).into_owned();
                let response = handler(request);
                stream.write_all(&response).expect("write response");
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
