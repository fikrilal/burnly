#![expect(
    dead_code,
    reason = "Phase 3A defines collector contracts consumed by later Phase 3 adapters"
)]

mod candidate;
mod descriptor;
mod failure;
mod request;
mod result;

#[allow(
    unused_imports,
    reason = "collection is the intentional application contract facade"
)]
pub(crate) use candidate::{
    CandidateProvenance, CandidateWarning, DailyUsageCandidate, ModelUsageCandidate,
};
#[allow(
    unused_imports,
    reason = "collection is the intentional application contract facade"
)]
pub(crate) use descriptor::{
    CollectorDescriptor, CollectorIntegrity, CollectorKey, ProfileDescriptor,
};
#[allow(
    unused_imports,
    reason = "collection is the intentional application contract facade"
)]
pub(crate) use failure::{
    CollectorFailure, CollectorFailureCategory, CollectorFailureCode, CollectorFailureContext,
};
#[allow(
    unused_imports,
    reason = "collection is the intentional application contract facade"
)]
pub(crate) use request::{
    CollectionId, CollectionProjection, CollectionRequest, CollectionScope, CollectionSettings,
    DetectionReason, DetectionRequest, IncrementalScope,
};
#[allow(
    unused_imports,
    reason = "collection is the intentional application contract facade"
)]
pub(crate) use result::{
    CollectionMetadata, CollectionOutcome, CollectionPeriod, CollectionResult, CollectionWarning,
    DetectionIssue, DetectionResult, DetectionState, ProcessSummary, RejectedRecord,
    ResultValidationError,
};
