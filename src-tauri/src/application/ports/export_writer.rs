use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportWriteOutcome {
    Written,
    Cancelled,
}

pub(crate) trait ExportWriter: Send + Sync {
    fn write_csv(
        &self,
        suggested_name: &str,
        contents: &[u8],
    ) -> Result<ExportWriteOutcome, ExportWriterError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportWriterError {
    #[error("export destination is unavailable")]
    DestinationUnavailable,
    #[error("export file could not be written")]
    WriteFailed,
}
