use std::time::Instant;

use chrono::{DateTime, Utc};

use crate::application::collection::{
    CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionRequest,
    CollectionResult, CollectorDescriptor, CollectorFailure, CollectorFailureCode,
    CollectorIntegrity, CollectorKey, DetectionIssue, DetectionRequest, DetectionResult,
    DetectionState, ProcessSummary, ProfileDescriptor,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;

#[cfg(test)]
use super::discovery::{LocalListener, ProcessSnapshot};
use super::mapper::{self, AntigravityMappingContext, ConversationUsage};
use super::{
    extract_usage_records, ConversationDatabase, ConversationIndex, RuntimeClient,
    RuntimeClientError, RuntimeDiscovery, RuntimeEndpoint,
};

const COLLECTOR_KEY: &str = "antigravity";
const DISPLAY_NAME: &str = "Antigravity";
const COLLECTOR_VERSION: &str = "local-rpc";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;

#[derive(Debug, Clone)]
pub(crate) struct AntigravityCollector {
    conversation_index: ConversationIndex,
    runtime_discovery: RuntimeDiscoverySource,
    runtime_usage: RuntimeUsageSource,
}

impl AntigravityCollector {
    pub(crate) fn new() -> Self {
        Self {
            conversation_index: ConversationIndex::default(),
            runtime_discovery: RuntimeDiscoverySource::Current,
            runtime_usage: RuntimeUsageSource::Current(RuntimeClient::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn empty_from_data_root(data_root: impl Into<std::path::PathBuf>) -> Self {
        Self::from_parts(
            ConversationIndex::from_data_root(data_root),
            RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
                10,
                Some(std::path::PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::ipv4(34415)],
            )]),
            RuntimeUsageSource::Fixed(Vec::new()),
        )
    }

    #[cfg(test)]
    fn from_parts(
        conversation_index: ConversationIndex,
        runtime_discovery: RuntimeDiscovery,
        runtime_usage: RuntimeUsageSource,
    ) -> Self {
        Self {
            conversation_index,
            runtime_discovery: RuntimeDiscoverySource::Fixed(runtime_discovery),
            runtime_usage,
        }
    }
}

#[derive(Debug, Clone)]
enum RuntimeDiscoverySource {
    Current,
    #[cfg(test)]
    Fixed(RuntimeDiscovery),
}

impl RuntimeDiscoverySource {
    fn discover(&self) -> Vec<RuntimeEndpoint> {
        match self {
            Self::Current => RuntimeDiscovery::current().discover(),
            #[cfg(test)]
            Self::Fixed(discovery) => discovery.discover(),
        }
    }
}

#[derive(Debug, Clone)]
enum RuntimeUsageSource {
    Current(RuntimeClient),
    #[cfg(test)]
    Fixed(Vec<ConversationUsage>),
}

impl RuntimeUsageSource {
    fn collect(
        &self,
        endpoints: &[RuntimeEndpoint],
        conversations: &[ConversationDatabase],
    ) -> Result<Vec<ConversationUsage>, RuntimeClientError> {
        match self {
            Self::Current(client) => collect_runtime_usage(client, endpoints, conversations),
            #[cfg(test)]
            Self::Fixed(usage) => Ok(usage.clone()),
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

        let endpoints = self.runtime_discovery.discover();
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
                "antigravity.runtime_unavailable",
                "Antigravity local runtime endpoint was not found.",
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

        let endpoints = self.runtime_discovery.discover();
        if endpoints.is_empty() {
            return Err(failure(&request, CollectorFailureCode::SourceNotFound));
        }
        let conversations = self
            .conversation_index
            .list(
                request.scope(),
                request.aggregation_timezone().unwrap_or("UTC"),
            )
            .map_err(|_| failure(&request, CollectorFailureCode::ScopeNotRepresentable))?;
        let conversations = bounded_conversations(conversations);
        if conversations.is_empty() {
            return empty_result(&request, started, started_at);
        }
        let usage = self
            .runtime_usage
            .collect(&endpoints, &conversations)
            .map_err(|_| failure(&request, CollectorFailureCode::SourceNotFound))?;
        if cancellation.is_cancelled() {
            return Err(failure(&request, CollectorFailureCode::Cancelled));
        }

        result_from_usage(&request, started, started_at, usage)
    }
}

fn collect_runtime_usage(
    client: &RuntimeClient,
    endpoints: &[RuntimeEndpoint],
    conversations: &[ConversationDatabase],
) -> Result<Vec<ConversationUsage>, RuntimeClientError> {
    let mut collected = Vec::new();
    let mut attempted = false;
    let mut successful_streams = 0_u32;
    for conversation in conversations {
        let mut records = Vec::new();
        for endpoint in endpoints
            .iter()
            .filter(|endpoint| endpoint.variant == conversation.variant)
        {
            attempted = true;
            let frames =
                match client.stream_agent_state_updates(endpoint, &conversation.conversation_id) {
                    Ok(frames) => frames,
                    Err(_) => continue,
                };
            successful_streams = successful_streams.saturating_add(1);
            let Ok(mut extracted) =
                extract_usage_records(conversation.variant, &conversation.conversation_id, &frames)
            else {
                continue;
            };
            records.append(&mut extracted);
        }
        let records = dedupe_records(records);
        if !records.is_empty() {
            collected.push(ConversationUsage {
                database: conversation.clone(),
                records,
            });
        }
    }
    if !attempted {
        return Err(RuntimeClientError::ConnectionFailed);
    }
    if successful_streams == 0 {
        return Err(RuntimeClientError::ConnectionFailed);
    }
    Ok(collected)
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

    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionReason,
    };
    use crate::infrastructure::collectors::antigravity::discovery::{
        LocalListener, ProcessSnapshot,
    };
    use crate::infrastructure::collectors::antigravity::product_variant::AntigravityProductVariant;

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
        assert_eq!(result.issues[0].code, "antigravity.runtime_unavailable");
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

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    fn collector_with_discovery(runtime_discovery: RuntimeDiscovery) -> AntigravityCollector {
        let data_root = TempDir::new().expect("tempdir");
        AntigravityCollector::from_parts(
            ConversationIndex::from_data_root(data_root.path()),
            runtime_discovery,
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
            RuntimeUsageSource::Fixed(fixed_usage()),
        );
        (data_root, collector)
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
            modified_at: Utc
                .with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
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
        DetectionRequest {
            source,
            reason: DetectionReason::Startup,
            requested_at: Utc
                .with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
        }
    }

    fn daily_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new(format!("{}-daily", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            "UTC",
            Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("request")
    }

    fn session_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new(format!("{}-session", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
        )
    }
}
