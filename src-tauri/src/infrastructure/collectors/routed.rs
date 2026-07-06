use std::sync::Arc;

use crate::application::collection::{
    CollectionRequest, CollectionResult, CollectorDescriptor, CollectorFailure, DetectionRequest,
    DetectionResult,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;

#[derive(Clone)]
pub(crate) struct RoutedCollector {
    ccusage: Arc<dyn Collector>,
    cline: Arc<dyn Collector>,
    zcode: Arc<dyn Collector>,
    antigravity: Arc<dyn Collector>,
}

impl RoutedCollector {
    pub(crate) fn new(
        ccusage: Arc<dyn Collector>,
        cline: Arc<dyn Collector>,
        zcode: Arc<dyn Collector>,
        antigravity: Arc<dyn Collector>,
    ) -> Self {
        Self {
            ccusage,
            cline,
            zcode,
            antigravity,
        }
    }

    fn collector_for(&self, source: SourceKey) -> Result<&Arc<dyn Collector>, CollectorFailure> {
        match source {
            SourceKey::ClaudeCode | SourceKey::Codex | SourceKey::OpenCode | SourceKey::Pi => {
                Ok(&self.ccusage)
            }
            SourceKey::Cline => Ok(&self.cline),
            SourceKey::ZCode => Ok(&self.zcode),
            SourceKey::Antigravity => Ok(&self.antigravity),
            SourceKey::GrokBuild => Err(CollectorFailure::new(
                crate::application::collection::CollectorFailureCode::UnsupportedSource,
                Some(source),
                None,
            )),
            #[cfg(test)]
            SourceKey::TestUnsupported => Err(CollectorFailure::new(
                crate::application::collection::CollectorFailureCode::UnsupportedSource,
                Some(source),
                None,
            )),
        }
    }
}

impl Collector for RoutedCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        let mut descriptor = self.ccusage.describe()?;
        descriptor.profiles.extend(self.cline.describe()?.profiles);
        descriptor.profiles.extend(self.zcode.describe()?.profiles);
        descriptor
            .profiles
            .extend(self.antigravity.describe()?.profiles);
        Ok(descriptor)
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        self.collector_for(request.source)?
            .detect(request, cancellation)
    }

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        self.collector_for(request.source())?
            .collect(request, cancellation)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionScope,
        CollectorFailureCode, CollectorIntegrity, CollectorKey, DailyUsageCandidate,
        ModelUsageCandidate, ProcessSummary, ProfileDescriptor,
    };
    use crate::domain::usage::{CostKind, CurrencyCode, TokenUsage, UsageCost, ValuedCostStatus};

    #[test]
    fn routes_collection_by_source() {
        let ccusage = Arc::new(RecordingCollector::new("ccusage"));
        let cline = Arc::new(RecordingCollector::new("cline"));
        let zcode = Arc::new(RecordingCollector::new("zcode"));
        let antigravity = Arc::new(RecordingCollector::new("antigravity"));
        let collector = RoutedCollector::new(
            ccusage.clone(),
            cline.clone(),
            zcode.clone(),
            antigravity.clone(),
        );

        collector
            .collect(request(SourceKey::ClaudeCode), &NeverCancelled)
            .expect("claude-code collection");
        collector
            .collect(request(SourceKey::Codex), &NeverCancelled)
            .expect("codex collection");
        collector
            .collect(request(SourceKey::OpenCode), &NeverCancelled)
            .expect("opencode collection");
        collector
            .collect(request(SourceKey::Pi), &NeverCancelled)
            .expect("pi collection");
        collector
            .collect(request(SourceKey::Cline), &NeverCancelled)
            .expect("cline collection");
        collector
            .collect(request(SourceKey::ZCode), &NeverCancelled)
            .expect("zcode collection");
        collector
            .collect(request(SourceKey::Antigravity), &NeverCancelled)
            .expect("antigravity collection");

        assert_eq!(
            ccusage.sources(),
            vec![
                SourceKey::ClaudeCode,
                SourceKey::Codex,
                SourceKey::OpenCode,
                SourceKey::Pi
            ]
        );
        assert_eq!(cline.sources(), vec![SourceKey::Cline]);
        assert_eq!(zcode.sources(), vec![SourceKey::ZCode]);
        assert_eq!(antigravity.sources(), vec![SourceKey::Antigravity]);
    }

    #[test]
    fn grok_build_fails_closed_until_native_collector_is_wired() {
        let collector = RoutedCollector::new(
            Arc::new(RecordingCollector::new("ccusage")),
            Arc::new(RecordingCollector::new("cline")),
            Arc::new(RecordingCollector::new("zcode")),
            Arc::new(RecordingCollector::new("antigravity")),
        );

        let failure = collector
            .collect(request(SourceKey::GrokBuild), &NeverCancelled)
            .expect_err("grok-build should fail closed");

        assert_eq!(failure.code, CollectorFailureCode::UnsupportedSource);
        assert_eq!(failure.source_key, Some(SourceKey::GrokBuild));
    }

    #[test]
    fn aggregates_descriptors_from_all_wired_collectors() {
        let collector = RoutedCollector::new(
            Arc::new(RecordingCollector::new("ccusage")),
            Arc::new(RecordingCollector::new("cline")),
            Arc::new(RecordingCollector::new("zcode")),
            Arc::new(RecordingCollector::new("antigravity")),
        );

        let descriptor = collector.describe().expect("descriptor");
        let sources = descriptor
            .profiles
            .iter()
            .map(|profile| profile.source)
            .collect::<Vec<_>>();

        assert_eq!(
            sources,
            vec![
                SourceKey::ClaudeCode,
                SourceKey::Codex,
                SourceKey::OpenCode,
                SourceKey::Pi,
                SourceKey::Cline,
                SourceKey::ZCode,
                SourceKey::Antigravity
            ]
        );
    }

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    struct RecordingCollector {
        key: &'static str,
        sources: Mutex<Vec<SourceKey>>,
    }

    impl RecordingCollector {
        fn new(key: &'static str) -> Self {
            Self {
                key,
                sources: Mutex::new(Vec::new()),
            }
        }

        fn sources(&self) -> Vec<SourceKey> {
            self.sources.lock().expect("sources").clone()
        }
    }

    impl Collector for RecordingCollector {
        fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
            Ok(CollectorDescriptor {
                collector: CollectorKey::new(self.key).expect("collector key"),
                display_name: self.key.to_owned(),
                runtime_version: "test".to_owned(),
                expected_version: "test".to_owned(),
                adapter_version: 1,
                binary_target: "test".to_owned(),
                integrity: CollectorIntegrity::UnverifiedDevelopment,
                profiles: profiles_for(self.key),
            })
        }

        fn detect(
            &self,
            request: DetectionRequest,
            _cancellation: &dyn CancellationSignal,
        ) -> Result<DetectionResult, CollectorFailure> {
            self.sources.lock().expect("sources").push(request.source);
            Ok(DetectionResult {
                source: request.source,
                state: crate::application::collection::DetectionState::Available,
                supported_projections: vec![CollectionProjection::Daily],
                data_roots_found: 1,
                usage_artifacts_found: true,
                checked_at: request.requested_at,
                issues: Vec::new(),
            })
        }

        fn collect(
            &self,
            request: CollectionRequest,
            _cancellation: &dyn CancellationSignal,
        ) -> Result<CollectionResult, CollectorFailure> {
            self.sources.lock().expect("sources").push(request.source());
            CollectionResult::daily(
                metadata(&request, self.key),
                vec![daily_candidate(&request)],
                Vec::new(),
                Vec::new(),
                process_summary(),
            )
            .map_err(|_| CollectorFailure::new(CollectorFailureCode::Internal, None, None))
        }
    }

    fn request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new(format!("{}-daily", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            "UTC",
            Utc.with_ymd_and_hms(2026, 6, 30, 12, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("request")
    }

    fn profiles_for(key: &str) -> Vec<ProfileDescriptor> {
        match key {
            "ccusage" => vec![
                profile(SourceKey::ClaudeCode),
                profile(SourceKey::Codex),
                profile(SourceKey::OpenCode),
                profile(SourceKey::Pi),
            ],
            "cline" => vec![profile(SourceKey::Cline)],
            "zcode" => vec![profile(SourceKey::ZCode)],
            "antigravity" => vec![profile(SourceKey::Antigravity)],
            _ => Vec::new(),
        }
    }

    fn profile(source: SourceKey) -> ProfileDescriptor {
        ProfileDescriptor {
            source,
            profile_version: 1,
            supported_projections: vec![CollectionProjection::Daily, CollectionProjection::Session],
        }
    }

    fn metadata(request: &CollectionRequest, collector: &str) -> CollectionMetadata {
        CollectionMetadata::new(
            request.collection_id().clone(),
            CollectorKey::new(collector).expect("collector key"),
            "test".to_owned(),
            request.source(),
            request.scope().clone(),
            1,
            CollectionPeriod {
                started_at: *request.requested_at(),
                finished_at: *request.requested_at(),
            },
        )
        .expect("metadata")
    }

    fn daily_candidate(request: &CollectionRequest) -> DailyUsageCandidate {
        let tokens = TokenUsage::new(Some(1), Some(0), Some(0), Some(0), 1).expect("tokens");
        let cost = UsageCost::Valued {
            amount_micros: 1,
            currency: CurrencyCode::new("USD").expect("currency"),
            kind: CostKind::CollectorCalculated,
            status: ValuedCostStatus::Estimated,
        };
        DailyUsageCandidate {
            provenance: crate::application::collection::CandidateProvenance {
                source: request.source(),
                collector: CollectorKey::new("test").expect("collector key"),
                collector_version: "test".to_owned(),
                profile_version: 1,
                collection_id: request.collection_id().clone(),
                observed_at: *request.requested_at(),
                data_quality: crate::domain::usage::DataQuality::Complete,
                warnings: Vec::new(),
            },
            source_key: format!("{}:daily:v1:UTC:2026-06-30", request.source().as_str()),
            usage_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).expect("date"),
            aggregation_timezone: "UTC".to_owned(),
            tokens: tokens.clone(),
            cost: cost.clone(),
            model_breakdowns: vec![ModelUsageCandidate {
                raw_model_id: "test".to_owned(),
                tokens,
                cost,
            }],
        }
    }

    fn process_summary() -> ProcessSummary {
        ProcessSummary {
            runtime_ms: 0,
            stdout_bytes: 0,
            stderr_bytes: 0,
            exit_code: None,
        }
    }
}
