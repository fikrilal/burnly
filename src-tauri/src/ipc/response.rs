use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use uuid::Uuid;

pub(crate) const CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub(super) struct IpcResponse<T>(IpcResponseBody<T>);

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum IpcResponseBody<T> {
    Success {
        ok: bool,
        data: T,
        meta: ResponseMeta,
    },
    Failure {
        ok: bool,
        error: IpcError,
        meta: ResponseMeta,
    },
}

impl<T> IpcResponse<T> {
    pub(super) fn success(data: T) -> Self {
        Self::success_with_meta(data, ResponseMeta::generate())
    }

    pub(super) fn failure(error: IpcError) -> Self {
        Self::failure_with_meta(error, ResponseMeta::generate())
    }

    fn success_with_meta(data: T, meta: ResponseMeta) -> Self {
        Self(IpcResponseBody::Success {
            ok: true,
            data,
            meta,
        })
    }

    fn failure_with_meta(error: IpcError, meta: ResponseMeta) -> Self {
        Self(IpcResponseBody::Failure {
            ok: false,
            error,
            meta,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseMeta {
    contract_version: u16,
    request_id: String,
    generated_at: String,
}

impl ResponseMeta {
    fn generate() -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            request_id: Uuid::new_v4().to_string(),
            generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }

    #[cfg(test)]
    fn fixed(request_id: &str, generated_at: &str) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            request_id: request_id.to_owned(),
            generated_at: generated_at.to_owned(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IpcError {
    code: &'static str,
    message: &'static str,
    category: ErrorCategory,
    retryable: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    field_errors: Vec<FieldError>,
    details: (),
}

impl IpcError {
    pub(super) fn new(
        code: &'static str,
        message: &'static str,
        category: ErrorCategory,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            message,
            category,
            retryable,
            field_errors: Vec::new(),
            details: (),
        }
    }

    pub(super) fn with_field_errors(mut self, field_errors: Vec<FieldError>) -> Self {
        self.field_errors = field_errors;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ErrorCategory {
    Validation,
    Conflict,
    NotFound,
    Collector,
    Persistence,
    Permission,
    Platform,
    Update,
    Unavailable,
    Internal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FieldError {
    field: &'static str,
    code: &'static str,
    message: &'static str,
}

impl FieldError {
    pub(super) fn new(field: &'static str, code: &'static str, message: &'static str) -> Self {
        Self {
            field,
            code,
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::{json, value::Value};

    use super::*;

    const REQUEST_ID: &str = "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901";
    const GENERATED_AT: &str = "2026-06-14T07:30:00.000Z";

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ExampleData {
        application_version: &'static str,
        last_refresh_at: Option<&'static str>,
    }

    #[test]
    fn success_matches_the_v1_fixture() {
        let response = IpcResponse::success_with_meta(
            ExampleData {
                application_version: "0.1.0",
                last_refresh_at: None,
            },
            fixed_meta(),
        );

        assert_fixture(
            &response,
            include_str!("../../../tests/fixtures/ipc/v1/response-success.json"),
        );
    }

    #[test]
    fn failure_matches_the_v1_fixture() {
        let error = IpcError::new(
            "validation.invalid_date_range",
            "The selected date range is invalid.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "dateRange.startDate",
            "validation.before_end_date",
            "Start date must not be after end date.",
        )]);
        let response = IpcResponse::<()>::failure_with_meta(error, fixed_meta());

        assert_fixture(
            &response,
            include_str!("../../../tests/fixtures/ipc/v1/response-error.json"),
        );
    }

    #[test]
    fn generated_metadata_uses_unique_uuid_request_ids_and_utc_timestamps() {
        let first = response_meta(IpcResponse::success(json!({})));
        let second = response_meta(IpcResponse::<()>::failure(IpcError::new(
            "internal.unexpected",
            "Burnly could not complete the request.",
            ErrorCategory::Internal,
            false,
        )));

        assert_eq!(first["contractVersion"], CONTRACT_VERSION);
        assert_ne!(first["requestId"], second["requestId"]);
        assert!(Uuid::parse_str(first["requestId"].as_str().expect("request id")).is_ok());
        assert!(first["generatedAt"]
            .as_str()
            .expect("generated timestamp")
            .ends_with('Z'));
    }

    #[test]
    fn all_error_categories_use_approved_wire_values() {
        let categories = [
            ErrorCategory::Validation,
            ErrorCategory::Conflict,
            ErrorCategory::NotFound,
            ErrorCategory::Collector,
            ErrorCategory::Persistence,
            ErrorCategory::Permission,
            ErrorCategory::Platform,
            ErrorCategory::Update,
            ErrorCategory::Unavailable,
            ErrorCategory::Internal,
        ];
        let values = categories
            .into_iter()
            .map(|category| serde_json::to_value(category).expect("serialize category"))
            .collect::<Vec<_>>();

        assert_eq!(
            values,
            vec![
                json!("validation"),
                json!("conflict"),
                json!("not_found"),
                json!("collector"),
                json!("persistence"),
                json!("permission"),
                json!("platform"),
                json!("update"),
                json!("unavailable"),
                json!("internal"),
            ]
        );
    }

    #[test]
    fn error_serialization_contains_only_reviewed_safe_fields() {
        let error = IpcError::new(
            "app.recovery_required",
            "Burnly could not open its local data.",
            ErrorCategory::Persistence,
            false,
        );
        let value = serde_json::to_value(IpcResponse::<()>::failure_with_meta(error, fixed_meta()))
            .expect("serialize response");
        let serialized = value.to_string();

        assert_eq!(
            value["error"],
            json!({
                "code": "app.recovery_required",
                "message": "Burnly could not open its local data.",
                "category": "persistence",
                "retryable": false,
                "details": null,
            })
        );
        for forbidden in ["SELECT", "/home/", "stack backtrace", "API_TOKEN"] {
            assert!(!serialized.contains(forbidden));
        }
    }

    fn fixed_meta() -> ResponseMeta {
        ResponseMeta::fixed(REQUEST_ID, GENERATED_AT)
    }

    fn response_meta<T: Serialize>(response: IpcResponse<T>) -> Value {
        serde_json::to_value(response).expect("serialize response")["meta"].clone()
    }

    fn assert_fixture(value: &impl Serialize, fixture: &str) {
        let actual = serde_json::to_value(value).expect("serialize fixture value");
        let expected: Value = serde_json::from_str(fixture).expect("parse fixture");
        assert_eq!(actual, expected);
    }
}
