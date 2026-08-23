use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::application::collection::{
    CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionRequest,
    CollectionResult, CollectionScope, CollectorDescriptor, CollectorFailure, CollectorFailureCode,
    CollectorIntegrity, CollectorKey, DetectionIssue, DetectionRequest, DetectionResult,
    DetectionState, ProcessSummary, ProfileDescriptor,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;

use super::app_ide_sqlite_reader::{collect_app_ide_sqlite_fallback, AppIdeSqliteCollectionReport};
use super::cli_sqlite_reader::{
    collect_cli_sqlite_usage, is_sqlite_artifact, CliSqliteCollectionReport,
};
#[cfg(test)]
use super::discovery::{LocalListener, ProcessSnapshot};
use super::mapper::{self, AntigravityMappingContext, ConversationUsage};
use super::product_variant::AntigravityProductVariant;
use super::runtime_metadata_client::{
    fetch_generator_metadata_items, list_trajectory_summaries, TrajectorySummary,
};
use super::usage_cache::{AntigravityUsageCacheClient, NoOpAntigravityUsageCache};
use super::{
    extract_usage_from_generator_metadata, ConversationDatabase, ConversationIndex, RuntimeClient,
    RuntimeDiscovery, RuntimeDiscoveryReport, RuntimeEndpoint, PROFILE_VERSION,
};

const COLLECTOR_KEY: &str = "antigravity";
const DISPLAY_NAME: &str = "Antigravity";
const COLLECTOR_VERSION: &str = "local-rpc";
const ADAPTER_VERSION: u16 = 1;
const MAX_INCREMENTAL_CONVERSATIONS: usize = 100;
const CONVERSATION_BATCH_SIZE: usize = 20;

#[derive(Clone)]
pub(crate) struct AntigravityCollector {
    conversation_index: ConversationIndex,
    runtime_discovery: RuntimeDiscoverySource,
    endpoint_validation: EndpointValidationSource,
    runtime_usage: RuntimeUsageSource,
    usage_cache: AntigravityUsageCacheClient,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
    calculator: BurnlyCostCalculator,
}

impl AntigravityCollector {
    pub(crate) fn new() -> Self {
        let runtime_client = RuntimeClient::new();
        Self {
            conversation_index: ConversationIndex::default(),
            runtime_discovery: RuntimeDiscoverySource::Current,
            endpoint_validation: EndpointValidationSource::Current(runtime_client.clone()),
            runtime_usage: RuntimeUsageSource::Current(runtime_client),
            usage_cache: AntigravityUsageCacheClient::new(Arc::new(NoOpAntigravityUsageCache)),
            diagnostics: None,
            calculator: BurnlyCostCalculator::new(),
        }
    }

    pub(crate) fn with_diagnostic_recorder(
        diagnostics: Arc<dyn DiagnosticRecorder>,
        usage_cache: Arc<
            dyn crate::application::ports::antigravity_usage_cache::AntigravityUsageCache,
        >,
    ) -> Self {
        Self {
            diagnostics: Some(diagnostics),
            usage_cache: AntigravityUsageCacheClient::new(usage_cache),
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
            AntigravityUsageCacheClient::new(Arc::new(NoOpAntigravityUsageCache)),
        )
    }

    #[cfg(test)]
    fn from_parts(
        conversation_index: ConversationIndex,
        runtime_discovery: RuntimeDiscovery,
        endpoint_validation: EndpointValidationSource,
        runtime_usage: RuntimeUsageSource,
        usage_cache: AntigravityUsageCacheClient,
    ) -> Self {
        Self {
            conversation_index,
            runtime_discovery: RuntimeDiscoverySource::Fixed(runtime_discovery),
            endpoint_validation,
            runtime_usage,
            usage_cache,
            diagnostics: None,
            calculator: BurnlyCostCalculator::new(),
        }
    }

    #[cfg(test)]
    fn with_usage_cache(mut self, usage_cache: AntigravityUsageCacheClient) -> Self {
        self.usage_cache = usage_cache;
        self
    }

    #[cfg(test)]
    fn with_test_diagnostics(mut self, diagnostics: Arc<dyn DiagnosticRecorder>) -> Self {
        self.diagnostics = Some(diagnostics);
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
    RuntimeMetadataUnavailable,
}

impl AntigravityRuntimeCollectionFailureReason {
    const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::RuntimeNotFound => "antigravity.runtime_not_found",
            Self::RuntimeIdentityProbeFailed => "antigravity.runtime_identity_probe_failed",
            Self::NoMatchingRuntimeEndpoint => "antigravity.runtime_endpoint_mismatch",
            Self::RuntimeMetadataUnavailable => "antigravity.metadata_rpc_unavailable",
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
            Self::RuntimeMetadataUnavailable => {
                "Antigravity runtime endpoints were found, but generator metadata could not be read."
            }
        }
    }

    const fn failure_reason(self) -> &'static str {
        match self {
            Self::RuntimeNotFound => "runtime_not_found",
            Self::RuntimeIdentityProbeFailed => "runtime_identity_probe_failed",
            Self::NoMatchingRuntimeEndpoint => "no_matching_runtime_endpoint",
            Self::RuntimeMetadataUnavailable => "metadata_rpc_unavailable",
        }
    }

    const fn collector_failure_code(self) -> CollectorFailureCode {
        match self {
            Self::RuntimeNotFound
            | Self::RuntimeIdentityProbeFailed
            | Self::NoMatchingRuntimeEndpoint
            | Self::RuntimeMetadataUnavailable => CollectorFailureCode::SourceNotFound,
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
        let validation = self.endpoint_validation.validate(&discovery.endpoints);
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
        let validation = self.endpoint_validation.validate(&discovery.endpoints);
        let mut diagnostics = AntigravityDiagnosticCounters {
            process_candidates_found: discovery.process_candidates_found,
            endpoints_found: discovery.endpoints.len(),
            endpoints_accepted: validation.endpoints.len(),
            identity_probes_attempted: validation.identity_probes_attempted,
            identity_probes_succeeded: validation.identity_probes_succeeded,
            ..AntigravityDiagnosticCounters::default()
        };
        let endpoints = validation.endpoints;
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
                        variants: Vec::new(),
                    },
                );
                failure(&request, CollectorFailureCode::ScopeNotRepresentable)
            })?;
        let conversations = bounded_conversations(conversations, request.scope());
        diagnostics.sqlite_dbs_scanned = conversations
            .iter()
            .filter(|conversation| is_sqlite_artifact(&conversation.path))
            .count();
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
                    variants: Vec::new(),
                },
            );
            return empty_result(&request, started, started_at);
        }
        let mut collected_usage = Vec::new();
        let mut sqlite_report = CliSqliteCollectionReport::default();
        let mut app_ide_report = AppIdeSqliteCollectionReport::default();
        for batch in conversations.chunks(CONVERSATION_BATCH_SIZE) {
            if cancellation.is_cancelled() {
                return Err(failure(&request, CollectorFailureCode::Cancelled));
            }
            let (cli_usage, batch_cli_report) = collect_cli_sqlite_usage(batch).unwrap_or_default();
            merge_conversation_usage(&mut collected_usage, cli_usage);
            merge_cli_sqlite_report(&mut sqlite_report, batch_cli_report);

            let (batch_app_ide_usage, batch_app_ide_report) =
                collect_app_ide_sqlite_fallback(batch);
            merge_conversation_usage(&mut collected_usage, batch_app_ide_usage);
            merge_app_ide_sqlite_report(&mut app_ide_report, batch_app_ide_report);
        }
        apply_cli_sqlite_diagnostics(&mut diagnostics, &sqlite_report);
        if sqlite_report.records_rejected > 0 {
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Warning,
                    code: "antigravity.sqlite_parse_failed",
                    summary: "Antigravity CLI SQLite usage metadata could not be parsed for one or more conversations.",
                    counters: &diagnostics,
                    failure_code: None,
                    failure_reason: Some("sqlite_parse_failed"),
                    variants: Vec::new(),
                },
            );
        }

        apply_app_ide_sqlite_diagnostics(&mut diagnostics, &app_ide_report);
        if app_ide_report.conversations_accepted > 0 {
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Info,
                    code: "antigravity.sqlite_fallback_accepted",
                    summary:
                        "Antigravity experimental App/IDE SQLite fallback produced usage records.",
                    counters: &diagnostics,
                    failure_code: None,
                    failure_reason: Some("sqlite_fallback_accepted"),
                    variants: variant_names(&app_ide_report.variants_accepted),
                },
            );
        }
        if app_ide_report.conversations_rejected > 0 {
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Info,
                    code: "antigravity.sqlite_fallback_rejected",
                    summary: "Antigravity experimental App/IDE SQLite fallback rejected one or more conversation databases.",
                    counters: &diagnostics,
                    failure_code: None,
                    failure_reason: Some("sqlite_fallback_rejected"),
                    variants: variant_names(&app_ide_report.variants_rejected),
                },
            );
        }

        let runtime_targets = conversations_needing_runtime(&conversations, &collected_usage);
        let default_failure_reason = if discovery.endpoints.is_empty() {
            AntigravityRuntimeCollectionFailureReason::RuntimeNotFound
        } else if endpoints.is_empty() {
            AntigravityRuntimeCollectionFailureReason::RuntimeIdentityProbeFailed
        } else {
            AntigravityRuntimeCollectionFailureReason::RuntimeMetadataUnavailable
        };
        let mut runtime_failure = if endpoints.is_empty() && !runtime_targets.is_empty() {
            Some(default_failure_reason)
        } else {
            None
        };

        if !endpoints.is_empty() && !runtime_targets.is_empty() {
            match self.runtime_usage.collect(&endpoints, &runtime_targets) {
                Ok(report) => {
                    runtime_failure = None;
                    diagnostics.metadata_calls_attempted = report.metadata_calls_attempted;
                    diagnostics.metadata_calls_succeeded = report.metadata_calls_succeeded;
                    diagnostics.records_extracted = diagnostics
                        .records_extracted
                        .saturating_add(report.records_extracted);
                    diagnostics.records_rejected = diagnostics
                        .records_rejected
                        .saturating_add(report.records_rejected);
                    merge_conversation_usage(&mut collected_usage, report.usage);
                }
                Err(error) => {
                    runtime_failure = Some(error.reason);
                    diagnostics.metadata_calls_attempted = error.report.metadata_calls_attempted;
                    diagnostics.metadata_calls_succeeded = error.report.metadata_calls_succeeded;
                    diagnostics.records_extracted = diagnostics
                        .records_extracted
                        .saturating_add(error.report.records_extracted);
                    diagnostics.records_rejected = diagnostics
                        .records_rejected
                        .saturating_add(error.report.records_rejected);
                    merge_conversation_usage(&mut collected_usage, error.report.usage);
                }
            }
        }

        if !collected_usage.is_empty()
            && full_cli_scan_is_incomplete(request.scope(), &sqlite_report)
        {
            self.record_diagnostic(
                &request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Warning,
                    code: "antigravity.full_reconciliation_incomplete",
                    summary:
                        "Antigravity full reconciliation could not process every CLI usage record.",
                    counters: &diagnostics,
                    failure_code: Some(CollectorFailureCode::IncompatibleEnvelope.code()),
                    failure_reason: Some("full_reconciliation_incomplete"),
                    variants: vec![AntigravityProductVariant::Cli.as_str()],
                },
            );
            return Err(failure(
                &request,
                CollectorFailureCode::IncompatibleEnvelope,
            ));
        }

        if !collected_usage.is_empty() {
            let resolution = match self.usage_cache.reconcile_usage(
                &mut collected_usage,
                COLLECTOR_VERSION,
                started_at,
            ) {
                Ok(report) => report,
                Err(_) => {
                    self.record_diagnostic(
                        &request,
                        AntigravityDiagnosticInput {
                            severity: DiagnosticSeverity::Warning,
                            code: "antigravity.cache_resolution_failed",
                            summary: "Antigravity usage timestamps could not be resolved durably.",
                            counters: &diagnostics,
                            failure_code: Some(CollectorFailureCode::Internal.code()),
                            failure_reason: Some("cache_resolution_failed"),
                            variants: Vec::new(),
                        },
                    );
                    return Err(failure(&request, CollectorFailureCode::Internal));
                }
            };
            diagnostics.source_reported_timestamp_records = resolution.source_reported_records;
            diagnostics.first_seen_timestamp_records = resolution.first_seen_records;
            diagnostics.legacy_records_repaired = resolution.legacy_records_repaired;
            diagnostics.unresolved_legacy_records = resolution.unresolved_legacy_records;
        }

        self.finish_collection(FinishCollectionInput {
            request: &request,
            started,
            started_at,
            diagnostics: &mut diagnostics,
            conversations: &conversations,
            usage: collected_usage,
            runtime_failure,
            default_failure_reason,
        })
    }
}

struct AntigravityDiagnosticInput<'a> {
    severity: DiagnosticSeverity,
    code: &'a str,
    summary: &'a str,
    counters: &'a AntigravityDiagnosticCounters,
    failure_code: Option<&'a str>,
    failure_reason: Option<&'a str>,
    variants: Vec<&'a str>,
}

struct FinishCollectionInput<'a> {
    request: &'a CollectionRequest,
    started: Instant,
    started_at: DateTime<Utc>,
    diagnostics: &'a mut AntigravityDiagnosticCounters,
    conversations: &'a [ConversationDatabase],
    usage: Vec<ConversationUsage>,
    runtime_failure: Option<AntigravityRuntimeCollectionFailureReason>,
    default_failure_reason: AntigravityRuntimeCollectionFailureReason,
}

impl AntigravityCollector {
    fn finish_collection(
        &self,
        input: FinishCollectionInput<'_>,
    ) -> Result<CollectionResult, CollectorFailure> {
        let FinishCollectionInput {
            request,
            started,
            started_at,
            diagnostics,
            conversations,
            mut usage,
            runtime_failure,
            default_failure_reason,
        } = input;
        let supplement = self
            .usage_cache
            .supplement_usage(
                request.scope(),
                request.aggregation_timezone().unwrap_or("UTC"),
                conversations,
                &mut usage,
            )
            .unwrap_or_default();
        diagnostics.cache_records_read = supplement.records_read;
        diagnostics.cache_records_used = supplement.records_used;
        for conversation_usage in &mut usage {
            conversation_usage.records = dedupe_records(conversation_usage.records.clone());
        }

        if usage.is_empty() {
            let reason = runtime_failure.unwrap_or(default_failure_reason);
            let failure_code = reason.collector_failure_code();
            self.record_diagnostic(
                request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Warning,
                    code: reason.diagnostic_code(),
                    summary: reason.summary(),
                    counters: diagnostics,
                    failure_code: Some(failure_code.code()),
                    failure_reason: Some(reason.failure_reason()),
                    variants: Vec::new(),
                },
            );
            return Err(failure(request, failure_code));
        }

        if supplement.used_cache {
            self.record_diagnostic(
                request,
                AntigravityDiagnosticInput {
                    severity: DiagnosticSeverity::Info,
                    code: "antigravity.cache_used",
                    summary: "Antigravity collection used cached usage records because runtime metadata was unavailable.",
                    counters: diagnostics,
                    failure_code: None,
                    failure_reason: Some("cache_used"),
                    variants: Vec::new(),
                },
            );
        }

        self.record_diagnostic(
            request,
            AntigravityDiagnosticInput {
                severity: DiagnosticSeverity::Info,
                code: "antigravity.collection_completed",
                summary: "Antigravity collection completed.",
                counters: diagnostics,
                failure_code: None,
                failure_reason: None,
                variants: Vec::new(),
            },
        );
        result_from_usage(request, started, started_at, usage, &self.calculator)
    }

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
                "cacheRecordsRead": input.counters.cache_records_read,
                "cacheRecordsUsed": input.counters.cache_records_used,
                "sourceReportedTimestampRecords": input.counters.source_reported_timestamp_records,
                "firstSeenTimestampRecords": input.counters.first_seen_timestamp_records,
                "legacyRecordsRepaired": input.counters.legacy_records_repaired,
                "unresolvedLegacyRecords": input.counters.unresolved_legacy_records,
                "sqliteRecordsExtracted": input.counters.sqlite_records_extracted,
                "sqliteRecordsRejected": input.counters.sqlite_records_rejected,
                "sqliteConversationsParsed": input.counters.sqlite_conversations_parsed,
                "sqliteConversationsFailed": input.counters.sqlite_conversations_failed,
                "appIdeSqliteRecordsExtracted": input.counters.app_ide_sqlite_records_extracted,
                "appIdeSqliteRecordsRejected": input.counters.app_ide_sqlite_records_rejected,
                "appIdeSqliteConversationsAccepted": input.counters.app_ide_sqlite_conversations_accepted,
                "appIdeSqliteConversationsRejected": input.counters.app_ide_sqlite_conversations_rejected,
                "variants": input.variants,
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
    let mut metadata_calls_attempted = 0_u32;
    let mut metadata_calls_succeeded = 0_u32;
    let mut records_extracted = 0_u32;
    let mut records_rejected = 0_u32;
    for endpoint in endpoints {
        let summaries = runtime_summaries_for_endpoint(client, endpoint, conversations);
        if summaries.is_empty() {
            continue;
        }
        attempted = true;
        for summary in summaries {
            metadata_calls_attempted = metadata_calls_attempted.saturating_add(1);
            let metadata_items =
                match fetch_generator_metadata_items(client, endpoint, &summary.cascade_id) {
                    Ok(items) => {
                        metadata_calls_succeeded = metadata_calls_succeeded.saturating_add(1);
                        items
                    }
                    Err(_) => continue,
                };
            if metadata_items.is_empty() {
                continue;
            }
            let conversation = conversation_for_summary(endpoint.variant, &summary, conversations);
            let extracted = match extract_usage_from_generator_metadata(
                conversation.variant,
                &conversation.conversation_id,
                &metadata_items,
            ) {
                Ok(extracted) => extracted,
                Err(_) => {
                    records_rejected = records_rejected.saturating_add(1);
                    continue;
                }
            };
            records_extracted =
                records_extracted.saturating_add(extracted.len().try_into().unwrap_or(u32::MAX));
            merge_conversation_usage(
                &mut collected,
                vec![ConversationUsage {
                    database: conversation,
                    records: extracted,
                }],
            );
        }
    }
    let report = RuntimeUsageReport {
        usage: collected,
        metadata_calls_attempted,
        metadata_calls_succeeded,
        records_extracted,
        records_rejected,
    };
    if !attempted {
        return Err(AntigravityRuntimeCollectionFailure {
            reason: AntigravityRuntimeCollectionFailureReason::NoMatchingRuntimeEndpoint,
            report,
        });
    }
    if metadata_calls_succeeded == 0 {
        return Err(AntigravityRuntimeCollectionFailure {
            reason: AntigravityRuntimeCollectionFailureReason::RuntimeMetadataUnavailable,
            report,
        });
    }
    Ok(report)
}

fn runtime_summaries_for_endpoint(
    client: &RuntimeClient,
    endpoint: &RuntimeEndpoint,
    conversations: &[ConversationDatabase],
) -> Vec<TrajectorySummary> {
    match list_trajectory_summaries(client, endpoint) {
        Ok(summaries) if !summaries.is_empty() => summaries,
        _ => conversations
            .iter()
            .filter(|conversation| conversation.variant == endpoint.variant)
            .map(|conversation| TrajectorySummary {
                cascade_id: conversation.conversation_id.clone(),
                step_count: None,
            })
            .collect(),
    }
}

fn conversation_for_summary(
    variant: AntigravityProductVariant,
    summary: &TrajectorySummary,
    conversations: &[ConversationDatabase],
) -> ConversationDatabase {
    conversations
        .iter()
        .find(|conversation| {
            conversation.variant == variant && conversation.conversation_id == summary.cascade_id
        })
        .cloned()
        .unwrap_or_else(|| ConversationDatabase {
            variant,
            conversation_id: summary.cascade_id.clone(),
            path: PathBuf::new(),
            modified_at: Utc::now(),
        })
}

#[derive(Debug, Clone, Default)]
struct RuntimeUsageReport {
    usage: Vec<ConversationUsage>,
    metadata_calls_attempted: u32,
    metadata_calls_succeeded: u32,
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
            metadata_calls_succeeded: usage.len().try_into().unwrap_or(u32::MAX),
            metadata_calls_attempted: usage.len().try_into().unwrap_or(u32::MAX),
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
    cache_records_read: u32,
    cache_records_used: u32,
    source_reported_timestamp_records: u32,
    first_seen_timestamp_records: u32,
    legacy_records_repaired: u32,
    unresolved_legacy_records: u32,
    sqlite_records_extracted: u32,
    sqlite_records_rejected: u32,
    sqlite_conversations_parsed: u32,
    sqlite_conversations_failed: u32,
    app_ide_sqlite_records_extracted: u32,
    app_ide_sqlite_records_rejected: u32,
    app_ide_sqlite_conversations_accepted: u32,
    app_ide_sqlite_conversations_rejected: u32,
}

fn variant_names(
    variants: &std::collections::BTreeSet<AntigravityProductVariant>,
) -> Vec<&'static str> {
    variants.iter().map(|variant| variant.as_str()).collect()
}

fn apply_cli_sqlite_diagnostics(
    diagnostics: &mut AntigravityDiagnosticCounters,
    report: &CliSqliteCollectionReport,
) {
    diagnostics.sqlite_records_extracted = report.records_extracted;
    diagnostics.sqlite_records_rejected = report.records_rejected;
    diagnostics.sqlite_conversations_parsed = report.conversations_parsed;
    diagnostics.sqlite_conversations_failed = report.conversations_failed;
    diagnostics.records_extracted = diagnostics
        .records_extracted
        .saturating_add(report.records_extracted);
    diagnostics.records_rejected = diagnostics
        .records_rejected
        .saturating_add(report.records_rejected);
}

fn apply_app_ide_sqlite_diagnostics(
    diagnostics: &mut AntigravityDiagnosticCounters,
    report: &AppIdeSqliteCollectionReport,
) {
    diagnostics.app_ide_sqlite_records_extracted = report.records_extracted;
    diagnostics.app_ide_sqlite_records_rejected = report.records_rejected;
    diagnostics.app_ide_sqlite_conversations_accepted = report.conversations_accepted;
    diagnostics.app_ide_sqlite_conversations_rejected = report.conversations_rejected;
    diagnostics.records_extracted = diagnostics
        .records_extracted
        .saturating_add(report.records_extracted);
    diagnostics.records_rejected = diagnostics
        .records_rejected
        .saturating_add(report.records_rejected);
}

fn conversations_needing_runtime(
    conversations: &[ConversationDatabase],
    collected: &[ConversationUsage],
) -> Vec<ConversationDatabase> {
    conversations
        .iter()
        .filter(|conversation| {
            let has_sqlite_records = collected.iter().any(|usage| {
                usage.database.conversation_id == conversation.conversation_id
                    && usage.database.variant == conversation.variant
                    && !usage.records.is_empty()
            });
            conversation.variant != AntigravityProductVariant::Cli || !has_sqlite_records
        })
        .cloned()
        .collect()
}

fn merge_conversation_usage(target: &mut Vec<ConversationUsage>, incoming: Vec<ConversationUsage>) {
    for mut entry in incoming {
        if let Some(existing) = target.iter_mut().find(|usage| {
            usage.database.conversation_id == entry.database.conversation_id
                && usage.database.variant == entry.database.variant
        }) {
            existing.records.append(&mut entry.records);
            existing.records = dedupe_records(existing.records.clone());
        } else {
            target.push(entry);
        }
    }
}

fn dedupe_records(
    records: Vec<super::AntigravityUsageRecord>,
) -> Vec<super::AntigravityUsageRecord> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();
    for record in records {
        let key = record.response_id.clone().unwrap_or_else(|| {
            if let Some(index) = record.source_record_index {
                return format!(
                    "{}:{}:idx:{index}",
                    record.variant.as_str(),
                    record.conversation_id
                );
            }
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
    scope: &CollectionScope,
) -> Vec<ConversationDatabase> {
    if !matches!(scope, CollectionScope::Full) {
        conversations.truncate(MAX_INCREMENTAL_CONVERSATIONS);
    }
    conversations
}

fn full_cli_scan_is_incomplete(
    scope: &CollectionScope,
    report: &CliSqliteCollectionReport,
) -> bool {
    matches!(scope, CollectionScope::Full)
        && (report.records_rejected > 0 || report.conversations_failed > 0)
}

fn merge_cli_sqlite_report(
    target: &mut CliSqliteCollectionReport,
    incoming: CliSqliteCollectionReport,
) {
    target.records_extracted = target
        .records_extracted
        .saturating_add(incoming.records_extracted);
    target.records_rejected = target
        .records_rejected
        .saturating_add(incoming.records_rejected);
    target.conversations_parsed = target
        .conversations_parsed
        .saturating_add(incoming.conversations_parsed);
    target.conversations_failed = target
        .conversations_failed
        .saturating_add(incoming.conversations_failed);
}

fn merge_app_ide_sqlite_report(
    target: &mut AppIdeSqliteCollectionReport,
    incoming: AppIdeSqliteCollectionReport,
) {
    target.records_extracted = target
        .records_extracted
        .saturating_add(incoming.records_extracted);
    target.records_rejected = target
        .records_rejected
        .saturating_add(incoming.records_rejected);
    target.conversations_accepted = target
        .conversations_accepted
        .saturating_add(incoming.conversations_accepted);
    target.conversations_rejected = target
        .conversations_rejected
        .saturating_add(incoming.conversations_rejected);
    target.variants_accepted.extend(incoming.variants_accepted);
    target.variants_rejected.extend(incoming.variants_rejected);
}

fn result_from_usage(
    request: &CollectionRequest,
    started: Instant,
    started_at: DateTime<Utc>,
    usage: Vec<ConversationUsage>,
    calculator: &BurnlyCostCalculator,
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
            let candidates =
                mapper::map_daily(usage, timezone, request.scope(), &context, calculator)
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
            let candidates = mapper::map_sessions(usage, &context, calculator)
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
    let collector = collector_key()?;
    Ok(CollectorDescriptor {
        collector: collector.clone(),
        display_name: DISPLAY_NAME.to_owned(),
        runtime_version: COLLECTOR_VERSION.to_owned(),
        expected_version: COLLECTOR_VERSION.to_owned(),
        adapter_version: ADAPTER_VERSION,
        binary_target: std::env::consts::OS.to_owned(),
        integrity: CollectorIntegrity::UnverifiedDevelopment,
        profiles: vec![ProfileDescriptor {
            collector,
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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
    use crate::infrastructure::database::{Database, SqliteAntigravityUsageCacheStore};

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
    #[ignore = "set BURNLY_ANTIGRAVITY_EVIDENCE_LEDGER to run against local Antigravity data"]
    fn runtime_evidence_collects_default_location_without_incomplete_baseline() {
        let ledger_path = std::env::var_os("BURNLY_ANTIGRAVITY_EVIDENCE_LEDGER")
            .map(PathBuf::from)
            .expect("BURNLY_ANTIGRAVITY_EVIDENCE_LEDGER must name a disposable database");
        let mut cache_database = Database::open(&ledger_path).expect("open evidence cache");
        cache_database
            .migrate_to_latest()
            .expect("migrate evidence cache");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = AntigravityCollector::with_diagnostic_recorder(
            diagnostics.clone(),
            Arc::new(SqliteAntigravityUsageCacheStore::new(cache_database)),
        );
        let requested_at = Utc::now();

        let daily = collector
            .collect(
                support_daily_request(
                    "antigravity-runtime-evidence-daily",
                    SourceKey::Antigravity,
                    CollectionScope::Full,
                    "Asia/Jakarta",
                    requested_at,
                ),
                &NeverCancelled,
            )
            .expect("full daily collection");
        let sessions = collector
            .collect(
                support_session_request(
                    "antigravity-runtime-evidence-session",
                    SourceKey::Antigravity,
                    requested_at,
                ),
                &NeverCancelled,
            )
            .expect("full session collection");
        let events = diagnostics.events();

        assert!(!daily.daily_candidates().is_empty());
        assert!(!sessions.session_candidates().is_empty());
        assert!(!events
            .iter()
            .any(|event| { event.code.as_str() == "antigravity.full_reconciliation_incomplete" }));
        println!(
            "antigravity_runtime_evidence=v1 daily_outcome={:?} days={} daily_tokens={} session_outcome={:?} sessions={} session_tokens={} diagnostics={:?}",
            daily.outcome(),
            daily.daily_candidates().len(),
            daily
                .daily_candidates()
                .iter()
                .map(|candidate| candidate.tokens.total_tokens())
                .sum::<u64>(),
            sessions.outcome(),
            sessions.session_candidates().len(),
            sessions
                .session_candidates()
                .iter()
                .map(|candidate| candidate.tokens.total_tokens())
                .sum::<u64>(),
            events
                .iter()
                .map(|event| event.code.as_str())
                .collect::<Vec<_>>()
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
        let data_root = TempDir::new().expect("tempdir");
        create_db(
            data_root.path(),
            AntigravityProductVariant::App,
            "conversation",
        );
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(Vec::new()),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Current(RuntimeClient::new()),
            noop_usage_cache_client(),
        );

        let error = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect_err("missing runtime");

        assert_eq!(error.code, CollectorFailureCode::SourceNotFound);
    }

    #[test]
    fn records_diagnostic_when_runtime_endpoint_is_missing() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let data_root = TempDir::new().expect("tempdir");
        create_db(
            data_root.path(),
            AntigravityProductVariant::App,
            "conversation",
        );
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(Vec::new()),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Current(RuntimeClient::new()),
            noop_usage_cache_client(),
        )
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
        assert!(context.contains(r#""metadataCallsAttempted":0"#));
        assert!(context.contains(r#""failureCode":"source.not_found""#));
        assert!(context.contains(r#""failureReason":"no_matching_runtime_endpoint""#));
    }

    #[test]
    fn uses_cached_usage_when_runtime_metadata_is_unavailable() {
        use crate::infrastructure::collectors::antigravity::usage_cache::tests::{
            cached_record, RecordingUsageCache,
        };

        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let cache = RecordingUsageCache::default().seed(vec![cached_record(
            AntigravityProductVariant::App,
            "conversation",
            "response-cached",
            55,
            11,
        )]);
        let (_directory, collector) = collector_with_conversation_and_runtime(
            AntigravityProductVariant::App,
            AntigravityProductVariant::App,
            RuntimeUsageSource::Failing(
                AntigravityRuntimeCollectionFailureReason::RuntimeMetadataUnavailable,
            ),
        );
        let collector = collector
            .with_usage_cache(AntigravityUsageCacheClient::new(Arc::new(cache)))
            .with_test_diagnostics(diagnostics.clone());

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("cached collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        assert_eq!(result.daily_candidates()[0].tokens.input_tokens(), Some(55));
        let events = diagnostics.events();
        assert!(events
            .iter()
            .any(|event| event.code.as_str() == "antigravity.cache_used"));
        assert!(events
            .iter()
            .any(|event| { event.code.as_str() == "antigravity.collection_completed" }));
    }

    #[test]
    fn upserts_cache_after_successful_runtime_collection() {
        use crate::infrastructure::collectors::antigravity::usage_cache::tests::RecordingUsageCache;

        let cache = Arc::new(RecordingUsageCache::default());
        let (_directory, collector) = collector_with_usage();
        let collector = collector.with_usage_cache(AntigravityUsageCacheClient::new(cache.clone()));

        collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("collection");

        assert!(!cache.upserts().is_empty());
    }

    #[test]
    fn records_diagnostic_when_runtime_metadata_is_unavailable() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (_directory, collector) = collector_with_conversation_and_runtime(
            AntigravityProductVariant::App,
            AntigravityProductVariant::App,
            RuntimeUsageSource::Failing(
                AntigravityRuntimeCollectionFailureReason::RuntimeMetadataUnavailable,
            ),
        );
        let collector = collector.with_test_diagnostics(diagnostics.clone());

        let error = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect_err("metadata unavailable");

        assert_eq!(error.code, CollectorFailureCode::SourceNotFound);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].code.as_str(),
            "antigravity.metadata_rpc_unavailable"
        );
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""endpointsFound":1"#));
        assert!(context.contains(r#""conversationArtifactsFound":1"#));
        assert!(context.contains(r#""failureCode":"source.not_found""#));
        assert!(context.contains(r#""failureReason":"metadata_rpc_unavailable""#));
    }

    #[test]
    fn collects_app_usage_from_sqlite_fallback_without_runtime() {
        use rusqlite::params;
        use rusqlite::Connection;

        use crate::infrastructure::collectors::antigravity::protobuf_usage::tests::{
            sample_gen_metadata_blob, sample_trajectory_metadata_blob,
        };

        let data_root = TempDir::new().expect("tempdir");
        let path = data_root
            .path()
            .join("antigravity/conversations/app-session.db");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        let connection = Connection::open(&path).expect("open database");
        connection
            .execute_batch(
                "CREATE TABLE gen_metadata (idx integer, data blob, size integer);
                 CREATE TABLE trajectory_metadata_blob (id text, data blob);",
            )
            .expect("schema");
        connection
            .execute(
                "INSERT INTO gen_metadata (idx, data, size) VALUES (0, ?1, 0)",
                params![sample_gen_metadata_blob("response-app")],
            )
            .expect("insert gen_metadata");
        connection
            .execute(
                "INSERT INTO trajectory_metadata_blob (id, data) VALUES ('main', ?1)",
                params![sample_trajectory_metadata_blob()],
            )
            .expect("insert trajectory metadata");

        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(Vec::new()),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(Vec::new()),
            noop_usage_cache_client(),
        );

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("sqlite fallback collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        assert_eq!(
            result.daily_candidates()[0].tokens.input_tokens(),
            Some(150)
        );
    }

    #[test]
    fn full_cli_reconciliation_routes_protobuf_artifacts_away_from_sqlite() {
        use rusqlite::params;
        use rusqlite::Connection;

        use crate::infrastructure::collectors::antigravity::protobuf_usage::tests::{
            sample_gen_metadata_blob, sample_trajectory_metadata_blob,
        };

        let data_root = TempDir::new().expect("tempdir");
        let conversations = data_root.path().join("antigravity-cli/conversations");
        fs::create_dir_all(&conversations).expect("conversation directory");
        let database_path = conversations.join("sqlite-conversation.db");
        let connection = Connection::open(&database_path).expect("open database");
        connection
            .execute_batch(
                "CREATE TABLE gen_metadata (idx integer, data blob, size integer);
                 CREATE TABLE trajectory_metadata_blob (id text, data blob);",
            )
            .expect("schema");
        connection
            .execute(
                "INSERT INTO gen_metadata (idx, data, size) VALUES (0, ?1, 0)",
                params![sample_gen_metadata_blob("response-cli")],
            )
            .expect("insert gen_metadata");
        connection
            .execute(
                "INSERT INTO trajectory_metadata_blob (id, data) VALUES ('main', ?1)",
                params![sample_trajectory_metadata_blob()],
            )
            .expect("insert trajectory metadata");
        drop(connection);
        fs::write(
            conversations.join("legacy-conversation.pb"),
            b"legacy protobuf artifact",
        )
        .expect("protobuf artifact");

        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(Vec::new()),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(Vec::new()),
            noop_usage_cache_client(),
        );

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("full CLI collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
    }

    #[test]
    fn uses_runtime_after_app_sqlite_fallback_rejects_schema_mismatch() {
        let data_root = TempDir::new().expect("tempdir");
        create_db(
            data_root.path(),
            AntigravityProductVariant::App,
            "app-conversation",
        );
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(vec![process_for_variant(
                AntigravityProductVariant::App,
            )]),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(fixed_usage()),
            noop_usage_cache_client(),
        );

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("runtime collection after fallback rejection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        assert_eq!(
            result.daily_candidates()[0].tokens.input_tokens(),
            Some(180)
        );
    }

    #[test]
    fn collects_cli_usage_from_sqlite_without_runtime() {
        use rusqlite::params;
        use rusqlite::Connection;

        use crate::infrastructure::collectors::antigravity::protobuf_usage::tests::{
            sample_gen_metadata_blob, sample_trajectory_metadata_blob,
        };

        let data_root = TempDir::new().expect("tempdir");
        let path = data_root
            .path()
            .join("antigravity-cli/conversations/cli-session.db");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        let connection = Connection::open(&path).expect("open database");
        connection
            .execute_batch(
                "CREATE TABLE gen_metadata (idx integer, data blob, size integer);
                 CREATE TABLE trajectory_metadata_blob (id text, data blob);",
            )
            .expect("schema");
        connection
            .execute(
                "INSERT INTO gen_metadata (idx, data, size) VALUES (0, ?1, 0)",
                params![sample_gen_metadata_blob("response-cli")],
            )
            .expect("insert gen_metadata");
        connection
            .execute(
                "INSERT INTO trajectory_metadata_blob (id, data) VALUES ('main', ?1)",
                params![sample_trajectory_metadata_blob()],
            )
            .expect("insert trajectory metadata");

        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(Vec::new()),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(Vec::new()),
            noop_usage_cache_client(),
        );

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("sqlite collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        assert_eq!(
            result.daily_candidates()[0].tokens.input_tokens(),
            Some(150)
        );
        assert_eq!(
            result.daily_candidates()[0].tokens.output_tokens(),
            Some(25)
        );
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
        assert!(context.contains(r#""metadataCallsSucceeded":2"#));
    }

    #[test]
    fn continues_collection_when_one_metadata_fetch_fails() {
        use std::io::{Read, Write};
        use std::net::{Ipv4Addr, TcpListener};
        use std::sync::{Mutex, MutexGuard};
        use std::thread;

        static LOCK: Mutex<()> = Mutex::new(());

        fn metadata_test_lock() -> MutexGuard<'static, ()> {
            LOCK.lock().expect("metadata integration test lock")
        }

        fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            let mut content_length = None;
            loop {
                let read = stream.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
                else {
                    continue;
                };
                if content_length.is_none() {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    content_length = headers.lines().find_map(|line| {
                        line.strip_prefix("Content-Length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    });
                }
                let expected_length = header_end + 4 + content_length.unwrap_or(0);
                if request.len() >= expected_length {
                    break;
                }
            }
            request
        }

        let _guard = metadata_test_lock();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("listener");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            for _ in 0..3 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let request_bytes = read_http_request(&mut stream);
                let request = String::from_utf8_lossy(&request_bytes);
                let response = if request.contains("GetAllCascadeTrajectories") {
                    let body = br#"{"trajectorySummaries":{"conversation-fail":{"stepCount":1},"conversation-ok":{"stepCount":1}}}"#;
                    [
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len())
                            .into_bytes(),
                        body.to_vec(),
                    ]
                    .concat()
                } else if request.contains("conversation-fail") {
                    b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n".to_vec()
                } else {
                    let body = br#"{"generatorMetadata":[{"chatModel":{"model":"gemini","modelDisplayName":"Gemini Flash","usage":{"model":"gemini","inputTokens":"25","outputTokens":"5","responseId":"response-ok"}}}]}"#;
                    [
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len())
                            .into_bytes(),
                        body.to_vec(),
                    ]
                    .concat()
                };
                stream.write_all(&response).expect("write response");
            }
        });

        let data_root = TempDir::new().expect("tempdir");
        create_db(
            data_root.path(),
            AntigravityProductVariant::App,
            "conversation-fail",
        );
        create_db(
            data_root.path(),
            AntigravityProductVariant::App,
            "conversation-ok",
        );
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
                10,
                Some(PathBuf::from(
                    "/opt/antigravity/Antigravity-x64/language_server",
                )),
                vec![
                    "language_server".to_owned(),
                    "--override_ide_name".to_owned(),
                    "antigravity".to_owned(),
                ],
                vec![LocalListener::ipv4(port)],
            )]),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Current(RuntimeClient::new()),
            noop_usage_cache_client(),
        );

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("collection");

        handle.join().expect("server thread");
        assert_eq!(result.daily_candidates().len(), 1);
        assert_eq!(result.daily_candidates()[0].tokens.input_tokens(), Some(25));
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
        let data_root = TempDir::new().expect("tempdir");
        create_db(
            data_root.path(),
            AntigravityProductVariant::Cli,
            "conversation",
        );
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
                10,
                Some(PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::ipv4(34415)],
            )]),
            EndpointValidationSource::RejectAll,
            RuntimeUsageSource::Current(RuntimeClient::new()),
            noop_usage_cache_client(),
        )
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
            noop_usage_cache_client(),
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
            noop_usage_cache_client(),
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
            noop_usage_cache_client(),
        );
        (data_root, collector)
    }

    fn noop_usage_cache_client() -> AntigravityUsageCacheClient {
        AntigravityUsageCacheClient::new(Arc::new(NoOpAntigravityUsageCache))
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

    #[test]
    fn full_scope_keeps_every_conversation_while_incremental_scope_remains_bounded() {
        let conversations = (0..125)
            .map(|index| {
                database(
                    AntigravityProductVariant::Cli,
                    &format!("conversation-{index:03}"),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            bounded_conversations(conversations.clone(), &CollectionScope::Full).len(),
            125
        );
        assert_eq!(
            bounded_conversations(
                conversations,
                &CollectionScope::incremental(
                    chrono::NaiveDate::from_ymd_opt(2026, 8, 22).expect("date"),
                    chrono::NaiveDate::from_ymd_opt(2026, 8, 22).expect("date"),
                )
                .expect("scope"),
            )
            .len(),
            MAX_INCREMENTAL_CONVERSATIONS
        );
    }

    #[test]
    fn incomplete_full_cli_scan_cannot_establish_a_compatible_baseline() {
        let incomplete = CliSqliteCollectionReport {
            records_rejected: 1,
            ..CliSqliteCollectionReport::default()
        };

        assert!(full_cli_scan_is_incomplete(
            &CollectionScope::Full,
            &incomplete
        ));
        assert!(!full_cli_scan_is_incomplete(
            &CollectionScope::incremental(
                chrono::NaiveDate::from_ymd_opt(2026, 8, 22).expect("date"),
                chrono::NaiveDate::from_ymd_opt(2026, 8, 22).expect("date"),
            )
            .expect("scope"),
            &incomplete
        ));
        assert!(!full_cli_scan_is_incomplete(
            &CollectionScope::Full,
            &CliSqliteCollectionReport::default()
        ));
    }

    struct CancelBeforeSecondBatch {
        checks: AtomicUsize,
    }

    impl CancellationSignal for CancelBeforeSecondBatch {
        fn is_cancelled(&self) -> bool {
            self.checks.fetch_add(1, Ordering::SeqCst) >= 2
        }
    }

    #[test]
    fn full_reconciliation_checks_cancellation_between_conversation_batches() {
        let data_root = TempDir::new().expect("tempdir");
        for index in 0..=CONVERSATION_BATCH_SIZE {
            create_db(
                data_root.path(),
                AntigravityProductVariant::Cli,
                &format!("conversation-{index:03}"),
            );
        }
        let collector = AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            RuntimeDiscovery::from_processes(Vec::new()),
            EndpointValidationSource::Passthrough,
            RuntimeUsageSource::Fixed(Vec::new()),
            noop_usage_cache_client(),
        );

        let error = collector
            .collect(
                daily_request(SourceKey::Antigravity),
                &CancelBeforeSecondBatch {
                    checks: AtomicUsize::new(0),
                },
            )
            .expect_err("second batch must observe cancellation");

        assert_eq!(error.code, CollectorFailureCode::Cancelled);
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
            source_record_index: None,
            observed_at: None,
            timestamp_origin: crate::application::ports::antigravity_usage_cache::AntigravityTimestampOrigin::Unresolved,
            legacy_fallback_at: None,
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
