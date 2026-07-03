use crate::application::diagnostics::{
    DiagnosticsReport, DiagnosticsReportError, DiagnosticsReportRequest,
};

pub(crate) trait DiagnosticsReportStore: Send + Sync {
    fn report(
        &self,
        request: DiagnosticsReportRequest,
    ) -> Result<DiagnosticsReport, DiagnosticsReportError>;
}
