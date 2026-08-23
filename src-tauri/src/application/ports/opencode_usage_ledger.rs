#![allow(
    dead_code,
    reason = "chunk 2 defines the ledger contract consumed by the native adapter in chunk 4"
)]

use thiserror::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OpenCodeTokenVector {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) reasoning: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
}

impl OpenCodeTokenVector {
    pub(crate) fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            input: self.input.checked_add(other.input)?,
            output: self.output.checked_add(other.output)?,
            reasoning: self.reasoning.checked_add(other.reasoning)?,
            cache_read: self.cache_read.checked_add(other.cache_read)?,
            cache_write: self.cache_write.checked_add(other.cache_write)?,
        })
    }

    pub(crate) fn checked_sub(self, other: Self) -> Option<Self> {
        Some(Self {
            input: self.input.checked_sub(other.input)?,
            output: self.output.checked_sub(other.output)?,
            reasoning: self.reasoning.checked_sub(other.reasoning)?,
            cache_read: self.cache_read.checked_sub(other.cache_read)?,
            cache_write: self.cache_write.checked_sub(other.cache_write)?,
        })
    }

    pub(crate) const fn is_zero(self) -> bool {
        self.input == 0
            && self.output == 0
            && self.reasoning == 0
            && self.cache_read == 0
            && self.cache_write == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum OpenCodeExactOrigin {
    V1Message,
    V2Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeLedgerOrigin {
    V1Message,
    V2Message,
    CumulativeRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeDataQuality {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeTimestampOrigin {
    SourceReported,
    SourceLifecycle,
    FirstSeen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeRecoveryDisposition {
    Ready,
    DeferredLiveWrite,
    StableIncomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeReconciliationState {
    Complete,
    Partial,
    DeferredLiveWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCodeExactUsage {
    pub(crate) message_id: String,
    pub(crate) activity_at_ms: i64,
    pub(crate) provider_id: String,
    pub(crate) raw_model_id: String,
    pub(crate) tokens: OpenCodeTokenVector,
    pub(crate) cost_micros: Option<u64>,
    pub(crate) origin: OpenCodeExactOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCodeSessionLedgerSnapshot {
    pub(crate) session_id: String,
    pub(crate) source_updated_at_ms: i64,
    pub(crate) recovery_activity_at_ms: Option<i64>,
    pub(crate) cumulative_tokens: OpenCodeTokenVector,
    pub(crate) cumulative_cost_micros: Option<u64>,
    pub(crate) exact_usage: Vec<OpenCodeExactUsage>,
    pub(crate) recovery_disposition: OpenCodeRecoveryDisposition,
    pub(crate) observed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCodeLedgerRecord {
    pub(crate) source_message_id: Option<String>,
    pub(crate) recovery_sequence: Option<u64>,
    pub(crate) session_id: String,
    pub(crate) activity_at_ms: i64,
    pub(crate) timestamp_origin: OpenCodeTimestampOrigin,
    pub(crate) provider_id: Option<String>,
    pub(crate) raw_model_id: String,
    pub(crate) tokens: OpenCodeTokenVector,
    pub(crate) cost_micros: Option<u64>,
    pub(crate) origin: OpenCodeLedgerOrigin,
    pub(crate) quality: OpenCodeDataQuality,
    pub(crate) first_seen_at_ms: i64,
    pub(crate) last_seen_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCodeSessionCheckpoint {
    pub(crate) session_id: String,
    pub(crate) accepted_tokens: OpenCodeTokenVector,
    pub(crate) accepted_cost_micros: Option<u64>,
    pub(crate) observed_source_tokens: OpenCodeTokenVector,
    pub(crate) observed_source_cost_micros: Option<u64>,
    pub(crate) source_updated_at_ms: i64,
    pub(crate) reconciliation_state: OpenCodeReconciliationState,
    pub(crate) next_recovery_sequence: u64,
    pub(crate) first_observed_at_ms: i64,
    pub(crate) last_reconciled_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCodeLedgerReconcileResult {
    pub(crate) records: Vec<OpenCodeLedgerRecord>,
    pub(crate) checkpoint: OpenCodeSessionCheckpoint,
    pub(crate) exact_records_accepted: u32,
    pub(crate) recovery_segments_created: u32,
    pub(crate) late_exact_reclassified: u32,
    pub(crate) late_exact_ignored: u32,
    pub(crate) counter_regressions: u32,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeUsageLedgerError {
    #[error("OpenCode usage ledger storage failed")]
    Storage,
    #[error("OpenCode usage snapshot is incompatible")]
    IncompatibleSnapshot,
}

pub(crate) trait OpenCodeUsageLedger: Send + Sync {
    fn reconcile_session(
        &self,
        snapshot: &OpenCodeSessionLedgerSnapshot,
    ) -> Result<OpenCodeLedgerReconcileResult, OpenCodeUsageLedgerError>;

    fn read_checkpoint(
        &self,
        session_id: &str,
    ) -> Result<Option<OpenCodeSessionCheckpoint>, OpenCodeUsageLedgerError>;

    fn read_session_records(
        &self,
        session_id: &str,
    ) -> Result<Vec<OpenCodeLedgerRecord>, OpenCodeUsageLedgerError>;
}
