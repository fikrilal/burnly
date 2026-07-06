use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::application::collection::{
    CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionRequest,
    CollectionResult, CollectorDescriptor, CollectorFailure, CollectorFailureCode,
    CollectorIntegrity, CollectorKey, DetectionIssue, DetectionRequest, DetectionResult,
    DetectionState, ProcessSummary, ProfileDescriptor,
};
use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;

#[cfg(test)]
use super::discovery::{LocalListener, ProcessSnapshot};
use super::mapper::{self, AntigravityMappingContext, ConversationUsage};
use super::{
    extract_usage_records, ConversationDatabase, ConversationIndex, RuntimeClient,
    RuntimeDiscovery, RuntimeDiscoveryReport, RuntimeEndpoint,
};

const COLLECTOR_KEY: &str = "antigravity";
const DISPLAY_NAME: &str = "Antigravity";
const COLLECTOR_VERSION: &str = "local-rpc";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;

#[derive(Clone)]
pub(crate) struct AntigravityCollector {
    conversation_index: ConversationIndex,
    runtime_discovery: RuntimeDiscoverySource,
    endpoint_validation: EndpointValidationSource,
    runtime_usage: RuntimeUsageSource,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
}

impl AntigravityCollector {
    pub(crate) fn new() -> Self {
        let runtime_client = RuntimeClient::new();
        Self {
            conversation_index: ConversationIndex::default(),
            runtime_discovery: RuntimeDiscoverySource::Current,
            endpoint_validation: EndpointValidationSource::Current(runtime_client.clone()),
            runtime_usage: RuntimeUsageSource::Current(runtime_client),
            diagnostics: None,
        }
    }

    pub(crate) fn with_diagnostic_recorder(diagnostics: Arc<dyn DiagnosticRecorder>) -> Self {
        Self {
            diagnostics: Some(diagnostics),
            ..Self::new()
        }
    }

    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "bootstrap tests use this helper in cfg-specific builds"
    )]
    pub(crate) fn empty_from_data_root(data_root: impl Into<std::path::PathBuf>) -> Self {
        Self::from_parts(
            ConversationIndex::from_data_root(data_root),
            RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
                10,
                Some(std::path::PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::ipv4(34415)],
            )]),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(Vec::new()),
        )
    }

    #[cfg(test)]
    fn from_parts(
        conversation_index: ConversationIndex,
        runtime_discovery: RuntimeDiscovery,
        endpoint_validation: EndpointValidationSource,
        runtime_usage: RuntimeUsageSource,
    ) -> Self {
        Self {
            conversation_index,
            runtime_discovery: RuntimeDiscoverySource::Fixed(runtime_discovery),
            endpoint_validation,
            runtime_usage,
            diagnostics: None,
        }
    }

    #[cfg(test)]
    fn with_test_diagnostics(mut self, diagnostics: Arc<dyn DiagnosticRecorder>) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    #[cfg(test)]
    fn with_endpoint_validation(mut self, endpoint_validation: EndpointValidationSource) -> Self {
        self.endpoint_validation = endpoint_validation;
        self
    }
}

#[derive(Debug, Clone)]
enum RuntimeDiscoverySource {
    Current,
    #[cfg(test)]
    Fixed(RuntimeDiscovery),
}

impl RuntimeDiscoverySource {
    fn discover(&self) -> RuntimeDiscoveryReport {
        match self {
            Self::Current => RuntimeDiscovery::current().discover_report(),
            #[cfg(test)]
            Self::Fixed(discovery) => discovery.discover_report(),
        }
    }
}

#[derive(Debug, Clone)]
enum EndpointValidationSource {
    Current(RuntimeClient),
    #[cfg(test)]
    Passthrough,
    #[cfg(test)]
    RejectAll,
}

impl EndpointValidationSource {
    fn validate(&self, candidates: &[RuntimeEndpoint]) -> EndpointValidationReport {
        match self {
            Self::Current(client) => validate_runtime_endpoints(client, candidates),
            #[cfg(test)]
            Self::Passthrough => EndpointValidationReport {
                endpoints: candidates.to_vec(),
                identity_probes_attempted: candidates.len().try_into().unwrap_or(u32::MAX),
                identity_probes_succeeded: candidates.len().try_into().unwrap_or(u32::MAX),
            },
            #[cfg(test)]
            Self::RejectAll => EndpointValidationReport {
                endpoints: Vec::new(),
                identity_probes_attempted: candidates.len().try_into().unwrap_or(u32::MAX),
                identity_probes_succeeded: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
struct EndpointValidationReport {
    endpoints: Vec<RuntimeEndpoint>,
    identity_probes_attempted: u32,
    identity_probes_succeeded: u32,
}

fn validate_runtime_endpoints(
    client: &RuntimeClient,
    candidates: &[RuntimeEndpoint],
) -> EndpointValidationReport {
    let mut endpoints = Vec::new();
    let mut identity_probes_attempted = 0_u32;
    let mut identity_probes_succeeded = 0_u32;
    for candidate in candidates {
        identity_probes_attempted = identity_probes_attempted.saturating_add(1);
        if client.probe_identity(candidate).is_ok() {
            identity_probes_succeeded = identity_probes_succeeded.saturating_add(1);
            endpoints.push(candidate.clone());
        }
    }
    EndpointValidationReport {
        endpoints,
        identity_probes_attempted,
        identity_probes_succeeded,
    }
}

#[derive(Debug, Clone)]
enum RuntimeUsageSource {
    Current(RuntimeClient),
    #[cfg(test)]
    Fixed(Vec<ConversationUsage>),
    #[cfg(test)]
    Failing(AntigravityRuntimeCollectionFailureReason),
}

impl RuntimeUsageSource {
    fn collect(
        &self,
        endpoints: &[RuntimeEndpoint],
        conversations: &[ConversationDatabase],
    ) -> Result<RuntimeUsageReport, AntigravityRuntimeCollectionFailure> {
        match self {
            Self::Current(client) => collect_runtime_usage(client, endpoints, conversations),
            #[cfg(test)]
            Self::Fixed(usage) => Ok(RuntimeUsageReport::from_usage(usage.clone())),
            #[cfg(test)]
            Self::Failing(reason) => Err(AntigravityRuntimeCollectionFailure {
                reason: *reason,
                report: RuntimeUsageReport::default(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct AntigravityRuntimeCollectionFailure {
    reason: AntigravityRuntimeCollectionFailureReason,
    report: RuntimeUsageReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AntigravityRuntimeCollectionFailureReason {
    RuntimeNotFound,
    RuntimeIdentityProbeFailed,
    NoMatchingRuntimeEndpoint,
    RuntimeStreamUnavailable,
}

impl AntigravityRuntimeCollectionFailureReason {
    const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::RuntimeNotFound => "antigravity.runtime_not_found",
            Self::RuntimeIdentityProbeFailed => "antigravity.runtime_identity_probe_failed",
            Self::NoMatchingRuntimeEndpoint => "antigravity.runtime_endpoint_mismatch",
            Self::RuntimeStreamUnavailable => "antigravity.runtime_stream_unavailable",
        }
    }

    const fn summary(self) -> &'static str {
        match self {
            Self::RuntimeNotFound => "Antigravity local runtime endpoint was not found.",
            Self::RuntimeIdentityProbeFailed => {
                "Antigravity process listeners were found, but no language-server endpoint passed identity validation."
            }
            Self::NoMatchingRuntimeEndpoint => {
                "Antigravity conversation artifacts were found, but no matching runtime endpoint was available."
            }
            Self::RuntimeStreamUnavailable => {
                "Antigravity runtime endpoints were found, but usage streams could not be read."
            }
        }
    }

    const fn failure_reason(self) -> &'static str {
        match self {
            Self::RuntimeNotFound => "runtime_not_found",
            Self::RuntimeIdentityProbeFailed => "runtime_identity_probe_failed",
            Self::NoMatchingRuntimeEndpoint => "no_matching_runtime_endpoint",
            Self::RuntimeStreamUnavailable => "runtime_stream_unavailable",
        }
    }

    const fn collector_failure_code(self) -> CollectorFailureCode {
        match self {
            Self::RuntimeNotFound
            | Self::RuntimeIdentityProbeFailed
            | Self::NoMatchingRuntimeEndpoint
            | Self::RuntimeStreamUnavailable => CollectorFailureCode::SourceNotFound,
        }
    }
}

impl Collector for AntigravityCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        descriptor()
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        if cancellation.is_cancelled() {
            return Ok(DetectionResult {
                source: request.source,
                state: DetectionState::Cancelled,
                supported_projections: Vec::new(),
                data_roots_found: 0,
                usage_artifacts_found: false,
                checked_at: request.requested_at,
                issues: Vec::new(),
            });
        }
        if request.source != SourceKey::Antigravity {
            return Ok(DetectionResult {
                source: request.source,
                state: DetectionState::Unsupported,
                supported_projections: Vec::new(),
                data_roots_found: 0,
                usage_artifacts_found: false,
                checked_at: request.requested_at,
                issues: vec![issue(
                    "antigravity.unsupported_source",
                    "Source is not Antigravity.",
                )],
            });
        }

        let discovery = self.runtime_discovery.discover();
        let validation = self
            .endpoint_validation
            .validate(&discovery.endpoints);
        let endpoints = validation.endpoints;
        let conversations = self
            .conversation_index
            .list(
                &crate::application::collection::CollectionScope::Full,
                "UTC",
            )
            .unwrap_or_default();
        let data_roots_found = conversations
            .iter()
            .map(|conversation| conversation.variant)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            .try_into()
            .unwrap_or(u16::MAX);
        let state = if !endpoints.is_empty() {
            DetectionState::Available
        } else if !conversations.is_empty() {
            DetectionState::AvailableNoData
        } else {
            DetectionState::NotFound
        };
        let issues = if endpoints.is_empty() {
            vec![issue(
                if discovery.endpoints.is_empty() {
                    "antigravity.runtime_not_found"
                } else {
                    "antigravity.runtime_identity_probe_failed"
                },
                if discovery.endpoints.is_empty() {
                    "Antigravity local runtime endpoint was not found."
                } else {
                    "Antigravity process listeners were found, but no language-server endpoint passed identity validation."
                },
            )]
        } else {
            Vec::new()
        };

        Ok(DetectionResult {
            source: SourceKey::Antigravity,
            state,
            supported_projections: supported_projections(),
            data_roots_found,
            usage_artifacts_found: !conversations.is_empty(),
            checked_at: request.requested_at,
            issues,
        })
    }

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        let started = Instant::now();
        let started_at = Utc::now();
        validate_request(&request)?;
        if cancellation.is_cancelled() {
            return Err(failure(&request, CollectorFailureCode::Cancelled));
        }

        let discovery = self.runtime_discovery.discover();
        let validation = self
            .endpoint_validation
            .validate(&discovery.endpoints);
        let mut diagnostics = AntigravityDiagnosticCounters {
            process_candidates_found: discovery.process_candidates_found,
            endpoints_found: discovery.endpoints.len(),
            endpoints_accepted: validation.endpoints.len(),
            identity_probes_attempted: validation.identity_probes_attempted,
            identity_probes_succeeded: validation.identity_probes_succeeded,
            ..AntigravityDiagnosticCounters::default()
        };
        let endpoints = validation.endpoints;
        if endpoints.is_empty() {
            let reason = if discovery.endpoints.is_empty() {
                AntigravityRuntimeCollectionFailureReason::RuntimeNotFound
            } else {
                AntigravityRuntimeCollectionFailureReason::RuntimeIdentityProbeFailed
            };
            let failure_code = reason.collector_failure_code();
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Warning,
                    code: reason.diagnostic_code(),
                    summary: reason.summary(),
                    counters: &diagnostics,
                    failure_code: Some(failure_code.code()),
                    failure_reason: Some(reason.failure_reason()),
                },
            );
            return Err(failure(&request, failure_code));
        }
        let conversations = self
            .conversation_index
            .list(
                request.scope(),
                request.aggregation_timezone().unwrap_or("UTC"),
            )
            .map_err(|_| {
                self.record_diagnostic(
                    &request,
                    AntigravityDiagnosticInput {
                        severity: DiagnosticSeverity::Warning,
                        code: "antigravity.collection_failed",
                        summary: "Antigravity collection failed.",
                        counters: &diagnostics,
                        failure_code: Some(CollectorFailureCode::ScopeNotRepresentable.code()),
                        failure_reason: Some("scope_not_representable"),
                    },
                );
                failure(&request, CollectorFailureCode::ScopeNotRepresentable)
            })?;
        let conversations = bounded_conversations(conversations);
        diagnostics.sqlite_dbs_scanned = conversations.len();
        diagnostics.conversations_found = conversations.len();
        if conversations.is_empty() {
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Info,
                    code: "antigravity.collection_empty",
                    summary: "Antigravity collection found no conversation artifacts.",
                    counters: &diagnostics,
                    failure_code: None,
                    failure_reason: None,
                },
            );
            return empty_result(&request, started, started_at);
        }
        let report = match self.runtime_usage.collect(&endpoints, &conversations) {
            Ok(report) => report,
            Err(error) => {
                diagnostics.stream_calls_attempted = error.report.stream_calls_attempted;
                diagnostics.streams_succeeded = error.report.streams_succeeded;
                diagnostics.records_extracted = error.report.records_extracted;
                diagnostics.records_rejected = error.report.records_rejected;
                let failure_code = error.reason.collector_failure_code();
                self.record_diagnostic(
                    &request,
                    AntigravityDiagnosticInput {
                        severity: DiagnosticSeverity::Warning,
                        code: error.reason.diagnostic_code(),
                        summary: error.reason.summary(),
                        counters: &diagnostics,
                        failure_code: Some(failure_code.code()),
                        failure_reason: Some(error.reason.failure_reason()),
                    },
                );
                return Err(failure(&request, failure_code));
            }
        };
        diagnostics.stream_calls_attempted = report.stream_calls_attempted;
        diagnostics.streams_succeeded = report.streams_succeeded;
        diagnostics.records_extracted = report.records_extracted;
        diagnostics.records_rejected = report.records_rejected;
        if cancellation.is_cancelled() {
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Warning,
                    code: "antigravity.collection_cancelled",
                    summary: "Antigravity collection was cancelled.",
                    counters: &diagnostics,
                    failure_code: Some(CollectorFailureCode::Cancelled.code()),
                    failure_reason: Some("cancelled"),
                },
            );
            return Err(failure(&request, CollectorFailureCode::Cancelled));
        }

        let result = result_from_usage(&request, started, started_at, report.usage);
        if result.is_ok() {
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Info,
                    code: "antigravity.collection_completed",
                    summary: "Antigravity collection completed.",
                    counters: &diagnostics,
                    failure_code: None,
                    failure_reason: None,
                },
            );
        }
        result
    }
}

struct AntigravityDiagnosticInput<'a> {
    severity: DiagnosticSeverity,
    code: &'a str,
    summary: &'a str,
    counters: &'a AntigravityDiagnosticCounters,
    failure_code: Option<&'a str>,
    failure_reason: Option<&'a str>,
}

impl AntigravityCollector {
    fn record_diagnostic(
        &self,
        request: &CollectionRequest,
        input: AntigravityDiagnosticInput<'_>,
    ) {
        let Some(recorder) = &self.diagnostics else {
            return;
        };
        let Ok(code) = DiagnosticCode::new(input.code) else {
            return;
        };
        let Ok(summary) = DiagnosticSummary::new(input.summary) else {
            return;
        };
        let Ok(context) = DiagnosticContext::new(
            json!({
                "source": "antigravity",
                "projection": projection_name(request.projection()),
                "failureCode": input.failure_code,
                "failureReason": input.failure_reason,
                "processCandidatesFound": input.counters.process_candidates_found,
                "endpointsFound": input.counters.endpoints_found,
                "endpointsAccepted": input.counters.endpoints_accepted,
                "identityProbesAttempted": input.counters.identity_probes_attempted,
                "identityProbesSucceeded": input.counters.identity_probes_succeeded,
                "sqliteDbsScanned": input.counters.sqlite_dbs_scanned,
                "metadataCallsAttempted": input.counters.metadata_calls_attempted,
                "metadataCallsSucceeded": input.counters.metadata_calls_succeeded,
                "conversationArtifactsFound": input.counters.conversations_found,
                "streamCallsAttempted": input.counters.stream_calls_attempted,
                "streamsSucceeded": input.counters.streams_succeeded,
                "recordsExtracted": input.counters.records_extracted,
                "recordsRejected": input.counters.records_rejected,
            })
            .to_string(),
        ) else {
            return;
        };
        let Ok(event) = DiagnosticEvent::new(
            DiagnosticArea::Collector,
            input.severity,
            code,
            summary,
            Some(context),
            Utc::now().timestamp_millis(),
        ) else {
            return;
        };
        recorder.record(event);
    }
}

fn collect_runtime_usage(
    client: &RuntimeClient,
    endpoints: &[RuntimeEndpoint],
    conversations: &[ConversationDatabase],
) -> Result<RuntimeUsageReport, AntigravityRuntimeCollectionFailure> {
    let mut collected = Vec::new();
    let mut attempted = false;
    let mut stream_calls_attempted = 0_u32;
    let mut successful_streams = 0_u32;
    let mut records_extracted = 0_u32;
    let mut records_rejected = 0_u32;
    for conversation in conversations {
        let mut records = Vec::new();
        for endpoint in endpoints
            .iter()
            .filter(|endpoint| endpoint.variant == conversation.variant)
        {
            attempted = true;
            stream_calls_attempted = stream_calls_attempted.saturating_add(1);
            let frames =
                match client.stream_agent_state_updates(endpoint, &conversation.conversation_id) {
                    Ok(frames) => frames,
                    Err(_) => continue,
                };
            successful_streams = successful_streams.saturating_add(1);
            let mut extracted = match extract_usage_records(
                conversation.variant,
                &conversation.conversation_id,
                &frames,
            ) {
                Ok(extracted) => extracted,
                Err(_) => {
                    records_rejected = records_rejected.saturating_add(1);
                    continue;
                }
            };
            records_extracted =
                records_extracted.saturating_add(extracted.len().try_into().unwrap_or(u32::MAX));
            records.append(&mut extracted);
        }
        let records_before_dedupe = records.len();
        let records = dedupe_records(records);
        records_rejected = records_rejected.saturating_add(
            records_before_dedupe
                .saturating_sub(records.len())
                .try_into()
                .unwrap_or(u32::MAX),
        );
        if !records.is_empty() {
            collected.push(ConversationUsage {
                database: conversation.clone(),
                records,
            });
        }
    }
    let report = RuntimeUsageReport {
        usage: collected,
        stream_calls_attempted,
        streams_succeeded: successful_streams,
        records_extracted,
        records_rejected,
    };
    if !attempted {
        return Err(AntigravityRuntimeCollectionFailure {
            reason: AntigravityRuntimeCollectionFailureReason::NoMatchingRuntimeEndpoint,
            report,
        });
    }
    if successful_streams == 0 {
        return Err(AntigravityRuntimeCollectionFailure {
            reason: AntigravityRuntimeCollectionFailureReason::RuntimeStreamUnavailable,
            report,
        });
    }
    Ok(report)
}

#[derive(Debug, Clone, Default)]
struct RuntimeUsageReport {
    usage: Vec<ConversationUsage>,
    stream_calls_attempted: u32,
    streams_succeeded: u32,
    records_extracted: u32,
    records_rejected: u32,
}

impl RuntimeUsageReport {
    #[cfg(test)]
    fn from_usage(usage: Vec<ConversationUsage>) -> Self {
        let records_extracted = usage
            .iter()
            .map(|conversation| conversation.records.len())
            .sum::<usize>()
            .try_into()
            .unwrap_or(u32::MAX);
        Self {
            streams_succeeded: usage.len().try_into().unwrap_or(u32::MAX),
            stream_calls_attempted: usage.len().try_into().unwrap_or(u32::MAX),
            usage,
            records_extracted,
            records_rejected: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct AntigravityDiagnosticCounters {
    process_candidates_found: usize,
    endpoints_found: usize,
    endpoints_accepted: usize,
    identity_probes_attempted: u32,
    identity_probes_succeeded: u32,
    sqlite_dbs_scanned: usize,
    metadata_calls_attempted: u32,
    metadata_calls_succeeded: u32,
    conversations_found: usize,
    stream_calls_attempted: u32,
    streams_succeeded: u32,
    records_extracted: u32,
    records_rejected: u32,
}

fn dedupe_records(
    records: Vec<super::AntigravityUsageRecord>,
) -> Vec<super::AntigravityUsageRecord> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();
    for record in records {
        let key = record.response_id.clone().unwrap_or_else(|| {
            format!(
                "{}:{}:{}:{}:{}",
                record.variant.as_str(),
                record.conversation_id,
                record.raw_model_id,
                record.input_tokens,
                record.output_tokens
            )
        });
        if seen.insert(key) {
            deduped.push(record);
        }
    }
    deduped
}

fn bounded_conversations(
    mut conversations: Vec<ConversationDatabase>,
) -> Vec<ConversationDatabase> {
    conversations.truncate(100);
    conversations
}

fn result_from_usage(
    request: &CollectionRequest,
    started: Instant,
    started_at: DateTime<Utc>,
    usage: Vec<ConversationUsage>,
) -> Result<CollectionResult, CollectorFailure> {
    let finished_at = Utc::now();
    let metadata = metadata(request, started_at, finished_at)?;
    let context = AntigravityMappingContext::new(
        collector_key()?,
        COLLECTOR_VERSION.to_owned(),
        request.collection_id().clone(),
        finished_at,
    )
    .map_err(|_| failure(request, CollectorFailureCode::Internal))?;
    let process_summary = ProcessSummary {
        runtime_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        stdout_bytes: 0,
        stderr_bytes: 0,
        exit_code: None,
    };

    match request.projection() {
        CollectionProjection::Daily => {
            let timezone = request
                .aggregation_timezone()
                .ok_or_else(|| failure(request, CollectorFailureCode::ScopeNotRepresentable))?;
            let candidates = mapper::map_daily(usage, timezone, request.scope(), &context)
                .map_err(|_| failure(request, CollectorFailureCode::IncompatibleEnvelope))?;
            CollectionResult::daily(
                metadata,
                candidates,
                Vec::new(),
                Vec::new(),
                process_summary,
            )
        }
        CollectionProjection::Session => {
            let candidates = mapper::map_sessions(usage, &context)
                .map_err(|_| failure(request, CollectorFailureCode::IncompatibleEnvelope))?;
            CollectionResult::session(
                metadata,
                candidates,
                Vec::new(),
                Vec::new(),
                process_summary,
            )
        }
    }
    .map_err(|_| failure(request, CollectorFailureCode::Internal))
}

fn empty_result(
    request: &CollectionRequest,
    started: Instant,
    started_at: DateTime<Utc>,
) -> Result<CollectionResult, CollectorFailure> {
    let finished_at = Utc::now();
    let metadata = metadata(request, started_at, finished_at)?;
    let process_summary = ProcessSummary {
        runtime_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        stdout_bytes: 0,
        stderr_bytes: 0,
        exit_code: None,
    };

    match request.projection() {
        CollectionProjection::Daily => CollectionResult::daily(
            metadata,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            process_summary,
        ),
        CollectionProjection::Session => CollectionResult::session(
            metadata,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            process_summary,
        ),
    }
    .map_err(|_| failure(request, CollectorFailureCode::Internal))
}

fn metadata(
    request: &CollectionRequest,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> Result<CollectionMetadata, CollectorFailure> {
    CollectionMetadata::new(
        request.collection_id().clone(),
        collector_key()?,
        COLLECTOR_VERSION.to_owned(),
        SourceKey::Antigravity,
        request.scope().clone(),
        PROFILE_VERSION,
        CollectionPeriod {
            started_at,
            finished_at,
        },
    )
    .map_err(|_| failure(request, CollectorFailureCode::Internal))
}

fn validate_request(request: &CollectionRequest) -> Result<(), CollectorFailure> {
    if request.source() != SourceKey::Antigravity {
        return Err(failure(request, CollectorFailureCode::UnsupportedSource));
    }
    Ok(())
}

fn descriptor() -> Result<CollectorDescriptor, CollectorFailure> {
    Ok(CollectorDescriptor {
        collector: collector_key()?,
        display_name: DISPLAY_NAME.to_owned(),
        runtime_version: COLLECTOR_VERSION.to_owned(),
        expected_version: COLLECTOR_VERSION.to_owned(),
        adapter_version: ADAPTER_VERSION,
        binary_target: std::env::consts::OS.to_owned(),
        integrity: CollectorIntegrity::UnverifiedDevelopment,
        profiles: vec![ProfileDescriptor {
            source: SourceKey::Antigravity,
            profile_version: PROFILE_VERSION,
            supported_projections: supported_projections(),
        }],
    })
}

fn supported_projections() -> Vec<CollectionProjection> {
    vec![CollectionProjection::Daily, CollectionProjection::Session]
}

fn collector_key() -> Result<CollectorKey, CollectorFailure> {
    CollectorKey::new(COLLECTOR_KEY)
        .map_err(|_| CollectorFailure::new(CollectorFailureCode::Internal, None, None))
}

fn failure(request: &CollectionRequest, code: CollectorFailureCode) -> CollectorFailure {
    CollectorFailure::new(code, Some(request.source()), Some(request.projection()))
}

fn projection_name(projection: CollectionProjection) -> &'static str {
    match projection {
        CollectionProjection::Daily => "daily",
        CollectionProjection::Session => "session",
    }
}

fn issue(code: &str, message: &str) -> DetectionIssue {
    DetectionIssue {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{CollectionOutcome, CollectionScope};
    use crate::application::diagnostics::DiagnosticSeverity;
    use crate::infrastructure::collectors::antigravity::discovery::{
        LocalListener, ProcessSnapshot,
    };
    use crate::infrastructure::collectors::antigravity::product_variant::AntigravityProductVariant;
    use crate::infrastructure::collectors::support::{
        daily_request as support_daily_request, detection_request as support_detection_request,
        fixed_timestamp, session_request as support_session_request, NeverCancelled,
        RecordingDiagnostics,
    };

    #[test]
    fn describes_antigravity_profile() {
        let collector = AntigravityCollector::new();

        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.collector.as_str(), "antigravity");
        assert_eq!(descriptor.display_name, "Antigravity");
        assert_eq!(descriptor.profiles[0].source, SourceKey::Antigravity);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn detects_pending_runtime_discovery_as_not_found() {
        let collector = collector_with_discovery(RuntimeDiscovery::from_processes(Vec::new()));

        let result = collector
            .detect(detection_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("detection");

        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(result.supported_projections, supported_projections());
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].code, "antigravity.runtime_not_found");
    }

    #[test]
    fn detects_available_runtime_endpoint() {
        let collector = collector_with_discovery(RuntimeDiscovery::from_processes(vec![
            ProcessSnapshot::new(
                10,
                Some(PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::ipv4(34415)],
            ),
        ]));

        let result = collector
            .detect(detection_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("detection");

        assert_eq!(result.state, DetectionState::Available);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn rejects_other_sources() {
        let collector = collector_with_discovery(RuntimeDiscovery::from_processes(Vec::new()));

        let error = collector
            .collect(daily_request(SourceKey::Cline), &NeverCancelled)
            .expect_err("unsupported source");

        assert_eq!(error.code, CollectorFailureCode::UnsupportedSource);
    }

    #[test]
    fn returns_source_not_found_when_runtime_endpoint_is_missing() {
        let collector = collector_with_discovery(RuntimeDiscovery::from_processes(Vec::new()));

        let error = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect_err("missing runtime");

        assert_eq!(error.code, CollectorFailureCode::SourceNotFound);
    }

    #[test]
    fn records_diagnostic_when_runtime_endpoint_is_missing() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = collector_with_discovery(RuntimeDiscovery::from_processes(Vec::new()))
            .with_test_diagnostics(diagnostics.clone());

        let error = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect_err("missing runtime");

        assert_eq!(error.code, CollectorFailureCode::SourceNotFound);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].area, DiagnosticArea::Collector);
        assert_eq!(events[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(events[0].code.as_str(), "antigravity.runtime_not_found");
        let context = events[0]
            .context
            .as_ref()
            .expect("context")
            .as_str()
            .to_owned();
        assert!(context.contains(r#""endpointsFound":0"#));
        assert!(context.contains(r#""endpointsAccepted":0"#));
        assert!(context.contains(r#""failureCode":"source.not_found""#));
        assert!(context.contains(r#""failureReason":"runtime_not_found""#));
        assert!(context.contains(r#""projection":"daily""#));
    }

    #[test]
    fn records_diagnostic_when_conversation_has_no_matching_runtime_endpoint() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (_directory, collector) = collector_with_conversation_and_runtime(
            AntigravityProductVariant::App,
            AntigravityProductVariant::Cli,
            RuntimeUsageSource::Current(RuntimeClient::new()),
        );
        let collector = collector.with_test_diagnostics(diagnostics.clone());

        let error = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect_err("variant mismatch");

        assert_eq!(error.code, CollectorFailureCode::SourceNotFound);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].code.as_str(),
            "antigravity.runtime_endpoint_mismatch"
        );
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""endpointsFound":1"#));
        assert!(context.contains(r#""conversationArtifactsFound":1"#));
        assert!(context.contains(r#""streamCallsAttempted":0"#));
        assert!(context.contains(r#""failureCode":"source.not_found""#));
        assert!(context.contains(r#""failureReason":"no_matching_runtime_endpoint""#));
    }

    #[test]
    fn records_diagnostic_when_runtime_stream_is_unavailable() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (_directory, collector) = collector_with_conversation_and_runtime(
            AntigravityProductVariant::App,
            AntigravityProductVariant::App,
            RuntimeUsageSource::Failing(
                AntigravityRuntimeCollectionFailureReason::RuntimeStreamUnavailable,
            ),
        );
        let collector = collector.with_test_diagnostics(diagnostics.clone());

        let error = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect_err("stream unavailable");

        assert_eq!(error.code, CollectorFailureCode::SourceNotFound);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].code.as_str(),
            "antigravity.runtime_stream_unavailable"
        );
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""endpointsFound":1"#));
        assert!(context.contains(r#""conversationArtifactsFound":1"#));
        assert!(context.contains(r#""failureCode":"source.not_found""#));
        assert!(context.contains(r#""failureReason":"runtime_stream_unavailable""#));
    }

    #[test]
    fn collects_daily_usage_from_runtime_records() {
        let (_directory, collector) = collector_with_usage();

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.metadata().collector().as_str(), "antigravity");
        assert_eq!(result.daily_candidates().len(), 1);
        let candidate = &result.daily_candidates()[0];
        assert_eq!(candidate.tokens.input_tokens(), Some(180));
        assert_eq!(candidate.tokens.output_tokens(), Some(50));
        assert_eq!(candidate.tokens.cache_read_tokens(), Some(12));
        assert_eq!(candidate.tokens.cache_creation_tokens(), Some(3));
        assert_eq!(candidate.tokens.total_tokens(), 245);
    }

    #[test]
    fn records_diagnostic_when_collection_completes() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (_directory, collector) = collector_with_usage();
        let collector = collector.with_test_diagnostics(diagnostics.clone());

        collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("collection");

        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, DiagnosticSeverity::Info);
        assert_eq!(events[0].code.as_str(), "antigravity.collection_completed");
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""processCandidatesFound":1"#));
        assert!(context.contains(r#""endpointsAccepted":1"#));
        assert!(context.contains(r#""sqliteDbsScanned":2"#));
        assert!(context.contains(r#""conversationArtifactsFound":2"#));
        assert!(context.contains(r#""recordsExtracted":2"#));
        assert!(context.contains(r#""recordsRejected":0"#));
        assert!(context.contains(r#""streamsSucceeded":2"#));
    }

    #[test]
    fn collects_session_usage_from_runtime_records() {
        let (_directory, collector) = collector_with_usage();

        let result = collector
            .collect(session_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.metadata().collector().as_str(), "antigravity");
        assert_eq!(result.session_candidates().len(), 2);
        assert!(result
            .session_candidates()
            .iter()
            .any(|candidate| candidate.source_session_id == "antigravity:app-conversation"));
    }

    #[test]
    fn records_diagnostic_when_identity_probe_rejects_all_endpoints() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = collector_with_discovery(RuntimeDiscovery::from_processes(vec![
            ProcessSnapshot::new(
                10,
                Some(PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::ipv4(34415)],
            ),
        ]))
        .with_endpoint_validation(EndpointValidationSource::RejectAll)
        .with_test_diagnostics(diagnostics.clone());

        let error = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect_err("identity probe failed");

        assert_eq!(error.code, CollectorFailureCode::SourceNotFound);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].code.as_str(),
            "antigravity.runtime_identity_probe_failed"
        );
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""endpointsFound":1"#));
        assert!(context.contains(r#""endpointsAccepted":0"#));
        assert!(context.contains(r#""identityProbesAttempted":1"#));
        assert!(context.contains(r#""identityProbesSucceeded":0"#));
        assert!(context.contains(r#""failureReason":"runtime_identity_probe_failed""#));
    }

    #[test]
    fn accepts_multiple_ide_endpoints_after_validation() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = collector_with_discovery(RuntimeDiscovery::from_processes(vec![
            ProcessSnapshot::new(
                11,
                Some(PathBuf::from(
                    "/opt/antigravity-ide/Antigravity-IDE/resources/app/extensions/antigravity/bin/language_server_linux_x64",
                )),
                vec![
                    "language_server_linux_x64".to_owned(),
                    "--app_data_dir".to_owned(),
                    "antigravity-ide".to_owned(),
                    "--csrf_token".to_owned(),
                    "token-main".to_owned(),
                ],
                vec![LocalListener::ipv4(35625)],
            ),
            ProcessSnapshot::new(
                12,
                Some(PathBuf::from(
                    "/opt/antigravity-ide/Antigravity-IDE/resources/app/extensions/antigravity/bin/language_server_linux_x64",
                )),
                vec![
                    "language_server_linux_x64".to_owned(),
                    "--enable_lsp".to_owned(),
                    "--app_data_dir".to_owned(),
                    "antigravity-ide".to_owned(),
                    "--csrf_token".to_owned(),
                    "token-workspace".to_owned(),
                ],
                vec![LocalListener::ipv4(41647)],
            ),
        ]))
        .with_test_diagnostics(diagnostics.clone());

        let result = collector
            .detect(detection_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("detection");

        assert_eq!(result.state, DetectionState::Available);
        assert!(result.issues.is_empty());
    }

    fn collector_with_discovery(runtime_discovery: RuntimeDiscovery) -> AntigravityCollector {
        let data_root = TempDir::new().expect("tempdir");
        AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            runtime_discovery,
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(Vec::new()),
        )
    }

    fn collector_with_usage() -> (TempDir, AntigravityCollector) {
        let data_root = TempDir::new().expect("tempdir");
        create_db(
            data_root.path(),
            AntigravityProductVariant::App,
            "app-conversation",
        );
        create_db(
            data_root.path(),
            AntigravityProductVariant::Ide,
            "ide-conversation",
        );
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
                10,
                Some(PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::ipv4(34415)],
            )]),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(fixed_usage()),
        );
        (data_root, collector)
    }

    fn collector_with_conversation_and_runtime(
        conversation_variant: AntigravityProductVariant,
        runtime_variant: AntigravityProductVariant,
        runtime_usage: RuntimeUsageSource,
    ) -> (TempDir, AntigravityCollector) {
        let data_root = TempDir::new().expect("tempdir");
        create_db(data_root.path(), conversation_variant, "conversation");
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(vec![process_for_variant(runtime_variant)]),
            EndpointValidationSource::Passthrough,
            runtime_usage,
        );
        (data_root, collector)
    }

    fn process_for_variant(variant: AntigravityProductVariant) -> ProcessSnapshot {
        match variant {
            AntigravityProductVariant::App => ProcessSnapshot::new(
                10,
                Some(PathBuf::from("/opt/antigravity/Antigravity-x64")),
                vec!["/opt/antigravity/Antigravity-x64".to_owned()],
                vec![LocalListener::ipv4(34415)],
            ),
            AntigravityProductVariant::Ide => ProcessSnapshot::new(
                10,
                Some(PathBuf::from("/opt/antigravity-ide/Antigravity-IDE")),
                vec!["/opt/antigravity-ide/Antigravity-IDE".to_owned()],
                vec![LocalListener::ipv4(34415)],
            ),
            AntigravityProductVariant::Cli => ProcessSnapshot::new(
                10,
                Some(PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::ipv4(34415)],
            ),
        }
    }

    fn fixed_usage() -> Vec<ConversationUsage> {
        vec![
            ConversationUsage {
                database: database(AntigravityProductVariant::App, "app-conversation"),
                records: vec![record(
                    AntigravityProductVariant::App,
                    "app-conversation",
                    "MODEL_PLACEHOLDER_M16",
                    "Gemini 3.1 Pro (High)",
                    100,
                    30,
                    5,
                    3,
                )],
            },
            ConversationUsage {
                database: database(AntigravityProductVariant::Ide, "ide-conversation"),
                records: vec![record(
                    AntigravityProductVariant::Ide,
                    "ide-conversation",
                    "gemini-flash",
                    "gemini-flash",
                    80,
                    20,
                    7,
                    0,
                )],
            },
        ]
    }

    fn database(variant: AntigravityProductVariant, conversation_id: &str) -> ConversationDatabase {
        ConversationDatabase {
            variant,
            conversation_id: conversation_id.to_owned(),
            path: conversation_id.into(),
            modified_at: timestamp(),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "test fixture keeps token fields explicit"
    )]
    fn record(
        variant: AntigravityProductVariant,
        conversation_id: &str,
        raw_model_id: &str,
        model_label: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    ) -> super::super::AntigravityUsageRecord {
        super::super::AntigravityUsageRecord {
            variant,
            conversation_id: conversation_id.to_owned(),
            raw_model_id: raw_model_id.to_owned(),
            model_label: model_label.to_owned(),
            api_provider: Some("API_PROVIDER_GOOGLE_GEMINI".to_owned()),
            response_id: Some(format!("{conversation_id}:{raw_model_id}")),
            input_tokens,
            output_tokens,
            thinking_output_tokens: 0,
            response_output_tokens: output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            consumed_credits: None,
            flow_credits_used: None,
            prompt_credits_used: None,
        }
    }

    fn create_db(root: &std::path::Path, variant: AntigravityProductVariant, name: &str) {
        let directory = root.join(variant.data_dir_name()).join("conversations");
        fs::create_dir_all(&directory).expect("conversation dir");
        File::create(directory.join(format!("{name}.db"))).expect("db file");
    }

    fn detection_request(source: SourceKey) -> DetectionRequest {
        support_detection_request(source, timestamp())
    }

    fn daily_request(source: SourceKey) -> CollectionRequest {
        support_daily_request(
            &format!("{}-daily", source.as_str()),
            source,
            CollectionScope::Full,
            "UTC",
            timestamp(),
        )
    }

    fn session_request(source: SourceKey) -> CollectionRequest {
        support_session_request(&format!("{}-session", source.as_str()), source, timestamp())
    }

    fn timestamp() -> chrono::DateTime<chrono::Utc> {
        fixed_timestamp(2026, 7, 2, 8, 0, 0)
    }
}
