mod descriptor;
mod detection;
mod diagnostics;
mod failure;
mod mapping;
mod run;
mod sqlite;

pub(in crate::infrastructure::collectors) use descriptor::{
    collector_key, daily_session_projections, single_source_descriptor, CollectorIdentity,
};
pub(in crate::infrastructure::collectors) use detection::{
    available_detection, cancelled_detection, detection_issue, invalid_configuration_detection,
    not_found_detection, unsupported_detection,
};
pub(in crate::infrastructure::collectors) use diagnostics::{
    record_collector_diagnostic, CollectorDiagnosticCounter,
};
pub(in crate::infrastructure::collectors) use failure::{
    missing_or_invalid_location_code, request_failure, validate_source,
    validation_failure_as_internal, validation_failure_preserving_all_rejected,
};
pub(in crate::infrastructure::collectors) use mapping::{
    checked_add_u64, date_in_scope, local_date_from_millis, provenance, utc_from_millis,
    MappingIdentity,
};
pub(in crate::infrastructure::collectors) use run::{
    collection_metadata, empty_collection_result, LocalCollectionRun,
};
pub(in crate::infrastructure::collectors) use sqlite::open_external_read_only;
