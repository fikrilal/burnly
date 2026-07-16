//! Best-effort access-token expiry extraction from a JWT `exp` claim.
//!
//! Desktop does not verify the JWT signature here; burnly-api remains the
//! authority. Expiry is used only for preflight refresh timing.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

/// Returns access token expiry as Unix epoch milliseconds when `exp` is present.
pub(crate) fn access_expires_at_ms(access_token: &str) -> Option<i64> {
    let parts: Vec<&str> = access_token.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let payload = parts[1];
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let exp = value.get("exp")?.as_i64().or_else(|| {
        value
            .get("exp")?
            .as_u64()
            .and_then(|seconds| i64::try_from(seconds).ok())
    })?;
    exp.checked_mul(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_payload(json: &str) -> String {
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    #[test]
    fn reads_exp_claim_as_epoch_ms() {
        let payload = encode_payload(r#"{"exp":1700000000,"sub":"user"}"#);
        let token = format!("aaa.{payload}.sig");
        assert_eq!(access_expires_at_ms(&token), Some(1_700_000_000_000));
    }

    #[test]
    fn rejects_malformed_tokens() {
        assert_eq!(access_expires_at_ms("not-a-jwt"), None);
        assert_eq!(access_expires_at_ms("a.b"), None);
    }
}
