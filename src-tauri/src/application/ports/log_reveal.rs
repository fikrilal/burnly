#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogRevealCapability {
    pub status: LogRevealAvailability,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogRevealAvailability {
    Available,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogRevealOutcome {
    Revealed,
    Missing,
    Unsupported,
}

pub(crate) trait LogRevealPort: Send + Sync {
    fn capability(&self) -> LogRevealCapability;
    fn reveal_logs(&self) -> Result<LogRevealOutcome, LogRevealError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogRevealError {
    Failed,
}
