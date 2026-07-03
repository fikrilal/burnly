use crate::application::diagnostics::DiagnosticEvent;

pub(crate) trait DiagnosticRecorder: Send + Sync {
    fn record(&self, event: DiagnosticEvent);
}
