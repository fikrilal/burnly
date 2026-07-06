use std::collections::BTreeSet;

use thiserror::Error;

use super::product_variant::AntigravityProductVariant;
use super::AntigravityUsageRecord;

const MAX_BLOB_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOKEN_VALUE: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedProtobufUsageRecord {
    pub(crate) raw_model_id: String,
    pub(crate) model_label: String,
    pub(crate) response_id: Option<String>,
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) thinking_output_tokens: u64,
    pub(crate) response_output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) observed_at_ms: Option<i64>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtobufUsageError {
    #[error("antigravity protobuf blob is too large")]
    BlobTooLarge,
    #[error("antigravity protobuf blob is malformed")]
    Malformed,
    #[error("antigravity protobuf usage token value is invalid")]
    InvalidTokenValue,
    #[error("antigravity protobuf usage token total overflowed")]
    TokenOverflow,
}

pub(crate) fn parse_gen_metadata_rows(
    variant: AntigravityProductVariant,
    conversation_id: &str,
    rows: &[Vec<u8>],
    session_timestamp_ms: i64,
) -> Result<Vec<AntigravityUsageRecord>, ProtobufUsageError> {
    let mut records = Vec::new();
    let mut seen_response_ids = BTreeSet::new();
    let mut rejected = 0_u32;

    for blob in rows {
        match parse_gen_metadata_blob(blob, session_timestamp_ms) {
            Ok(Some(parsed)) => {
                if let Some(response_id) = parsed.response_id.as_deref() {
                    if !seen_response_ids.insert(response_id.to_owned()) {
                        rejected = rejected.saturating_add(1);
                        continue;
                    }
                }
                records.push(to_usage_record(variant, conversation_id, parsed));
            }
            Ok(None) => {}
            Err(_) => {
                rejected = rejected.saturating_add(1);
            }
        }
    }

    if records.is_empty() && rejected > 0 {
        return Err(ProtobufUsageError::Malformed);
    }

    Ok(records)
}

pub(crate) fn parse_trajectory_created_ms(blob: &[u8]) -> Option<i64> {
    if blob.len() > MAX_BLOB_BYTES {
        return None;
    }
    message_field(blob, 2).and_then(proto_timestamp_ms)
}

fn parse_gen_metadata_blob(
    blob: &[u8],
    session_timestamp_ms: i64,
) -> Result<Option<ParsedProtobufUsageRecord>, ProtobufUsageError> {
    if blob.len() > MAX_BLOB_BYTES {
        return Err(ProtobufUsageError::BlobTooLarge);
    }

    let chat_model = message_field(blob, 1).ok_or(ProtobufUsageError::Malformed)?;
    let usage = message_field(chat_model, 4).ok_or(ProtobufUsageError::Malformed)?;

    let observed_at_ms = message_field(chat_model, 9)
        .and_then(|generation| message_field(generation, 4))
        .and_then(proto_timestamp_ms)
        .filter(|value| *value > 0)
        .or((session_timestamp_ms > 0).then_some(session_timestamp_ms));

    let fixed_input = token_field(usage, 1)?;
    let new_input = token_field(usage, 2)?;
    let input_tokens = checked_add_tokens(fixed_input, new_input)?;
    let cache_read_tokens = token_field(usage, 5)?;
    let response_output_tokens = token_field(usage, 9)?;
    let thinking_output_tokens = token_field(usage, 10)?;
    let output_tokens = checked_add_tokens(response_output_tokens, thinking_output_tokens)?;

    if input_tokens == 0
        && output_tokens == 0
        && cache_read_tokens == 0
        && thinking_output_tokens == 0
    {
        return Ok(None);
    }

    let response_id = string_field(usage, 11)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let model_label = string_field(chat_model, 19)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_owned();

    Ok(Some(ParsedProtobufUsageRecord {
        raw_model_id: model_label.clone(),
        model_label,
        response_id,
        input_tokens,
        output_tokens,
        thinking_output_tokens,
        response_output_tokens,
        cache_read_tokens,
        observed_at_ms,
    }))
}

fn to_usage_record(
    variant: AntigravityProductVariant,
    conversation_id: &str,
    parsed: ParsedProtobufUsageRecord,
) -> AntigravityUsageRecord {
    AntigravityUsageRecord {
        variant,
        conversation_id: conversation_id.to_owned(),
        raw_model_id: parsed.raw_model_id,
        model_label: parsed.model_label,
        api_provider: None,
        response_id: parsed.response_id,
        input_tokens: parsed.input_tokens,
        output_tokens: parsed.output_tokens,
        thinking_output_tokens: parsed.thinking_output_tokens,
        response_output_tokens: parsed.response_output_tokens,
        cache_read_tokens: parsed.cache_read_tokens,
        cache_write_tokens: 0,
        consumed_credits: None,
        flow_credits_used: None,
        prompt_credits_used: None,
    }
}

fn token_field(blob: &[u8], field: u64) -> Result<u64, ProtobufUsageError> {
    let value = varint_field(blob, field).unwrap_or(0);
    if value > MAX_TOKEN_VALUE {
        return Err(ProtobufUsageError::InvalidTokenValue);
    }
    Ok(value)
}

fn checked_add_tokens(left: u64, right: u64) -> Result<u64, ProtobufUsageError> {
    left.checked_add(right).ok_or(ProtobufUsageError::TokenOverflow)
}

fn proto_timestamp_ms(blob: &[u8]) -> Option<i64> {
    let seconds = i64::try_from(varint_field(blob, 1)?).ok()?;
    let nanos = i64::try_from(varint_field(blob, 2).unwrap_or(0)).ok()?;
    if !(0..=999_999_999).contains(&nanos) {
        return None;
    }
    seconds
        .checked_mul(1000)?
        .checked_add(nanos / 1_000_000)
}

enum Wire<'a> {
    Varint(u64),
    Len(&'a [u8]),
    Fixed64,
    Fixed32,
}

struct ProtoReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ProtoReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_varint(&mut self) -> Option<u64> {
        let mut result = 0_u64;
        let mut shift = 0_u32;
        loop {
            let byte = *self.buf.get(self.pos)?;
            self.pos += 1;
            result |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(result);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    fn next_field(&mut self) -> Option<(u64, Wire<'a>)> {
        if self.pos >= self.buf.len() {
            return None;
        }
        let tag = self.read_varint()?;
        let field = tag >> 3;
        let wire = match tag & 0x7 {
            0 => Wire::Varint(self.read_varint()?),
            1 => {
                self.pos = self.pos.checked_add(8).filter(|pos| *pos <= self.buf.len())?;
                Wire::Fixed64
            }
            2 => {
                let len = usize::try_from(self.read_varint()?).ok()?;
                let end = self.pos.checked_add(len).filter(|pos| *pos <= self.buf.len())?;
                let bytes = &self.buf[self.pos..end];
                self.pos = end;
                Wire::Len(bytes)
            }
            5 => {
                self.pos = self.pos.checked_add(4).filter(|pos| *pos <= self.buf.len())?;
                Wire::Fixed32
            }
            _ => return None,
        };
        Some((field, wire))
    }
}

fn message_field(blob: &[u8], field: u64) -> Option<&[u8]> {
    let mut reader = ProtoReader::new(blob);
    while let Some((found, wire)) = reader.next_field() {
        if found == field {
            if let Wire::Len(bytes) = wire {
                return Some(bytes);
            }
        }
    }
    None
}

fn varint_field(blob: &[u8], field: u64) -> Option<u64> {
    let mut reader = ProtoReader::new(blob);
    while let Some((found, wire)) = reader.next_field() {
        if found == field {
            if let Wire::Varint(value) = wire {
                return Some(value);
            }
        }
    }
    None
}

fn string_field(blob: &[u8], field: u64) -> Option<&str> {
    message_field(blob, field)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn enc_varint(field: u64, value: u64) -> Vec<u8> {
        let mut out = encode_varint(field << 3);
        out.extend(encode_varint(value));
        out
    }

    pub(crate) fn enc_len(field: u64, payload: &[u8]) -> Vec<u8> {
        let mut out = encode_varint((field << 3) | 2);
        out.extend(encode_varint(payload.len() as u64));
        out.extend_from_slice(payload);
        out
    }

    fn encode_varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                break;
            }
        }
        out
    }

    pub(crate) fn sample_gen_metadata_blob(response_id: &str) -> Vec<u8> {
        let mut usage = Vec::new();
        usage.extend(enc_varint(1, 100));
        usage.extend(enc_varint(2, 50));
        usage.extend(enc_varint(5, 12));
        usage.extend(enc_varint(9, 20));
        usage.extend(enc_varint(10, 5));
        usage.extend(enc_len(11, response_id.as_bytes()));

        let mut chat_model = Vec::new();
        chat_model.extend(enc_len(4, &usage));
        chat_model.extend(enc_len(19, b"gemini-3-flash-a"));

        enc_len(1, &chat_model)
    }

    pub(crate) fn sample_trajectory_metadata_blob() -> Vec<u8> {
        let mut created = Vec::new();
        created.extend(enc_varint(1, 1_781_502_653));
        created.extend(enc_varint(2, 0));
        enc_len(2, &created)
    }

    #[test]
    fn parses_usage_fields_from_gen_metadata_blob() {
        let records = parse_gen_metadata_rows(
            AntigravityProductVariant::Cli,
            "conversation-a",
            &[sample_gen_metadata_blob("response-1")],
            0,
        )
        .expect("records");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].input_tokens, 150);
        assert_eq!(records[0].cache_read_tokens, 12);
        assert_eq!(records[0].response_output_tokens, 20);
        assert_eq!(records[0].thinking_output_tokens, 5);
        assert_eq!(records[0].output_tokens, 25);
        assert_eq!(records[0].response_id.as_deref(), Some("response-1"));
        assert_eq!(records[0].model_label, "gemini-3-flash-a");
    }

    #[test]
    fn deduplicates_repeated_response_ids() {
        let blob = sample_gen_metadata_blob("response-dup");
        let records = parse_gen_metadata_rows(
            AntigravityProductVariant::Cli,
            "conversation-a",
            &[blob.clone(), blob],
            0,
        )
        .expect("records");

        assert_eq!(records.len(), 1);
    }

    #[test]
    fn malformed_blob_fails_soft_for_single_row() {
        let records = parse_gen_metadata_rows(
            AntigravityProductVariant::Cli,
            "conversation-a",
            &[vec![0xFF, 0xFF]],
            0,
        )
        .expect_err("malformed only row");

        assert_eq!(records, ProtobufUsageError::Malformed);
    }

    #[test]
    fn parses_trajectory_created_timestamp() {
        let timestamp = parse_trajectory_created_ms(&sample_trajectory_metadata_blob())
            .expect("timestamp");
        assert_eq!(timestamp, 1_781_502_653_000);
    }
}