use sha2::{Digest, Sha256};

const IDENTITY_PREFIX: &str = "path-sha256:v1:";

pub(crate) struct ProjectPathIdentity {
    key: String,
    fingerprint: [u8; 32],
}

impl ProjectPathIdentity {
    pub(crate) fn from_path(path: &str) -> Self {
        let fingerprint: [u8; 32] = Sha256::digest(path.as_bytes()).into();
        Self {
            key: format!("{IDENTITY_PREFIX}{}", encode_hex(&fingerprint)),
            fingerprint,
        }
    }

    pub(crate) fn is_key(value: &str) -> bool {
        value.starts_with(IDENTITY_PREFIX)
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_deterministic_and_does_not_contain_the_path() {
        let path = "/home/dante/secret-project";
        let first = ProjectPathIdentity::from_path(path);
        let second = ProjectPathIdentity::from_path(path);

        assert_eq!(first.key(), second.key());
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert!(ProjectPathIdentity::is_key(first.key()));
        assert!(!first.key().contains(path));
    }
}
