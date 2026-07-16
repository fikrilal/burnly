//! Cloud API failure type mapped from transport and problem+json.

use crate::application::cloud_session::CloudSessionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CloudApiErrorKind {
    Network,
    Timeout,
    Unauthorized,
    Forbidden,
    Validation,
    RateLimited,
    Problem,
    Decode,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloudFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloudApiError {
    pub kind: CloudApiErrorKind,
    pub message: String,
    pub code: Option<String>,
    pub status: Option<u16>,
    pub trace_id: Option<String>,
    pub field_errors: Vec<CloudFieldError>,
    pub retry_after_seconds: Option<u64>,
}

impl CloudApiError {
    pub(crate) fn network(message: impl Into<String>) -> Self {
        Self {
            kind: CloudApiErrorKind::Network,
            message: message.into(),
            code: None,
            status: None,
            trace_id: None,
            field_errors: Vec::new(),
            retry_after_seconds: None,
        }
    }

    pub(crate) fn timeout(message: impl Into<String>) -> Self {
        Self {
            kind: CloudApiErrorKind::Timeout,
            message: message.into(),
            code: None,
            status: None,
            trace_id: None,
            field_errors: Vec::new(),
            retry_after_seconds: None,
        }
    }

    pub(crate) fn decode(message: impl Into<String>) -> Self {
        Self {
            kind: CloudApiErrorKind::Decode,
            message: message.into(),
            code: None,
            status: None,
            trace_id: None,
            field_errors: Vec::new(),
            retry_after_seconds: None,
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: CloudApiErrorKind::Internal,
            message: message.into(),
            code: None,
            status: None,
            trace_id: None,
            field_errors: Vec::new(),
            retry_after_seconds: None,
        }
    }

    pub(crate) fn from_problem(
        status: u16,
        code: Option<String>,
        message: String,
        trace_id: Option<String>,
        field_errors: Vec<CloudFieldError>,
    ) -> Self {
        Self::from_problem_with_retry(status, code, message, trace_id, field_errors, None)
    }

    pub(crate) fn from_problem_with_retry(
        status: u16,
        code: Option<String>,
        message: String,
        trace_id: Option<String>,
        field_errors: Vec<CloudFieldError>,
        retry_after_seconds: Option<u64>,
    ) -> Self {
        let kind = match status {
            401 => CloudApiErrorKind::Unauthorized,
            403 => CloudApiErrorKind::Forbidden,
            400 if code.as_deref() == Some("VALIDATION_FAILED") => CloudApiErrorKind::Validation,
            429 => CloudApiErrorKind::RateLimited,
            _ => CloudApiErrorKind::Problem,
        };
        Self {
            kind,
            message,
            code,
            status: Some(status),
            trace_id,
            field_errors,
            retry_after_seconds,
        }
    }

    pub(crate) fn is_unauthorized(&self) -> bool {
        self.kind == CloudApiErrorKind::Unauthorized
            || self.code.as_deref() == Some("UNAUTHORIZED")
            || self.status == Some(401)
    }
}

impl From<CloudApiError> for CloudSessionError {
    fn from(value: CloudApiError) -> Self {
        match value.kind {
            CloudApiErrorKind::Unauthorized
                if value
                    .code
                    .as_deref()
                    .is_some_and(|code| code.starts_with("AUTH_")) =>
            {
                CloudSessionError::RefreshFailed { code: value.code }
            }
            CloudApiErrorKind::Unauthorized => CloudSessionError::RefreshFailed {
                code: value.code.or_else(|| Some("UNAUTHORIZED".into())),
            },
            _ => CloudSessionError::RefreshFailed { code: value.code },
        }
    }
}
