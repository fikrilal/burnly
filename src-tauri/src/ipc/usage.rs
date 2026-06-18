use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::application::ports::overview_store::OverviewStoreError;
use crate::application::ports::session_store::{
    SessionPageCursor, SessionPagination, SessionStoreError,
};
use crate::application::usage::{
    CalendarDayInfo, CalendarPeriod, CalendarQuery, CalendarQueryError, CalendarReadModel,
    CostCompleteness, CostValuation, DayDetailPeriod, DayDetailQuery, DayDetailQueryError,
    DayDetailReadModel, OverviewCost, OverviewDataStatus, OverviewModel, OverviewPeriod,
    OverviewQuery, OverviewQueryError, OverviewReadModel, OverviewSource, SessionQuery,
};
use crate::domain::usage::{SessionDetail, UsageSession};

use super::response::{ErrorCategory, FieldError, IpcError, IpcResponse};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UsageOverviewRequest {
    start_date: String,
    end_date: String,
    reporting_timezone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UsageOverviewResponse {
    period: UsageOverviewPeriodResponse,
    total_tokens: String,
    active_days: u32,
    cost: UsageOverviewCostResponse,
    sources: Vec<UsageOverviewSourceResponse>,
    models: Vec<UsageOverviewModelResponse>,
    as_of: String,
    last_successful_refresh_at: Option<String>,
    data_status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageOverviewPeriodResponse {
    start_date: String,
    end_date: String,
    reporting_timezone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageOverviewCostResponse {
    amount_micros: Option<String>,
    currency: Option<String>,
    valuation: &'static str,
    completeness: &'static str,
    unavailable_days: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageOverviewSourceResponse {
    source: &'static str,
    total_tokens: String,
    active_days: u32,
    cost: UsageOverviewCostResponse,
    has_partial_data: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageOverviewModelResponse {
    name: String,
    total_tokens: String,
    cost: UsageOverviewCostResponse,
}

#[tauri::command]
pub(super) fn usage_get_overview(
    request: UsageOverviewRequest,
    query: State<'_, OverviewQuery>,
) -> IpcResponse<UsageOverviewResponse> {
    let period = match period_from_request(request) {
        Ok(period) => period,
        Err(error) => return IpcResponse::failure(error),
    };

    match query.get(period) {
        Ok(overview) => match UsageOverviewResponse::try_from(overview) {
            Ok(response) => IpcResponse::success(response),
            Err(error) => IpcResponse::failure(storage_error(error)),
        },
        Err(error) => IpcResponse::failure(query_error(error)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActivityCalendarRequest {
    start_date: String,
    end_date: String,
    reporting_timezone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActivityCalendarResponse {
    days: Vec<ActivityCalendarDayResponse>,
    data_status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityCalendarDayResponse {
    date: String,
    total_tokens: String,
    active_sources: u32,
    cost: UsageOverviewCostResponse,
    has_partial_data: bool,
}

#[tauri::command]
pub(super) fn usage_get_calendar(
    request: ActivityCalendarRequest,
    query: State<'_, CalendarQuery>,
) -> IpcResponse<ActivityCalendarResponse> {
    let period = match calendar_period_from_request(request) {
        Ok(period) => period,
        Err(error) => return IpcResponse::failure(error),
    };

    match query.get(period) {
        Ok(model) => IpcResponse::success(ActivityCalendarResponse::from(model)),
        Err(error) => IpcResponse::failure(calendar_query_error(error)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DayDetailRequest {
    date: String,
    reporting_timezone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DayDetailModelResponse {
    source: &'static str,
    model: String,
    tokens: String,
    cost: UsageOverviewCostResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DayDetailResponse {
    date: String,
    total_tokens: String,
    cost: UsageOverviewCostResponse,
    models: Vec<DayDetailModelResponse>,
    as_of: String,
}

#[tauri::command]
pub(super) fn usage_get_day_detail(
    request: DayDetailRequest,
    query: State<'_, DayDetailQuery>,
) -> IpcResponse<DayDetailResponse> {
    let date = match parse_date(&request.date, "request.date") {
        Ok(date) => date,
        Err(error) => return IpcResponse::failure(error),
    };
    let period = match DayDetailPeriod::new(date, request.reporting_timezone) {
        Ok(period) => period,
        Err(error) => return IpcResponse::failure(day_detail_query_error(error)),
    };

    match query.get(period) {
        Ok(model) => match DayDetailResponse::try_from(model) {
            Ok(response) => IpcResponse::success(response),
            Err(error) => IpcResponse::failure(error),
        },
        Err(error) => IpcResponse::failure(day_detail_query_error(error)),
    }
}

fn period_from_request(request: UsageOverviewRequest) -> Result<OverviewPeriod, IpcError> {
    let start_date = parse_date(&request.start_date, "request.startDate")?;
    let end_date = parse_date(&request.end_date, "request.endDate")?;

    OverviewPeriod::new(start_date, end_date, request.reporting_timezone).map_err(query_error)
}

fn calendar_period_from_request(
    request: ActivityCalendarRequest,
) -> Result<CalendarPeriod, IpcError> {
    let start_date = parse_date(&request.start_date, "request.startDate")?;
    let end_date = parse_date(&request.end_date, "request.endDate")?;

    CalendarPeriod::new(start_date, end_date, request.reporting_timezone)
        .map_err(calendar_query_error)
}

fn parse_date(value: &str, field: &'static str) -> Result<NaiveDate, IpcError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        IpcError::new(
            "validation.invalid_date",
            "The selected date is invalid.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            field,
            "validation.date_format",
            "Date must use YYYY-MM-DD format.",
        )])
    })
}

fn query_error(error: OverviewQueryError) -> IpcError {
    match error {
        OverviewQueryError::InvalidPeriod => IpcError::new(
            "validation.invalid_date_range",
            "The selected date range is invalid.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "request.startDate",
            "validation.before_end_date",
            "Start date must not be after end date.",
        )]),
        OverviewQueryError::EmptyAggregationTimezone => IpcError::new(
            "validation.empty_reporting_timezone",
            "A reporting timezone is required.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "request.reportingTimezone",
            "validation.required",
            "Reporting timezone is required.",
        )]),
        OverviewQueryError::Storage(storage) => storage_error(storage),
    }
}

fn storage_error(error: OverviewStoreError) -> IpcError {
    match error {
        OverviewStoreError::Backend => IpcError::new(
            "usage.overview_unavailable",
            "Burnly could not read local usage data.",
            ErrorCategory::Persistence,
            true,
        ),
        OverviewStoreError::ValueOutOfRange | OverviewStoreError::MixedCurrencies => IpcError::new(
            "usage.overview_inconsistent",
            "Burnly found inconsistent local usage data.",
            ErrorCategory::Persistence,
            false,
        ),
    }
}

fn calendar_query_error(error: CalendarQueryError) -> IpcError {
    match error {
        CalendarQueryError::InvalidPeriod => IpcError::new(
            "validation.invalid_date_range",
            "The selected date range is invalid.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "request.startDate",
            "validation.before_end_date",
            "Start date must not be after end date.",
        )]),
        CalendarQueryError::EmptyAggregationTimezone => IpcError::new(
            "validation.empty_reporting_timezone",
            "A reporting timezone is required.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "request.reportingTimezone",
            "validation.required",
            "Reporting timezone is required.",
        )]),
        CalendarQueryError::Storage(_) => IpcError::new(
            "usage.calendar_unavailable",
            "Burnly could not read local calendar data.",
            ErrorCategory::Persistence,
            true,
        ),
    }
}

fn day_detail_query_error(error: DayDetailQueryError) -> IpcError {
    match error {
        DayDetailQueryError::EmptyAggregationTimezone => IpcError::new(
            "validation.empty_reporting_timezone",
            "A reporting timezone is required.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "request.reportingTimezone",
            "validation.required",
            "Reporting timezone is required.",
        )]),
        DayDetailQueryError::Storage(_) => IpcError::new(
            "usage.day_detail_unavailable",
            "Burnly could not read local day detail data.",
            ErrorCategory::Persistence,
            true,
        ),
    }
}

impl TryFrom<OverviewReadModel> for UsageOverviewResponse {
    type Error = OverviewStoreError;

    fn try_from(value: OverviewReadModel) -> Result<Self, Self::Error> {
        Ok(Self {
            period: UsageOverviewPeriodResponse {
                start_date: value.period.start_date().to_string(),
                end_date: value.period.end_date().to_string(),
                reporting_timezone: value.period.aggregation_timezone().to_owned(),
            },
            total_tokens: value.total_tokens.to_string(),
            active_days: value.active_days,
            cost: value.cost.into(),
            sources: value.sources.into_iter().map(Into::into).collect(),
            models: value.models.into_iter().map(Into::into).collect(),
            as_of: to_rfc3339(value.as_of_ms)?,
            last_successful_refresh_at: value
                .last_successful_refresh_at_ms
                .map(to_rfc3339)
                .transpose()?,
            data_status: data_status(value.data_status),
        })
    }
}

impl From<OverviewCost> for UsageOverviewCostResponse {
    fn from(value: OverviewCost) -> Self {
        Self {
            amount_micros: value.amount_micros.map(|amount| amount.to_string()),
            currency: value.currency.map(|currency| currency.as_str().to_owned()),
            valuation: cost_valuation(value.valuation),
            completeness: cost_completeness(value.completeness),
            unavailable_days: value.unavailable_days,
        }
    }
}

impl From<OverviewSource> for UsageOverviewSourceResponse {
    fn from(value: OverviewSource) -> Self {
        Self {
            source: value.source.as_str(),
            total_tokens: value.total_tokens.to_string(),
            active_days: value.active_days,
            cost: value.cost.into(),
            has_partial_data: value.has_partial_data,
        }
    }
}

impl From<OverviewModel> for UsageOverviewModelResponse {
    fn from(value: OverviewModel) -> Self {
        Self {
            name: value.name,
            total_tokens: value.total_tokens.to_string(),
            cost: value.cost.into(),
        }
    }
}

const fn data_status(value: OverviewDataStatus) -> &'static str {
    match value {
        OverviewDataStatus::Current => "current",
        OverviewDataStatus::Stale => "stale",
        OverviewDataStatus::Partial => "partial",
        OverviewDataStatus::Empty => "empty",
    }
}

const fn cost_valuation(value: CostValuation) -> &'static str {
    match value {
        CostValuation::Available => "available",
        CostValuation::Estimated => "estimated",
        CostValuation::Unavailable => "unavailable",
    }
}

const fn cost_completeness(value: CostCompleteness) -> &'static str {
    match value {
        CostCompleteness::Complete => "complete",
        CostCompleteness::Partial => "partial",
        CostCompleteness::Unavailable => "unavailable",
    }
}

fn to_rfc3339(epoch_ms: i64) -> Result<String, OverviewStoreError> {
    DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
        .ok_or(OverviewStoreError::ValueOutOfRange)
}

impl From<CalendarReadModel> for ActivityCalendarResponse {
    fn from(value: CalendarReadModel) -> Self {
        Self {
            days: value.days.into_iter().map(Into::into).collect(),
            data_status: data_status(value.data_status),
        }
    }
}

impl From<CalendarDayInfo> for ActivityCalendarDayResponse {
    fn from(value: CalendarDayInfo) -> Self {
        Self {
            date: value.date.to_string(),
            total_tokens: value.total_tokens.to_string(),
            active_sources: value.active_sources,
            cost: value.cost.into(),
            has_partial_data: value.has_partial_data,
        }
    }
}

impl TryFrom<DayDetailReadModel> for DayDetailResponse {
    type Error = IpcError;

    fn try_from(value: DayDetailReadModel) -> Result<Self, Self::Error> {
        let mut models = Vec::with_capacity(value.models.len());
        for m in value.models {
            models.push(DayDetailModelResponse {
                source: m.source.as_str(),
                model: m.model,
                tokens: m.tokens.to_string(),
                cost: m.cost.into(),
            });
        }

        Ok(Self {
            date: value.date.to_string(),
            total_tokens: value.total_tokens.to_string(),
            cost: value.cost.into(),
            models,
            as_of: to_rfc3339(value.as_of_ms).map_err(storage_error)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{to_value, Value};

    use super::*;

    #[test]
    fn query_failures_map_to_stable_user_safe_errors() {
        let validation = to_value(IpcResponse::<()>::failure(query_error(
            OverviewQueryError::InvalidPeriod,
        )))
        .expect("serialize validation error");
        let persistence = to_value(IpcResponse::<()>::failure(storage_error(
            OverviewStoreError::Backend,
        )))
        .expect("serialize persistence error");

        assert_eq!(validation["error"]["code"], "validation.invalid_date_range");
        assert_eq!(validation["error"]["category"], "validation");
        assert_eq!(validation["error"]["retryable"], false);
        assert_eq!(
            validation["error"]["fieldErrors"][0]["field"],
            "request.startDate"
        );
        assert_eq!(persistence["error"]["code"], "usage.overview_unavailable");
        assert_eq!(persistence["error"]["category"], "persistence");
        assert_eq!(persistence["error"]["retryable"], true);
        assert_eq!(persistence["error"]["details"], Value::Null);
    }

    #[test]
    fn invalid_timestamps_fail_instead_of_serializing_empty_values() {
        assert_eq!(
            to_rfc3339(i64::MAX),
            Err(OverviewStoreError::ValueOutOfRange)
        );
    }

    #[test]
    fn session_item_response_uses_opaque_ids_and_hides_local_details() {
        let session = session_fixture(
            42,
            Some(2_000),
            Some("/home/user/private-project".to_owned()),
        );

        let response = SessionItemResponse::try_from(session).expect("session response");

        assert_eq!(response.id, "session-v1-16");
        assert_eq!(response.source_id, "source-v1-7");
        assert_eq!(response.label, "Session session-v1-16");
        assert_eq!(response.project_path, None);
    }

    #[test]
    fn session_cursor_round_trips_activity_and_tiebreaker() {
        let session = session_fixture(35, Some(1_296), None);

        let cursor = encode_session_cursor(&session).expect("encode cursor");
        let parsed = parse_session_cursor(&cursor).expect("parse cursor");

        assert_eq!(
            parsed,
            SessionPageCursor {
                last_activity_at_ms: Some(1_296),
                session_id: 35,
            }
        );
    }

    fn session_fixture(
        session_id: i64,
        last_activity_at_ms: Option<i64>,
        project_path: Option<String>,
    ) -> UsageSession {
        UsageSession {
            session_id,
            source_id: 7,
            source_session_id: "raw-collector-session".to_owned(),
            project_id: project_path.as_ref().map(|_| 9),
            project_path,
            first_activity_at_ms: Some(1_000),
            last_activity_at_ms,
            tokens: crate::domain::usage::TokenUsage::new(None, None, None, None, 10)
                .expect("tokens"),
            cost: crate::domain::usage::UsageCost::Unavailable {
                kind: crate::domain::usage::CostKind::CollectorCalculated,
            },
        }
    }
}

const SESSION_ID_PREFIX: &str = "session-v1-";
const SOURCE_ID_PREFIX: &str = "source-v1-";
const SESSION_CURSOR_PREFIX: &str = "session-page-v1-";

fn encode_prefixed_id(prefix: &str, value: i64) -> Result<String, IpcError> {
    Ok(format!("{prefix}{}", encode_base36(value)?))
}

fn parse_optional_prefixed_id(
    value: Option<&str>,
    prefix: &str,
    field: &'static str,
) -> Result<Option<i64>, IpcError> {
    value
        .map(|current| parse_prefixed_id(current, prefix, field))
        .transpose()
}

fn parse_prefixed_id(value: &str, prefix: &str, field: &'static str) -> Result<i64, IpcError> {
    let encoded = value
        .strip_prefix(prefix)
        .ok_or_else(|| opaque_id_error(field))?;
    decode_base36(encoded).ok_or_else(|| opaque_id_error(field))
}

fn encode_session_cursor(session: &UsageSession) -> Result<String, IpcError> {
    let activity = session
        .last_activity_at_ms
        .map(encode_base36)
        .transpose()?
        .unwrap_or_else(|| "none".to_owned());
    let session_id = encode_base36(session.session_id)?;
    Ok(format!("{SESSION_CURSOR_PREFIX}{activity}-{session_id}"))
}

fn parse_optional_session_cursor(
    value: Option<&str>,
) -> Result<Option<SessionPageCursor>, IpcError> {
    value.map(parse_session_cursor).transpose()
}

fn parse_session_cursor(value: &str) -> Result<SessionPageCursor, IpcError> {
    let encoded = value
        .strip_prefix(SESSION_CURSOR_PREFIX)
        .ok_or_else(|| opaque_id_error("request.afterCursor"))?;
    let (activity, session_id) = encoded
        .rsplit_once('-')
        .ok_or_else(|| opaque_id_error("request.afterCursor"))?;
    let last_activity_at_ms = if activity == "none" {
        None
    } else {
        Some(decode_base36(activity).ok_or_else(|| opaque_id_error("request.afterCursor"))?)
    };
    let session_id =
        decode_base36(session_id).ok_or_else(|| opaque_id_error("request.afterCursor"))?;

    Ok(SessionPageCursor {
        last_activity_at_ms,
        session_id,
    })
}

fn encode_base36(value: i64) -> Result<String, IpcError> {
    let mut value = u64::try_from(value).map_err(|_| opaque_id_error("response.id"))?;
    if value == 0 {
        return Ok("0".to_owned());
    }

    let mut chars = Vec::new();
    while value > 0 {
        let digit = (value % 36) as u8;
        let byte = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + (digit - 10)
        };
        chars.push(byte as char);
        value /= 36;
    }
    chars.reverse();
    Ok(chars.into_iter().collect())
}

fn decode_base36(value: &str) -> Option<i64> {
    if value.is_empty() {
        return None;
    }
    let mut result = 0_u64;
    for byte in value.bytes() {
        let digit = match byte {
            b'0'..=b'9' => u64::from(byte - b'0'),
            b'a'..=b'z' => u64::from(byte - b'a' + 10),
            _ => return None,
        };
        result = result.checked_mul(36)?.checked_add(digit)?;
    }
    i64::try_from(result).ok()
}

fn opaque_id_error(field: &'static str) -> IpcError {
    IpcError::new(
        "validation.invalid_session_cursor",
        "The selected session reference is invalid.",
        ErrorCategory::Validation,
        false,
    )
    .with_field_errors(vec![FieldError::new(
        field,
        "validation.invalid_opaque_id",
        "Session references must come from Burnly.",
    )])
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionListRequest {
    source_id: Option<String>,
    limit: u32,
    after_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionListResponse {
    items: Vec<SessionItemResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionItemResponse {
    id: String,
    source_id: String,
    label: String,
    project_path: Option<String>,
    first_activity_at: Option<String>,
    last_activity_at: Option<String>,
    total_tokens: String,
    cost: UsageOverviewCostResponse,
}

#[tauri::command]
pub(super) fn usage_get_sessions(
    request: SessionListRequest,
    query: State<'_, SessionQuery>,
) -> IpcResponse<SessionListResponse> {
    let source_id = match parse_optional_prefixed_id(
        request.source_id.as_deref(),
        SOURCE_ID_PREFIX,
        "request.sourceId",
    ) {
        Ok(source_id) => source_id,
        Err(error) => return IpcResponse::failure(error),
    };
    let after = match parse_optional_session_cursor(request.after_cursor.as_deref()) {
        Ok(after) => after,
        Err(error) => return IpcResponse::failure(error),
    };
    let limit = std::cmp::min(request.limit, 100);
    let pagination = SessionPagination {
        limit: limit + 1, // request 1 more to check if there is a next page
        after,
    };

    match query.get_sessions(source_id, pagination) {
        Ok(mut sessions) => {
            let next_cursor = if sessions.len() > limit as usize {
                let last = sessions.pop().expect("has extra");
                match encode_session_cursor(&last) {
                    Ok(cursor) => Some(cursor),
                    Err(error) => return IpcResponse::failure(error),
                }
            } else {
                None
            };

            let items = match sessions
                .into_iter()
                .map(SessionItemResponse::try_from)
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(items) => items,
                Err(error) => return IpcResponse::failure(error),
            };
            IpcResponse::success(SessionListResponse { items, next_cursor })
        }
        Err(SessionStoreError::Backend) => IpcResponse::failure(IpcError::new(
            "usage.sessions_unavailable",
            "Burnly could not read local sessions data.",
            ErrorCategory::Persistence,
            true,
        )),
        Err(SessionStoreError::NotFound) => IpcResponse::success(SessionListResponse {
            items: vec![],
            next_cursor: None,
        }),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionDetailRequest {
    session_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionDetailResponse {
    session: SessionItemResponse,
    models: Vec<SessionModelUsageResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionModelUsageResponse {
    raw_model_id: Option<String>,
    total_tokens: String,
    cost: UsageOverviewCostResponse,
}

#[tauri::command]
pub(super) fn usage_get_session_detail(
    request: SessionDetailRequest,
    query: State<'_, SessionQuery>,
) -> IpcResponse<Option<SessionDetailResponse>> {
    let session_id =
        match parse_prefixed_id(&request.session_id, SESSION_ID_PREFIX, "request.sessionId") {
            Ok(session_id) => session_id,
            Err(error) => return IpcResponse::failure(error),
        };

    match query.get_session_detail(session_id) {
        Ok(detail) => match SessionDetailResponse::try_from(detail) {
            Ok(response) => IpcResponse::success(Some(response)),
            Err(error) => IpcResponse::failure(error),
        },
        Err(SessionStoreError::NotFound) => IpcResponse::success(None),
        Err(SessionStoreError::Backend) => IpcResponse::failure(IpcError::new(
            "usage.session_detail_unavailable",
            "Burnly could not read local session detail data.",
            ErrorCategory::Persistence,
            true,
        )),
    }
}

impl TryFrom<UsageSession> for SessionItemResponse {
    type Error = IpcError;

    fn try_from(value: UsageSession) -> Result<Self, Self::Error> {
        let id = encode_prefixed_id(SESSION_ID_PREFIX, value.session_id)?;
        let source_id = encode_prefixed_id(SOURCE_ID_PREFIX, value.source_id)?;

        Ok(Self {
            label: format!("Session {id}"),
            id,
            source_id,
            project_path: None,
            first_activity_at: value
                .first_activity_at_ms
                .map(|ms| to_rfc3339(ms).unwrap_or_default()),
            last_activity_at: value
                .last_activity_at_ms
                .map(|ms| to_rfc3339(ms).unwrap_or_default()),
            total_tokens: value.tokens.total_tokens().to_string(),
            cost: UsageOverviewCostResponse::from_usage_cost(&value.cost),
        })
    }
}

impl TryFrom<SessionDetail> for SessionDetailResponse {
    type Error = IpcError;

    fn try_from(value: SessionDetail) -> Result<Self, Self::Error> {
        Ok(Self {
            session: SessionItemResponse::try_from(value.session)?,
            models: value
                .model_breakdowns
                .into_iter()
                .map(|m| SessionModelUsageResponse {
                    raw_model_id: m.raw_model_id,
                    total_tokens: m.tokens.total_tokens().to_string(),
                    cost: UsageOverviewCostResponse::from_usage_cost(&m.cost),
                })
                .collect(),
        })
    }
}

impl UsageOverviewCostResponse {
    fn from_usage_cost(cost: &crate::domain::usage::UsageCost) -> Self {
        use crate::domain::usage::UsageCost;
        match cost {
            UsageCost::Valued {
                amount_micros,
                currency,
                status,
                ..
            } => Self {
                amount_micros: Some(amount_micros.to_string()),
                currency: Some(currency.as_str().to_owned()),
                valuation: match status {
                    crate::domain::usage::ValuedCostStatus::Available => "available",
                    crate::domain::usage::ValuedCostStatus::Estimated => "estimated",
                },
                completeness: "complete", // For models/sessions, we assume complete if valued.
                unavailable_days: 0,
            },
            UsageCost::NotApplicable { .. } => Self {
                amount_micros: None,
                currency: None,
                valuation: "unavailable",
                completeness: "complete",
                unavailable_days: 0,
            },
            UsageCost::Unavailable { .. } => Self {
                amount_micros: None,
                currency: None,
                valuation: "unavailable",
                completeness: "unavailable",
                unavailable_days: 1,
            },
        }
    }
}
