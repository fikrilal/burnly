use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::application::diagnostics::DiagnosticRedactor;
use crate::application::ports::export_store::{
    ExportCounts, ExportDataset, ExportOccurrence, ExportRow, ExportScope, ExportStore,
    ExportStoreError,
};
use crate::application::ports::export_writer::{
    ExportWriteOutcome, ExportWriter, ExportWriterError,
};

const MAX_EXPORT_ROWS: u64 = 100_000;
const CSV_HEADER: &str = "dataset,occurred_at,source,input_tokens,output_tokens,cache_creation_tokens,cache_read_tokens,total_tokens,cost_amount_micros,cost_currency,cost_status,data_quality\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportRequest {
    pub start_date: String,
    pub end_date: String,
    pub datasets: Vec<ExportDataset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportPreview {
    pub start_date: String,
    pub end_date: String,
    pub datasets: Vec<ExportDatasetPreview>,
    pub total_rows: u64,
    pub estimated_bytes: u64,
    pub privacy_notes: Vec<String>,
    pub preview_token: String,
    pub can_export: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportDatasetPreview {
    pub dataset: ExportDataset,
    pub rows: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfirmedExportRequest {
    pub request: ExportRequest,
    pub preview_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ExportOutcome {
    Exported { rows: u64 },
    Cancelled,
}

#[derive(Clone)]
pub(crate) struct ExportService {
    store: Arc<dyn ExportStore>,
    writer: Arc<dyn ExportWriter>,
}

impl ExportService {
    pub(crate) fn new(store: Arc<dyn ExportStore>, writer: Arc<dyn ExportWriter>) -> Self {
        Self { store, writer }
    }

    pub(crate) fn preview(&self, request: ExportRequest) -> Result<ExportPreview, ExportError> {
        let scope = validated_scope(request)?;
        let counts = self.store.counts(&scope)?;
        Ok(build_preview(scope, counts))
    }

    pub(crate) fn export(
        &self,
        confirmed: ConfirmedExportRequest,
    ) -> Result<ExportOutcome, ExportError> {
        let preview = self.preview(confirmed.request.clone())?;
        if preview.preview_token != confirmed.preview_token {
            return Err(ExportError::StalePreview);
        }
        if !preview.can_export {
            return Err(ExportError::TooLarge);
        }
        let scope = validated_scope(confirmed.request)?;
        let rows = self.store.rows(&scope)?;
        let row_count = u64::try_from(rows.len()).map_err(|_| ExportError::InvalidStoredValue)?;
        if row_count != preview.total_rows {
            return Err(ExportError::StalePreview);
        }
        let csv = serialize_csv(rows)?;
        let suggested_name = format!(
            "burnly-usage-{}-to-{}.csv",
            scope.start_date, scope.end_date
        );
        match self.writer.write_csv(&suggested_name, csv.as_bytes())? {
            ExportWriteOutcome::Written => Ok(ExportOutcome::Exported { rows: row_count }),
            ExportWriteOutcome::Cancelled => Ok(ExportOutcome::Cancelled),
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportError {
    #[error("export date range is invalid")]
    InvalidDateRange,
    #[error("at least one export dataset is required")]
    NoDatasets,
    #[error("export datasets contain duplicates")]
    DuplicateDataset,
    #[error("export preview is stale")]
    StalePreview,
    #[error("export is too large")]
    TooLarge,
    #[error("export storage is unavailable")]
    Unavailable,
    #[error("export data contains invalid values")]
    InvalidStoredValue,
    #[error("export destination is unavailable")]
    DestinationUnavailable,
    #[error("export file could not be written")]
    WriteFailed,
}

impl From<ExportStoreError> for ExportError {
    fn from(value: ExportStoreError) -> Self {
        match value {
            ExportStoreError::Unavailable => Self::Unavailable,
            ExportStoreError::InvalidStoredValue => Self::InvalidStoredValue,
        }
    }
}

impl From<ExportWriterError> for ExportError {
    fn from(value: ExportWriterError) -> Self {
        match value {
            ExportWriterError::DestinationUnavailable => Self::DestinationUnavailable,
            ExportWriterError::WriteFailed => Self::WriteFailed,
        }
    }
}

fn validated_scope(request: ExportRequest) -> Result<ExportScope, ExportError> {
    let start = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .map_err(|_| ExportError::InvalidDateRange)?;
    let end = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d")
        .map_err(|_| ExportError::InvalidDateRange)?;
    if start > end {
        return Err(ExportError::InvalidDateRange);
    }
    if request.datasets.is_empty() {
        return Err(ExportError::NoDatasets);
    }
    let unique = request.datasets.iter().copied().collect::<HashSet<_>>();
    if unique.len() != request.datasets.len() {
        return Err(ExportError::DuplicateDataset);
    }
    Ok(ExportScope {
        start_date: request.start_date,
        end_date: request.end_date,
        datasets: request.datasets,
    })
}

fn build_preview(scope: ExportScope, counts: ExportCounts) -> ExportPreview {
    let datasets = scope
        .datasets
        .iter()
        .map(|dataset| ExportDatasetPreview {
            dataset: *dataset,
            rows: match dataset {
                ExportDataset::DailyUsage => counts.daily_usage,
                ExportDataset::Sessions => counts.sessions,
            },
        })
        .collect::<Vec<_>>();
    let total_rows: u64 = datasets.iter().map(|dataset| dataset.rows).sum();
    let estimated_bytes = (CSV_HEADER.len() as u64).saturating_add(total_rows.saturating_mul(240));
    let preview_token = preview_token(&scope, &datasets);
    ExportPreview {
        start_date: scope.start_date, end_date: scope.end_date, datasets, total_rows,
        estimated_bytes, preview_token, can_export: total_rows <= MAX_EXPORT_ROWS,
        privacy_notes: vec![
            "Exports exclude prompts, collector payloads, credentials, raw project paths, and session identifiers.".to_owned(),
            "Session date filtering uses UTC activity dates.".to_owned(),
        ],
    }
}

fn preview_token(scope: &ExportScope, datasets: &[ExportDatasetPreview]) -> String {
    let mut digest = Sha256::new();
    digest.update(scope.start_date.as_bytes());
    digest.update([0]);
    digest.update(scope.end_date.as_bytes());
    for dataset in datasets {
        digest.update(match dataset.dataset {
            ExportDataset::DailyUsage => b"daily".as_slice(),
            ExportDataset::Sessions => b"sessions".as_slice(),
        });
        digest.update(dataset.rows.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn serialize_csv(rows: Vec<ExportRow>) -> Result<String, ExportError> {
    let mut output = String::from(CSV_HEADER);
    let redactor = DiagnosticRedactor;
    for row in rows {
        let fields = [
            match row.dataset {
                ExportDataset::DailyUsage => "daily_usage".to_owned(),
                ExportDataset::Sessions => "sessions".to_owned(),
            },
            occurrence(row.occurred_at)?,
            redactor.redact(&row.source),
            optional_number(row.input_tokens),
            optional_number(row.output_tokens),
            optional_number(row.cache_creation_tokens),
            optional_number(row.cache_read_tokens),
            row.total_tokens.to_string(),
            optional_number(row.cost_amount_micros),
            row.cost_currency.unwrap_or_default(),
            row.cost_status,
            row.data_quality,
        ];
        output.push_str(
            &fields
                .into_iter()
                .map(|field| csv_field(&field))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    Ok(output)
}

fn optional_number(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn occurrence(value: ExportOccurrence) -> Result<String, ExportError> {
    match value {
        ExportOccurrence::Date(date) => Ok(date),
        ExportOccurrence::TimestampMs(timestamp) => {
            chrono::DateTime::from_timestamp_millis(timestamp)
                .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                .ok_or(ExportError::InvalidStoredValue)
        }
    }
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakeStore {
        counts: ExportCounts,
        rows: Vec<ExportRow>,
    }

    impl ExportStore for FakeStore {
        fn counts(&self, _scope: &ExportScope) -> Result<ExportCounts, ExportStoreError> {
            Ok(self.counts.clone())
        }

        fn rows(&self, _scope: &ExportScope) -> Result<Vec<ExportRow>, ExportStoreError> {
            Ok(self.rows.clone())
        }
    }

    struct FakeWriter {
        outcome: ExportWriteOutcome,
        writes: Mutex<Vec<Vec<u8>>>,
    }

    impl ExportWriter for FakeWriter {
        fn write_csv(
            &self,
            _suggested_name: &str,
            contents: &[u8],
        ) -> Result<ExportWriteOutcome, ExportWriterError> {
            self.writes
                .lock()
                .expect("lock writes")
                .push(contents.to_vec());
            Ok(self.outcome)
        }
    }

    #[test]
    fn preview_binds_scope_counts_and_privacy_policy() {
        let service = service(
            ExportCounts {
                daily_usage: 2,
                sessions: 3,
            },
            Vec::new(),
            ExportWriteOutcome::Written,
        )
        .0;
        let preview = service.preview(request()).expect("preview");
        assert_eq!(preview.total_rows, 5);
        assert_eq!(preview.preview_token.len(), 64);
        assert!(preview
            .privacy_notes
            .iter()
            .any(|note| note.contains("raw project paths")));
        assert!(preview.can_export);
    }

    #[test]
    fn confirmed_export_writes_only_approved_redacted_csv_fields() {
        let row = ExportRow {
            dataset: ExportDataset::Sessions,
            occurred_at: ExportOccurrence::TimestampMs(1_750_291_200_000),
            source: "/private/source".to_owned(),
            input_tokens: Some(1),
            output_tokens: Some(2),
            cache_creation_tokens: None,
            cache_read_tokens: None,
            total_tokens: 3,
            cost_amount_micros: None,
            cost_currency: None,
            cost_status: "unavailable".to_owned(),
            data_quality: "complete".to_owned(),
        };
        let (service, writer) = service(
            ExportCounts {
                daily_usage: 0,
                sessions: 1,
            },
            vec![row],
            ExportWriteOutcome::Written,
        );
        let request = ExportRequest {
            datasets: vec![ExportDataset::Sessions],
            ..request()
        };
        let preview = service.preview(request.clone()).expect("preview");
        assert_eq!(
            service.export(ConfirmedExportRequest {
                request,
                preview_token: preview.preview_token
            }),
            Ok(ExportOutcome::Exported { rows: 1 })
        );
        let csv = String::from_utf8(writer.writes.lock().expect("lock writes")[0].clone())
            .expect("utf8 csv");
        assert!(csv.contains("[redacted-path]"));
        assert!(!csv.contains("/private/source"));
        assert!(!csv.contains("session_id"));
        assert!(!csv.contains("raw_path"));
    }

    #[test]
    fn stale_preview_and_cancelled_picker_do_not_claim_success() {
        let (service, writer) = service(
            ExportCounts {
                daily_usage: 0,
                sessions: 0,
            },
            Vec::new(),
            ExportWriteOutcome::Cancelled,
        );
        assert_eq!(
            service.export(ConfirmedExportRequest {
                request: request(),
                preview_token: "stale".to_owned()
            }),
            Err(ExportError::StalePreview)
        );
        assert!(writer.writes.lock().expect("lock writes").is_empty());
        let preview = service.preview(request()).expect("preview");
        assert_eq!(
            service.export(ConfirmedExportRequest {
                request: request(),
                preview_token: preview.preview_token
            }),
            Ok(ExportOutcome::Cancelled)
        );
    }

    #[test]
    fn writer_failure_remains_explicit() {
        struct FailedWriter;
        impl ExportWriter for FailedWriter {
            fn write_csv(
                &self,
                _suggested_name: &str,
                _contents: &[u8],
            ) -> Result<ExportWriteOutcome, ExportWriterError> {
                Err(ExportWriterError::WriteFailed)
            }
        }

        let service = ExportService::new(
            Arc::new(FakeStore {
                counts: ExportCounts {
                    daily_usage: 0,
                    sessions: 0,
                },
                rows: Vec::new(),
            }),
            Arc::new(FailedWriter),
        );
        let preview = service.preview(request()).expect("preview");

        assert_eq!(
            service.export(ConfirmedExportRequest {
                request: request(),
                preview_token: preview.preview_token,
            }),
            Err(ExportError::WriteFailed)
        );
    }

    fn request() -> ExportRequest {
        ExportRequest {
            start_date: "2026-06-01".to_owned(),
            end_date: "2026-06-30".to_owned(),
            datasets: vec![ExportDataset::DailyUsage, ExportDataset::Sessions],
        }
    }

    fn service(
        counts: ExportCounts,
        rows: Vec<ExportRow>,
        outcome: ExportWriteOutcome,
    ) -> (ExportService, Arc<FakeWriter>) {
        let writer = Arc::new(FakeWriter {
            outcome,
            writes: Mutex::new(Vec::new()),
        });
        (
            ExportService::new(Arc::new(FakeStore { counts, rows }), writer.clone()),
            writer,
        )
    }
}
