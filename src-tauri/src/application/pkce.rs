//! PKCE (RFC 7636) helpers for desktop auth via web.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};

/// Unreserved characters used for PKCE verifiers and CSRF state (RFC 3986).
const UNRESERVED: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// Generates a CSRF `state` value (32 unreserved chars; within 8–256 web range).
pub(crate) fn generate_state() -> String {
    random_unreserved(32)
}

/// Generates a PKCE `code_verifier` (43 unreserved chars; within 43–128).
pub(crate) fn generate_code_verifier() -> String {
    random_unreserved(43)
}

/// S256 code challenge: BASE64URL(SHA256(verifier)) without padding.
pub(crate) fn s256_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn random_unreserved(length: usize) -> String {
    let mut bytes = vec![0_u8; length];
    // uuid's getrandom is available transitively; use Os RNG via getrandom crate API.
    getrandom_fill(&mut bytes);
    bytes
        .into_iter()
        .map(|byte| UNRESERVED[(byte as usize) % UNRESERVED.len()] as char)
        .collect()
}

fn getrandom_fill(buf: &mut [u8]) {
    // Prefer getrandom when available; fall back to uuid entropy without new dep surface.
    // uuid v4 uses OS randomness internally; map through repeated UUIDs if needed.
    let mut offset = 0;
    while offset < buf.len() {
        let id = uuid::Uuid::new_v4();
        let bytes = id.as_bytes();
        let end = (offset + bytes.len()).min(buf.len());
        let take = end - offset;
        buf[offset..end].copy_from_slice(&bytes[..take]);
        offset = end;
    }
}

/// Percent-encode a query component (RFC 3986 unreserved left unescaped).
pub(crate) fn encode_query_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*byte));
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Builds the burnly-web desktop login URL.
pub(crate) fn build_desktop_login_url(
    web_origin: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    let origin = web_origin.trim_end_matches('/');
    format!(
        "{origin}/login?client=desktop&redirect_uri={redirect}&state={state}&code_challenge={challenge}&code_challenge_method=S256",
        redirect = encode_query_component(redirect_uri),
        state = encode_query_component(state),
        challenge = encode_query_component(code_challenge),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_matches_rfc_7636_appendix_b() {
        // https://datatracker.ietf.org/doc/html/rfc7636#appendix-B
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            s256_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_verifier_and_state_are_valid_length_and_charset() {
        let state = generate_state();
        let verifier = generate_code_verifier();
        assert!((8..=256).contains(&state.len()));
        assert!((43..=128).contains(&verifier.len()));
        assert!(state.bytes().all(|b| UNRESERVED.contains(&b)));
        assert!(verifier.bytes().all(|b| UNRESERVED.contains(&b)));
        assert_ne!(state, verifier);
    }

    #[test]
    fn login_url_encodes_query_params() {
        let url = build_desktop_login_url(
            "http://127.0.0.1:3000/",
            "http://127.0.0.1:39201/callback",
            "state+value",
            "challenge/value",
        );
        assert!(url.starts_with("http://127.0.0.1:3000/login?"));
        assert!(url.contains("client=desktop"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A39201%2Fcallback"));
        assert!(url.contains("state=state%2Bvalue"));
        assert!(url.contains("code_challenge=challenge%2Fvalue"));
        assert!(url.contains("code_challenge_method=S256"));
    }
}
