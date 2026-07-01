use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClineMessageUsage {
    pub message_id: String,
    pub timestamp_ms: i64,
    pub metrics: ClineUsageMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClineUsageMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_micros: u64,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum ClineMessageError {
    #[error("cline message json is malformed")]
    InvalidJson,
    #[error("cline message json is incompatible")]
    Incompatible,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageEnvelope {
    session_id: String,
    messages: Vec<MessageRecord>,
}

#[derive(Debug, Deserialize)]
struct MessageRecord {
    id: String,
    ts: Option<i64>,
    metrics: Option<RawMetrics>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMetrics {
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    cost: f64,
}

pub(crate) fn decode_messages(input: &str) -> Result<Vec<ClineMessageUsage>, ClineMessageError> {
    let envelope = serde_json::from_str::<MessageEnvelope>(input).map_err(decode_failure)?;
    if envelope.session_id.trim().is_empty() {
        return Err(ClineMessageError::Incompatible);
    }

    let mut usage = Vec::new();
    for message in envelope.messages {
        let Some(metrics) = message.metrics else {
            continue;
        };
        let timestamp_ms = message.ts.ok_or(ClineMessageError::Incompatible)?;
        if message.id.trim().is_empty() || timestamp_ms < 0 {
            return Err(ClineMessageError::Incompatible);
        }
        usage.push(ClineMessageUsage {
            message_id: message.id,
            timestamp_ms,
            metrics: metrics.try_into()?,
        });
    }

    Ok(usage)
}

impl TryFrom<RawMetrics> for ClineUsageMetrics {
    type Error = ClineMessageError;

    fn try_from(value: RawMetrics) -> Result<Self, Self::Error> {
        if value.input_tokens < 0
            || value.output_tokens < 0
            || value.cache_read_tokens < 0
            || value.cache_write_tokens < 0
            || !value.cost.is_finite()
            || value.cost < 0.0
        {
            return Err(ClineMessageError::Incompatible);
        }

        Ok(Self {
            input_tokens: value.input_tokens as u64,
            output_tokens: value.output_tokens as u64,
            cache_read_tokens: value.cache_read_tokens as u64,
            cache_write_tokens: value.cache_write_tokens as u64,
            cost_micros: cost_micros(value.cost)?,
        })
    }
}

fn cost_micros(cost: f64) -> Result<u64, ClineMessageError> {
    let micros = cost * 1_000_000.0;
    if !micros.is_finite() || micros < 0.0 || micros > u64::MAX as f64 {
        return Err(ClineMessageError::Incompatible);
    }
    Ok(micros.round() as u64)
}

fn decode_failure(error: serde_json::Error) -> ClineMessageError {
    match error.classify() {
        serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
            ClineMessageError::InvalidJson
        }
        serde_json::error::Category::Data | serde_json::error::Category::Io => {
            ClineMessageError::Incompatible
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/cline/messages/"
    );

    #[test]
    fn decodes_usage_metrics_without_requiring_message_content() {
        let usage = decode_messages(fixture("valid-session.messages.json")).expect("valid usage");

        assert_eq!(usage.len(), 2);
        assert_eq!(usage[0].message_id, "msg_2");
        assert_eq!(usage[0].timestamp_ms, 1_782_782_160_000);
        assert_eq!(usage[0].metrics.input_tokens, 5_000);
        assert_eq!(usage[0].metrics.output_tokens, 250);
        assert_eq!(usage[0].metrics.cache_read_tokens, 0);
        assert_eq!(usage[0].metrics.cache_write_tokens, 0);
        assert_eq!(usage[0].metrics.cost_micros, 4_750);
        assert_eq!(usage[1].metrics.input_tokens, 7_000);
        assert_eq!(usage[1].metrics.cost_micros, 6_750);
    }

    #[test]
    fn accepts_active_session_messages() {
        let usage = decode_messages(fixture("active-session.messages.json")).expect("active usage");

        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].message_id, "msg_active_1");
        assert_eq!(usage[0].metrics.cache_read_tokens, 200);
    }

    #[test]
    fn rejects_malformed_usage_shapes() {
        assert_eq!(
            decode_messages(fixture("malformed.json")).expect_err("malformed fixture"),
            ClineMessageError::Incompatible
        );
    }

    #[test]
    fn rejects_invalid_json() {
        assert_eq!(
            decode_messages("{").expect_err("invalid json"),
            ClineMessageError::InvalidJson
        );
    }

    fn fixture(name: &str) -> &'static str {
        let path = format!("{FIXTURES}{name}");
        Box::leak(
            std::fs::read_to_string(path)
                .expect("fixture")
                .into_boxed_str(),
        )
    }
}
