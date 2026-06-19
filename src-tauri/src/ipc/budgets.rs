use std::sync::Arc;

use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::application::budget_evaluation::BudgetCostCompleteness;
use crate::application::budget_progress::{
    BudgetProgressError, BudgetProgressItemState, BudgetProgressMetric, BudgetProgressQuery,
    BudgetProgressStatus, CurrentBudgetProgressItem, CurrentBudgetProgressReadModel,
};
use crate::application::budgets::{BudgetError, BudgetService};
use crate::domain::budget::{
    Budget, BudgetDefinition, BudgetId, BudgetLimit, BudgetPeriod, BudgetScope, BudgetThreshold,
    BudgetValidationError,
};
use crate::domain::usage::CurrencyCode;

use super::response::{ErrorCategory, FieldError, IpcError, IpcResponse};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BudgetResponse {
    id: String,
    revision: String,
    name: String,
    limit: BudgetLimitResponse,
    period: &'static str,
    scope: BudgetScopeResponse,
    enabled: bool,
    thresholds: Vec<BudgetThresholdResponse>,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum BudgetLimitResponse {
    Tokens {
        value: String,
    },
    Cost {
        amount_micros: String,
        currency: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum BudgetScopeResponse {
    Global,
    Source { source_id: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BudgetThresholdResponse {
    basis_points: u32,
    enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BudgetListResponse {
    items: Vec<BudgetResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BudgetIdRequest {
    budget_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateBudgetRequest {
    budget: BudgetDefinitionRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateBudgetRequest {
    budget_id: String,
    expected_revision: String,
    budget: BudgetDefinitionRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MutateBudgetRequest {
    budget_id: String,
    expected_revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeleteBudgetResponse {
    budget_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentBudgetProgressResponse {
    status: &'static str,
    reporting_timezone: String,
    as_of: String,
    configured_budget_count: usize,
    enabled_budget_count: usize,
    tray_summary: Option<String>,
    items: Vec<CurrentBudgetProgressItemResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentBudgetProgressItemResponse {
    budget_id: String,
    budget_name: String,
    period: &'static str,
    period_start_date: String,
    period_end_date: String,
    metric: &'static str,
    state: &'static str,
    current: Option<String>,
    limit: String,
    currency: Option<String>,
    basis_points: Option<String>,
    exceeded: bool,
    completeness: &'static str,
    unavailable_days: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BudgetDefinitionRequest {
    name: String,
    limit: BudgetLimitRequest,
    period: String,
    scope: BudgetScopeRequest,
    enabled: bool,
    thresholds: Vec<BudgetThresholdRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum BudgetLimitRequest {
    Tokens {
        value: String,
    },
    Cost {
        amount_micros: String,
        currency: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum BudgetScopeRequest {
    Global,
    Source { source_id: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BudgetThresholdRequest {
    basis_points: u32,
    enabled: bool,
}

#[tauri::command]
pub(super) fn budgets_list(service: State<'_, BudgetService>) -> IpcResponse<BudgetListResponse> {
    match service.list() {
        Ok(budgets) => IpcResponse::success(BudgetListResponse {
            items: budgets.into_iter().map(Into::into).collect(),
        }),
        Err(error) => IpcResponse::failure(budget_error(error)),
    }
}

#[tauri::command]
pub(super) fn budgets_get(
    service: State<'_, BudgetService>,
    request: BudgetIdRequest,
) -> IpcResponse<BudgetResponse> {
    let id = match parse_budget_id(&request.budget_id) {
        Ok(id) => id,
        Err(error) => return IpcResponse::failure(request_error(error)),
    };
    match service.get(id) {
        Ok(budget) => IpcResponse::success(budget.into()),
        Err(error) => IpcResponse::failure(budget_error(error)),
    }
}

#[tauri::command]
pub(super) fn budgets_create<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, BudgetService>,
    request: CreateBudgetRequest,
) -> IpcResponse<BudgetResponse> {
    let definition = match request.budget.into_domain() {
        Ok(definition) => definition,
        Err(error) => return IpcResponse::failure(request_error(error)),
    };
    match service.create(definition) {
        Ok(budget) => changed_response(&app, budget),
        Err(error) => IpcResponse::failure(budget_error(error)),
    }
}

#[tauri::command]
pub(super) fn budgets_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, BudgetService>,
    request: UpdateBudgetRequest,
) -> IpcResponse<BudgetResponse> {
    let (id, revision, definition) = match request.into_domain() {
        Ok(values) => values,
        Err(error) => return IpcResponse::failure(request_error(error)),
    };
    match service.update(id, revision, definition) {
        Ok(budget) => changed_response(&app, budget),
        Err(error) => IpcResponse::failure(budget_error(error)),
    }
}

#[tauri::command]
pub(super) fn budgets_enable<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, BudgetService>,
    request: MutateBudgetRequest,
) -> IpcResponse<BudgetResponse> {
    mutate_budget(&app, &service, request, BudgetService::enable)
}

#[tauri::command]
pub(super) fn budgets_disable<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, BudgetService>,
    request: MutateBudgetRequest,
) -> IpcResponse<BudgetResponse> {
    mutate_budget(&app, &service, request, BudgetService::disable)
}

#[tauri::command]
pub(super) fn budgets_delete<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, BudgetService>,
    request: MutateBudgetRequest,
) -> IpcResponse<DeleteBudgetResponse> {
    let (id, revision) = match request.into_domain() {
        Ok(values) => values,
        Err(error) => return IpcResponse::failure(request_error(error)),
    };
    match service.delete(id, revision) {
        Ok(()) => {
            emit_changed(&app);
            IpcResponse::success(DeleteBudgetResponse {
                budget_id: id.value().to_string(),
            })
        }
        Err(error) => IpcResponse::failure(budget_error(error)),
    }
}

#[tauri::command]
pub(super) fn budgets_get_progress(
    query: State<'_, Arc<BudgetProgressQuery>>,
) -> IpcResponse<CurrentBudgetProgressResponse> {
    match query.current() {
        Ok(progress) => IpcResponse::success(progress.into()),
        Err(error) => IpcResponse::failure(progress_error(error)),
    }
}

fn mutate_budget<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    service: &BudgetService,
    request: MutateBudgetRequest,
    operation: fn(&BudgetService, BudgetId, i64) -> Result<Budget, BudgetError>,
) -> IpcResponse<BudgetResponse> {
    let (id, revision) = match request.into_domain() {
        Ok(values) => values,
        Err(error) => return IpcResponse::failure(request_error(error)),
    };
    match operation(service, id, revision) {
        Ok(budget) => changed_response(app, budget),
        Err(error) => IpcResponse::failure(budget_error(error)),
    }
}

fn changed_response<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    budget: Budget,
) -> IpcResponse<BudgetResponse> {
    emit_changed(app);
    IpcResponse::success(budget.into())
}

fn emit_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let _ = app.emit(
        "burnly://v1/data-invalidated",
        serde_json::json!({ "scope": "budgets" }),
    );
}

impl From<Budget> for BudgetResponse {
    fn from(value: Budget) -> Self {
        let id = value.id().value().to_string();
        let revision = value.revision().to_string();
        let definition = value.definition();
        let limit = match definition.limit() {
            BudgetLimit::Tokens(value) => BudgetLimitResponse::Tokens {
                value: value.to_string(),
            },
            BudgetLimit::CostMicros {
                amount_micros,
                currency,
            } => BudgetLimitResponse::Cost {
                amount_micros: amount_micros.to_string(),
                currency: currency.as_str().to_owned(),
            },
        };
        let scope = match definition.scope() {
            BudgetScope::Global => BudgetScopeResponse::Global,
            BudgetScope::Source(source_id) => BudgetScopeResponse::Source {
                source_id: source_id.to_string(),
            },
        };
        Self {
            id,
            revision,
            name: definition.name().to_owned(),
            limit,
            period: definition.period().as_str(),
            scope,
            enabled: definition.enabled(),
            thresholds: definition
                .thresholds()
                .iter()
                .map(|threshold| BudgetThresholdResponse {
                    basis_points: threshold.basis_points(),
                    enabled: threshold.enabled(),
                })
                .collect(),
        }
    }
}

impl From<CurrentBudgetProgressReadModel> for CurrentBudgetProgressResponse {
    fn from(value: CurrentBudgetProgressReadModel) -> Self {
        Self {
            status: progress_status(value.status),
            reporting_timezone: value.reporting_timezone,
            as_of: value.as_of.to_rfc3339_opts(SecondsFormat::Millis, true),
            configured_budget_count: value.configured_budget_count,
            enabled_budget_count: value.enabled_budget_count,
            tray_summary: value.tray_summary,
            items: value.items.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CurrentBudgetProgressItem> for CurrentBudgetProgressItemResponse {
    fn from(value: CurrentBudgetProgressItem) -> Self {
        Self {
            budget_id: value.budget_id.to_string(),
            budget_name: value.budget_name,
            period: value.period.as_str(),
            period_start_date: value.period_start_date,
            period_end_date: value.period_end_date,
            metric: progress_metric(value.metric),
            state: progress_item_state(value.state),
            current: value.current.map(|current| current.to_string()),
            limit: value.limit.to_string(),
            currency: value.currency,
            basis_points: value
                .basis_points
                .map(|basis_points| basis_points.to_string()),
            exceeded: value.exceeded,
            completeness: cost_completeness(value.completeness),
            unavailable_days: value.unavailable_days,
        }
    }
}

const fn progress_status(status: BudgetProgressStatus) -> &'static str {
    match status {
        BudgetProgressStatus::NoBudgets => "no_budgets",
        BudgetProgressStatus::AllDisabled => "all_disabled",
        BudgetProgressStatus::Available => "available",
    }
}

const fn progress_metric(metric: BudgetProgressMetric) -> &'static str {
    match metric {
        BudgetProgressMetric::Tokens => "tokens",
        BudgetProgressMetric::Cost => "cost",
    }
}

const fn progress_item_state(state: BudgetProgressItemState) -> &'static str {
    match state {
        BudgetProgressItemState::Available => "available",
        BudgetProgressItemState::CostUnavailable => "cost_unavailable",
    }
}

const fn cost_completeness(completeness: BudgetCostCompleteness) -> &'static str {
    match completeness {
        BudgetCostCompleteness::Complete => "complete",
        BudgetCostCompleteness::Partial => "partial",
        BudgetCostCompleteness::Unavailable => "unavailable",
    }
}

impl BudgetDefinitionRequest {
    fn into_domain(self) -> Result<BudgetDefinition, RequestError> {
        let limit = match self.limit {
            BudgetLimitRequest::Tokens { value } => {
                BudgetLimit::tokens(parse_u64(&value)?).map_err(RequestError::Validation)?
            }
            BudgetLimitRequest::Cost {
                amount_micros,
                currency,
            } => BudgetLimit::cost_micros(
                parse_u64(&amount_micros)?,
                CurrencyCode::new(currency).map_err(|_| RequestError::Currency)?,
            )
            .map_err(RequestError::Validation)?,
        };
        let scope = match self.scope {
            BudgetScopeRequest::Global => BudgetScope::Global,
            BudgetScopeRequest::Source { source_id } => {
                BudgetScope::source(parse_i64(&source_id, RequestError::SourceId)?)
                    .map_err(RequestError::Validation)?
            }
        };
        let thresholds = self
            .thresholds
            .into_iter()
            .map(|threshold| {
                BudgetThreshold::new(threshold.basis_points, threshold.enabled)
                    .map_err(RequestError::Validation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        BudgetDefinition::new(
            self.name,
            limit,
            BudgetPeriod::parse(&self.period).map_err(RequestError::Validation)?,
            scope,
            self.enabled,
            thresholds,
        )
        .map_err(RequestError::Validation)
    }
}

impl UpdateBudgetRequest {
    fn into_domain(self) -> Result<(BudgetId, i64, BudgetDefinition), RequestError> {
        Ok((
            parse_budget_id(&self.budget_id)?,
            parse_revision(&self.expected_revision)?,
            self.budget.into_domain()?,
        ))
    }
}

impl MutateBudgetRequest {
    fn into_domain(self) -> Result<(BudgetId, i64), RequestError> {
        Ok((
            parse_budget_id(&self.budget_id)?,
            parse_revision(&self.expected_revision)?,
        ))
    }
}

fn parse_budget_id(value: &str) -> Result<BudgetId, RequestError> {
    BudgetId::new(parse_i64(value, RequestError::BudgetId)?).map_err(RequestError::Validation)
}

fn parse_revision(value: &str) -> Result<i64, RequestError> {
    let revision = parse_i64(value, RequestError::Revision)?;
    if revision <= 0 {
        return Err(RequestError::Validation(BudgetValidationError::Revision));
    }
    Ok(revision)
}

fn parse_i64(value: &str, error: RequestError) -> Result<i64, RequestError> {
    let parsed = value.parse::<i64>().map_err(|_| error)?;
    if parsed.to_string() != value {
        return Err(error);
    }
    Ok(parsed)
}

fn parse_u64(value: &str) -> Result<u64, RequestError> {
    let parsed = value.parse::<u64>().map_err(|_| RequestError::Limit)?;
    if parsed.to_string() != value {
        return Err(RequestError::Limit);
    }
    Ok(parsed)
}

#[derive(Debug, Clone, Copy)]
enum RequestError {
    Validation(BudgetValidationError),
    BudgetId,
    Revision,
    Limit,
    SourceId,
    Currency,
}

fn request_error(error: RequestError) -> IpcError {
    let field = match error {
        RequestError::Validation(error) => validation_field(error),
        RequestError::BudgetId => FieldError::new(
            "budgetId",
            "budgets.invalid_id",
            "Budget ID must be a positive integer string.",
        ),
        RequestError::Revision => FieldError::new(
            "expectedRevision",
            "budgets.invalid_revision",
            "Budget revision must be a positive integer string.",
        ),
        RequestError::Limit => FieldError::new(
            "budget.limit",
            "budgets.invalid_limit",
            "Budget limit must be a positive integer string.",
        ),
        RequestError::SourceId => FieldError::new(
            "budget.scope.sourceId",
            "budgets.invalid_source_id",
            "Source ID must be a positive integer string.",
        ),
        RequestError::Currency => FieldError::new(
            "budget.limit.currency",
            "budgets.invalid_currency",
            "Currency must contain three uppercase ASCII letters.",
        ),
    };
    IpcError::new(
        "budgets.validation_failed",
        "Some budget values are invalid.",
        ErrorCategory::Validation,
        false,
    )
    .with_field_errors(vec![field])
}

fn validation_field(error: BudgetValidationError) -> FieldError {
    match error {
        BudgetValidationError::BudgetId => FieldError::new(
            "budgetId",
            "budgets.invalid_id",
            "Budget ID must be positive.",
        ),
        BudgetValidationError::Name => FieldError::new(
            "budget.name",
            "budgets.invalid_name",
            "Budget name must not be empty.",
        ),
        BudgetValidationError::Limit => FieldError::new(
            "budget.limit",
            "budgets.invalid_limit",
            "Budget limit must be positive.",
        ),
        BudgetValidationError::Period => FieldError::new(
            "budget.period",
            "budgets.invalid_period",
            "Budget period must be daily, weekly, or monthly.",
        ),
        BudgetValidationError::SourceId => FieldError::new(
            "budget.scope.sourceId",
            "budgets.invalid_source_id",
            "Source ID must be positive.",
        ),
        BudgetValidationError::Threshold => FieldError::new(
            "budget.thresholds",
            "budgets.invalid_threshold",
            "Budget thresholds must be positive.",
        ),
        BudgetValidationError::DuplicateThreshold => FieldError::new(
            "budget.thresholds",
            "budgets.duplicate_threshold",
            "Budget thresholds must be unique.",
        ),
        BudgetValidationError::Revision => FieldError::new(
            "expectedRevision",
            "budgets.invalid_revision",
            "Budget revision must be positive.",
        ),
    }
}

fn budget_error(error: BudgetError) -> IpcError {
    match error {
        BudgetError::Validation(error) => request_error(RequestError::Validation(error)),
        BudgetError::NotFound => IpcError::new(
            "budgets.not_found",
            "The selected budget no longer exists.",
            ErrorCategory::NotFound,
            false,
        ),
        BudgetError::Conflict => IpcError::new(
            "budgets.revision_conflict",
            "This budget changed since it was loaded.",
            ErrorCategory::Conflict,
            true,
        ),
        BudgetError::UnknownSource => IpcError::new(
            "budgets.source_not_found",
            "The selected source no longer exists.",
            ErrorCategory::NotFound,
            false,
        ),
        BudgetError::StorageUnavailable | BudgetError::InvalidStoredValue => IpcError::new(
            "budgets.storage_unavailable",
            "Burnly could not access local budgets.",
            ErrorCategory::Persistence,
            true,
        ),
    }
}

fn progress_error(error: BudgetProgressError) -> IpcError {
    match error {
        BudgetProgressError::InvalidTimezone => IpcError::new(
            "budgets.progress_invalid_timezone",
            "Budget progress could not use the configured reporting timezone.",
            ErrorCategory::Validation,
            false,
        ),
        BudgetProgressError::InvalidTimestamp => IpcError::new(
            "budgets.progress_invalid_timestamp",
            "Budget progress could not use the current system time.",
            ErrorCategory::Internal,
            false,
        ),
        BudgetProgressError::StorageUnavailable
        | BudgetProgressError::InvalidStoredValue
        | BudgetProgressError::CurrencyMismatch => IpcError::new(
            "budgets.progress_unavailable",
            "Burnly could not calculate current budget progress.",
            ErrorCategory::Persistence,
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_uses_exact_strings_and_discriminated_variants() {
        let response = BudgetResponse::from(
            Budget::new(
                BudgetId::new(7).expect("id"),
                3,
                BudgetDefinition::new(
                    "Monthly cost",
                    BudgetLimit::cost_micros(
                        25_000_000,
                        CurrencyCode::new("USD").expect("currency"),
                    )
                    .expect("limit"),
                    BudgetPeriod::Monthly,
                    BudgetScope::source(2).expect("scope"),
                    true,
                    vec![BudgetThreshold::new(8_000, true).expect("threshold")],
                )
                .expect("definition"),
            )
            .expect("budget"),
        );

        assert_eq!(
            serde_json::to_value(response).expect("serialize"),
            serde_json::json!({
                "id": "7",
                "revision": "3",
                "name": "Monthly cost",
                "limit": {
                    "kind": "cost",
                    "amountMicros": "25000000",
                    "currency": "USD"
                },
                "period": "monthly",
                "scope": { "kind": "source", "sourceId": "2" },
                "enabled": true,
                "thresholds": [{ "basisPoints": 8000, "enabled": true }]
            })
        );
    }

    #[test]
    fn malformed_exact_values_map_to_stable_field_errors() {
        let error = request_error(RequestError::Revision);
        let value = serde_json::to_value(error).expect("serialize");

        assert_eq!(value["code"], "budgets.validation_failed");
        assert_eq!(value["fieldErrors"][0]["field"], "expectedRevision");
        assert_eq!(value["fieldErrors"][0]["code"], "budgets.invalid_revision");
    }

    #[test]
    fn application_failures_map_to_stable_categories() {
        let cases = [
            (
                BudgetError::NotFound,
                "budgets.not_found",
                "not_found",
                false,
            ),
            (
                BudgetError::Conflict,
                "budgets.revision_conflict",
                "conflict",
                true,
            ),
            (
                BudgetError::UnknownSource,
                "budgets.source_not_found",
                "not_found",
                false,
            ),
            (
                BudgetError::StorageUnavailable,
                "budgets.storage_unavailable",
                "persistence",
                true,
            ),
        ];

        for (error, code, category, retryable) in cases {
            let value = serde_json::to_value(budget_error(error)).expect("serialize");
            assert_eq!(value["code"], code);
            assert_eq!(value["category"], category);
            assert_eq!(value["retryable"], retryable);
        }
    }
}
