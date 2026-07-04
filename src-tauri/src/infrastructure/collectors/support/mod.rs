mod descriptor;
mod detection;
mod failure;
mod run;

pub(in crate::infrastructure::collectors) use descriptor::{
    collector_key, daily_session_projections, single_source_descriptor, CollectorIdentity,
};
pub(in crate::infrastructure::collectors) use detection::{
    available_detection, cancelled_detection, detection_issue, invalid_configuration_detection,
    not_found_detection, unsupported_detection,
};
pub(in crate::infrastructure::collectors) use failure::{
    missing_or_invalid_location_code, request_failure, validate_source,
    validation_failure_as_internal, validation_failure_preserving_all_rejected,
};
pub(in crate::infrastructure::collectors) use run::{
    collection_metadata, empty_collection_result, LocalCollectionRun,
};
