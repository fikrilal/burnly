//! Map cloud transport errors to collect-sync remote failures.

use crate::application::ports::collect_sync_remote::{
    CollectSyncFieldError, CollectSyncRemoteError,
};

use super::error::{CloudApiError, CloudApiErrorKind};

pub(crate) fn map_cloud_api_error(error: CloudApiError) -> CollectSyncRemoteError {
    match error.kind {
        CloudApiErrorKind::Network => CollectSyncRemoteError::Network {
            message: error.message,
        },
        CloudApiErrorKind::Timeout => CollectSyncRemoteError::Timeout {
            message: error.message,
        },
        CloudApiErrorKind::Unauthorized => CollectSyncRemoteError::Unauthorized {
            code: error.code,
            message: error.message,
        },
        CloudApiErrorKind::Forbidden => CollectSyncRemoteError::Forbidden {
            code: error.code,
            message: error.message,
        },
        CloudApiErrorKind::Validation => CollectSyncRemoteError::Validation {
            code: error.code,
            message: error.message,
            field_errors: map_field_errors(error.field_errors),
        },
        CloudApiErrorKind::RateLimited => CollectSyncRemoteError::RateLimited {
            code: error.code,
            message: error.message,
            retry_after_seconds: error.retry_after_seconds,
        },
        CloudApiErrorKind::Decode => CollectSyncRemoteError::Decode {
            message: error.message,
        },
        CloudApiErrorKind::Internal => CollectSyncRemoteError::Internal {
            message: error.message,
        },
        CloudApiErrorKind::Problem => map_problem(error),
    }
}

fn map_problem(error: CloudApiError) -> CollectSyncRemoteError {
    match (error.status, error.code.as_deref()) {
        (Some(404), Some("SYNC_DEVICE_NOT_FOUND")) | (Some(404), _) => {
            CollectSyncRemoteError::DeviceNotFound {
                message: error.message,
            }
        }
        (_, Some("SYNC_CONTRACT_UNSUPPORTED")) => CollectSyncRemoteError::ContractUnsupported {
            message: error.message,
        },
        (Some(409), Some("IDEMPOTENCY_IN_PROGRESS")) => {
            CollectSyncRemoteError::IdempotencyInProgress {
                message: error.message,
            }
        }
        (Some(409), _) => CollectSyncRemoteError::Conflict {
            code: error.code,
            message: error.message,
        },
        (Some(413), _) | (_, Some("SYNC_PAYLOAD_TOO_LARGE")) => {
            CollectSyncRemoteError::PayloadTooLarge {
                message: error.message,
            }
        }
        (Some(400), Some("VALIDATION_FAILED")) => CollectSyncRemoteError::Validation {
            code: error.code,
            message: error.message,
            field_errors: map_field_errors(error.field_errors),
        },
        _ => CollectSyncRemoteError::Problem {
            code: error.code,
            status: error.status,
            message: error.message,
            trace_id: error.trace_id,
        },
    }
}

fn map_field_errors(
    field_errors: Vec<super::error::CloudFieldError>,
) -> Vec<CollectSyncFieldError> {
    field_errors
        .into_iter()
        .map(|field| CollectSyncFieldError {
            field: field.field,
            code: field.code,
            message: field.message,
        })
        .collect()
}
