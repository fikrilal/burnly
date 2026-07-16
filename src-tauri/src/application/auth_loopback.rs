//! Single-shot loopback HTTP listener for desktop OAuth-style callbacks.
//!
//! Only binds to localhost / 127.0.0.1. Returns `code` and `state` query params.
//! Never logs secrets.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

const ACCEPT_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum LoopbackError {
    #[error("redirect URI is not a loopback HTTP URL")]
    InvalidRedirectUri,
    #[error("failed to bind loopback listener")]
    BindFailed,
    #[error("callback timed out")]
    Timeout,
    #[error("callback cancelled")]
    Cancelled,
    #[error("invalid callback request")]
    InvalidRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopbackBind {
    pub addr: SocketAddr,
    pub path: String,
}

/// Parses `http://127.0.0.1:PORT/path` or `http://localhost:PORT/path`.
pub(crate) fn parse_loopback_redirect(redirect_uri: &str) -> Result<LoopbackBind, LoopbackError> {
    let uri = redirect_uri.trim();
    let rest = uri
        .strip_prefix("http://")
        .ok_or(LoopbackError::InvalidRedirectUri)?;
    let (host_port, path) = match rest.split_once('/') {
        Some((host_port, path)) => (host_port, format!("/{path}")),
        None => (rest, "/".to_owned()),
    };
    let (host, port) = match host_port.split_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>()
                .map_err(|_| LoopbackError::InvalidRedirectUri)?,
        ),
        None => (host_port, 80_u16),
    };
    if host != "127.0.0.1" && !host.eq_ignore_ascii_case("localhost") {
        return Err(LoopbackError::InvalidRedirectUri);
    }
    let host_ip = if host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1"
    } else {
        host
    };
    let addr: SocketAddr = format!("{host_ip}:{port}")
        .parse()
        .map_err(|_| LoopbackError::InvalidRedirectUri)?;
    Ok(LoopbackBind { addr, path })
}

pub(crate) struct LoopbackServer {
    listener: TcpListener,
    path: String,
    cancel: Arc<AtomicBool>,
}

impl LoopbackServer {
    pub(crate) fn bind(redirect_uri: &str, cancel: Arc<AtomicBool>) -> Result<Self, LoopbackError> {
        let bind = parse_loopback_redirect(redirect_uri)?;
        let listener = TcpListener::bind(bind.addr).map_err(|_| LoopbackError::BindFailed)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| LoopbackError::BindFailed)?;
        Ok(Self {
            listener,
            path: bind.path,
            cancel,
        })
    }

    /// Accepts a single matching callback request or times out / cancels.
    pub(crate) fn accept_once(self, timeout: Duration) -> Result<(String, String), LoopbackError> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(LoopbackError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(LoopbackError::Timeout);
            }

            match self.listener.accept() {
                Ok((stream, _)) => match handle_connection(stream, &self.path) {
                    Ok(pair) => return Ok(pair),
                    Err(LoopbackError::InvalidRequest) => continue,
                    Err(error) => return Err(error),
                },
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(ACCEPT_POLL);
                }
                Err(_) => return Err(LoopbackError::InvalidRequest),
            }
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    expected_path: &str,
) -> Result<(String, String), LoopbackError> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0_u8; 8192];
    let n = stream
        .read(&mut buf)
        .map_err(|_| LoopbackError::InvalidRequest)?;
    let request = std::str::from_utf8(&buf[..n]).map_err(|_| LoopbackError::InvalidRequest)?;
    let (code, state) = parse_callback_request(request, expected_path)?;
    let body = "<!doctype html><html><body><p>Sign-in complete. You can close this window and return to Burnly.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    Ok((code, state))
}

fn parse_callback_request(
    request: &str,
    expected_path: &str,
) -> Result<(String, String), LoopbackError> {
    let first_line = request
        .lines()
        .next()
        .ok_or(LoopbackError::InvalidRequest)?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().ok_or(LoopbackError::InvalidRequest)?;
    let target = parts.next().ok_or(LoopbackError::InvalidRequest)?;
    if method != "GET" {
        return Err(LoopbackError::InvalidRequest);
    }
    let (path, query) = target
        .split_once('?')
        .map(|(p, q)| (p, Some(q)))
        .unwrap_or((target, None));
    if path != expected_path {
        return Err(LoopbackError::InvalidRequest);
    }
    let query = query.ok_or(LoopbackError::InvalidRequest)?;
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "code" => code = Some(percent_decode(value)),
            "state" => state = Some(percent_decode(value)),
            _ => {}
        }
    }
    let code = code.filter(|value| !value.is_empty());
    let state = state.filter(|value| !value.is_empty());
    match (code, state) {
        (Some(code), Some(state)) => Ok((code, state)),
        _ => Err(LoopbackError::InvalidRequest),
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &input[i + 1..i + 3];
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_loopback_redirect() {
        let bind = parse_loopback_redirect("http://127.0.0.1:39201/callback").expect("bind");
        assert_eq!(bind.addr.port(), 39201);
        assert_eq!(bind.path, "/callback");
    }

    #[test]
    fn rejects_non_loopback() {
        assert_eq!(
            parse_loopback_redirect("http://example.com:39201/callback"),
            Err(LoopbackError::InvalidRedirectUri)
        );
    }

    #[test]
    fn parses_get_callback_query() {
        let request = "GET /callback?code=abc%2B1&state=xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let (code, state) = parse_callback_request(request, "/callback").expect("parse");
        assert_eq!(code, "abc+1");
        assert_eq!(state, "xyz");
    }

    #[test]
    fn rejects_missing_code() {
        let request = "GET /callback?state=xyz HTTP/1.1\r\n\r\n";
        assert_eq!(
            parse_callback_request(request, "/callback"),
            Err(LoopbackError::InvalidRequest)
        );
    }
}
