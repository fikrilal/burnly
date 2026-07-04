mod descriptor;
mod failure;

pub(in crate::infrastructure::collectors) use descriptor::{
    collector_key, daily_session_projections, single_source_descriptor, CollectorIdentity,
};
pub(in crate::infrastructure::collectors) use failure::{
    missing_or_invalid_location_code, request_failure, validate_source,
    validation_failure_as_internal, validation_failure_preserving_all_rejected,
};
