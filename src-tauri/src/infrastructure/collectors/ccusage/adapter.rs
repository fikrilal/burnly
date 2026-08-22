use chrono::{DateTime, Utc};

use crate::{
    application::{
        collection::{
            CollectionMetadata, CollectionPeriod, CollectionRequest, CollectionResult,
            CollectorDescriptor, CollectorFailure, CollectorFailureCode, DetectionIssue,
            DetectionRequest, DetectionResult, DetectionState, ProcessSummary,
        },
        ports::collector::{CancellationSignal, Collector},
    },
    domain::source::SourceKey,
};

use super::{
    capability_profiles::profile_for,
    command::prepare_collection,
    envelopes::{
        claude_daily::decode as decode_daily, claude_session::decode as decode_session,
        codex_daily::decode as decode_codex_daily, codex_session::decode as decode_codex_session,
        pi_daily::decode as decode_pi_daily, pi_session::decode as decode_pi_session,
    },
    manifest::{development_manifest, BinaryTarget},
    mapper::{
        map_codex_daily, map_codex_session, map_daily, map_pi_daily, map_pi_session, map_session,
        MappingContext, MappingError,
    },
    process::{execute, ProcessLimits, ProcessOutput},
    sidecar::{verify, SidecarLocation, VerifiedSidecar},
};

#[derive(Debug, Clone)]
pub(crate) struct CcusageCollector {
    target: BinaryTarget,
    manifest: super::manifest::SidecarManifest,
    location: SidecarLocation,
    limits: ProcessLimits,
}

impl CcusageCollector {
    pub(crate) fn development(
        binary: impl Into<std::path::PathBuf>,
    ) -> Result<Self, CollectorFailure> {
        Ok(Self {
            target: current_target()?,
            manifest: development_manifest().clone(),
            location: SidecarLocation::DevelopmentBinary(binary.into()),
            limits: ProcessLimits::collection(),
        })
    }

    pub(crate) fn packaged(
        resource_directory: impl Into<std::path::PathBuf>,
    ) -> Result<Self, CollectorFailure> {
        let resource_directory = resource_directory.into();
        let manifest_path = resource_directory
            .join("sidecars")
            .join("ccusage")
            .join("manifest.json");
        let manifest_json = std::fs::read_to_string(manifest_path)
            .map_err(|_| failure(CollectorFailureCode::BinaryMissing))?;
        let manifest = super::manifest::SidecarManifest::parse(&manifest_json)
            .map_err(|_| failure(CollectorFailureCode::VersionMismatch))?;
        Ok(Self {
            target: current_target()?,
            manifest,
            location: SidecarLocation::PackagedResourceDirectory(resource_directory),
            limits: ProcessLimits::collection(),
        })
    }

    #[cfg(test)]
    fn with_limits(mut self, limits: ProcessLimits) -> Self {
        self.limits = limits;
        self
    }

    fn verify(
        &self,
        cancellation: &dyn CancellationSignal,
    ) -> Result<VerifiedSidecar, CollectorFailure> {
        verify(
            &self.manifest,
            self.target,
            self.location.clone(),
            cancellation,
        )
    }
}

impl Collector for CcusageCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        self.verify(&ActiveCancellation)
            .map(|sidecar| sidecar.descriptor)
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        let checked_at = Utc::now();
        if request.source != SourceKey::ClaudeCode
            && request.source != SourceKey::Codex
            && request.source != SourceKey::Pi
        {
            return Ok(DetectionResult {
                source: request.source,
                state: DetectionState::Unsupported,
                supported_projections: Vec::new(),
                data_roots_found: 0,
                usage_artifacts_found: false,
                checked_at,
                issues: vec![DetectionIssue {
                    code: CollectorFailureCode::UnsupportedSource.code().to_owned(),
                    message: "The requested source is unsupported by this collector.".to_owned(),
                }],
            });
        }
        let descriptor = self.verify(cancellation)?.descriptor;
        let supported_projections = descriptor
            .profiles
            .iter()
            .find(|profile| profile.source == request.source)
            .map(|profile| profile.supported_projections.clone())
            .unwrap_or_default();
        Ok(DetectionResult {
            source: request.source,
            state: DetectionState::Available,
            supported_projections,
            data_roots_found: 0,
            usage_artifacts_found: false,
            checked_at,
            issues: Vec::new(),
        })
    }

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        let started_at = Utc::now();
        profile_for(request.source(), request.projection())?;
        let sidecar = self.verify(cancellation)?;
        let prepared = prepare_collection(&sidecar.executable, &request)?;
        let output = execute(prepared.process(), cancellation, self.limits)?;
        let finished_at = Utc::now();
        let metadata = metadata(&request, &sidecar.descriptor, started_at, finished_at)?;
        let timezone = request.aggregation_timezone().unwrap_or("UTC");
        let context = MappingContext::new(
            request.source(),
            sidecar.descriptor.collector.clone(),
            sidecar.descriptor.runtime_version.clone(),
            profile_version(&sidecar.descriptor, request.source())?,
            request.collection_id().clone(),
            finished_at,
            timezone.to_owned(),
        )
        .map_err(mapping_failure)?;

        match (request.source(), request.projection()) {
            (
                SourceKey::ClaudeCode,
                crate::application::collection::CollectionProjection::Daily,
            ) => {
                let report = decode_daily(&output.stdout)?;
                let candidates = map_daily(report, context).map_err(mapping_failure)?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary(&output),
                )
                .map_err(|_| failure(CollectorFailureCode::Internal))
            }
            (
                SourceKey::ClaudeCode,
                crate::application::collection::CollectionProjection::Session,
            ) => {
                let report = decode_session(&output.stdout)?;
                let candidates = map_session(report, context).map_err(mapping_failure)?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary(&output),
                )
                .map_err(|_| failure(CollectorFailureCode::Internal))
            }
            (SourceKey::Codex, crate::application::collection::CollectionProjection::Daily) => {
                let report = decode_codex_daily(&output.stdout)?;
                let candidates = map_codex_daily(report, context).map_err(mapping_failure)?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary(&output),
                )
                .map_err(|_| failure(CollectorFailureCode::Internal))
            }
            (SourceKey::Codex, crate::application::collection::CollectionProjection::Session) => {
                let report = decode_codex_session(&output.stdout)?;
                let candidates = map_codex_session(report, context).map_err(mapping_failure)?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary(&output),
                )
                .map_err(|_| failure(CollectorFailureCode::Internal))
            }
            (
                SourceKey::OpenCode
                | SourceKey::Cline
                | SourceKey::ZCode
                | SourceKey::Antigravity
                | SourceKey::GrokBuild
                | SourceKey::CommandCode
                | SourceKey::Zed,
                _,
            ) => Err(failure(CollectorFailureCode::UnsupportedSource)),
            (SourceKey::Pi, crate::application::collection::CollectionProjection::Daily) => {
                // ccusage can emit `totals: null` for unused Pi sources.
                let report = decode_pi_daily(&output.stdout)?;
                let candidates = map_pi_daily(report, context).map_err(mapping_failure)?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary(&output),
                )
                .map_err(|_| failure(CollectorFailureCode::Internal))
            }
            (SourceKey::Pi, crate::application::collection::CollectionProjection::Session) => {
                let report = decode_pi_session(&output.stdout)?;
                let candidates = map_pi_session(report, context).map_err(mapping_failure)?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary(&output),
                )
                .map_err(|_| failure(CollectorFailureCode::Internal))
            }
            #[cfg(test)]
            _ => Err(failure(CollectorFailureCode::UnsupportedSource)),
        }
    }
}

fn metadata(
    request: &CollectionRequest,
    descriptor: &CollectorDescriptor,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> Result<CollectionMetadata, CollectorFailure> {
    CollectionMetadata::new(
        request.collection_id().clone(),
        descriptor.collector.clone(),
        descriptor.runtime_version.clone(),
        request.source(),
        request.scope().clone(),
        profile_version(descriptor, request.source())?,
        CollectionPeriod {
            started_at,
            finished_at,
        },
    )
    .map_err(|_| failure(CollectorFailureCode::Internal))
}

fn profile_version(
    descriptor: &CollectorDescriptor,
    source: SourceKey,
) -> Result<u16, CollectorFailure> {
    descriptor
        .profiles
        .iter()
        .find(|profile| profile.source == source)
        .map(|profile| profile.profile_version)
        .ok_or_else(|| failure(CollectorFailureCode::UnsupportedSource))
}

fn process_summary(output: &ProcessOutput) -> ProcessSummary {
    ProcessSummary {
        runtime_ms: output.context.runtime_ms.unwrap_or_default(),
        stdout_bytes: output.context.stdout_bytes.unwrap_or_default(),
        stderr_bytes: output.context.stderr_bytes.unwrap_or_default(),
        exit_code: output.context.exit_code,
    }
}

fn current_target() -> Result<BinaryTarget, CollectorFailure> {
    BinaryTarget::current().ok_or_else(|| failure(CollectorFailureCode::BinaryMissing))
}

fn mapping_failure(error: MappingError) -> CollectorFailure {
    match error {
        MappingError::EmptyAggregationTimezone => {
            failure(CollectorFailureCode::ScopeNotRepresentable)
        }
        _ => failure(CollectorFailureCode::AllRecordsRejected),
    }
}

fn failure(code: CollectorFailureCode) -> CollectorFailure {
    CollectorFailure::new(code, None, None)
}

struct ActiveCancellation;

impl CancellationSignal for ActiveCancellation {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        io::Write,
        ops::Deref,
        os::unix::fs::symlink,
        path::{Path, PathBuf},
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    use chrono::TimeZone;
    use sha2::{Digest, Sha256};

    use crate::application::{
        collection::{
            CollectionId, CollectionOutcome, CollectionProjection, CollectionScope, DetectionReason,
        },
        ports::collector::Collector,
    };

    use super::*;

    struct FakeCollector {
        _directory: tempfile::TempDir,
        collector: CcusageCollector,
    }

    impl Deref for FakeCollector {
        type Target = CcusageCollector;

        fn deref(&self) -> &Self::Target {
            &self.collector
        }
    }

    struct TestCancellation(AtomicBool);

    impl TestCancellation {
        fn active() -> Self {
            Self(AtomicBool::new(false))
        }

        fn cancelled() -> Self {
            Self(AtomicBool::new(true))
        }
    }

    impl CancellationSignal for TestCancellation {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::Acquire)
        }
    }

    #[test]
    fn collects_valid_fake_output_into_canonical_candidates() {
        let collector = fake_collector("valid");

        let result = collector
            .collect(
                daily_request(SourceKey::ClaudeCode),
                &TestCancellation::active(),
            )
            .expect("collection result");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 2);
        assert_eq!(
            result.daily_candidates()[0].source_key,
            "claude-code:daily:v1:UTC:2026-06-13"
        );
        assert_eq!(result.daily_candidates()[0].tokens.total_tokens(), 1_650);
        assert_eq!(result.daily_candidates()[0].model_breakdowns.len(), 2);
        assert!(result.process_summary().stdout_bytes > 0);
    }

    #[test]
    fn collects_empty_fake_output_as_successful_empty_result() {
        let result = fake_collector("empty")
            .collect(
                daily_request(SourceKey::ClaudeCode),
                &TestCancellation::active(),
            )
            .expect("empty collection result");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert!(result.daily_candidates().is_empty());
    }

    #[test]
    fn describes_and_detects_supported_fake_sidecar() {
        let collector = fake_collector("valid");

        let descriptor = collector.describe().expect("descriptor");
        assert_eq!(descriptor.collector.as_str(), "ccusage");
        assert_eq!(descriptor.runtime_version, "20.0.19");

        for source in [SourceKey::ClaudeCode, SourceKey::Codex, SourceKey::Pi] {
            collector
                .collect(
                    daily_request_with_timezone(source, "Asia/Jakarta"),
                    &TestCancellation::active(),
                )
                .unwrap_or_else(|error| panic!("{source:?} daily failed: {error:?}"));
            collector
                .collect(session_request(source), &TestCancellation::active())
                .unwrap_or_else(|error| panic!("{source:?} session failed: {error:?}"));
        }

        let detection = collector
            .detect(
                DetectionRequest {
                    source: SourceKey::ClaudeCode,
                    reason: DetectionReason::UserRequested,
                    requested_at: timestamp(),
                },
                &TestCancellation::active(),
            )
            .expect("detection");
        assert_eq!(detection.state, DetectionState::Available);
        assert_eq!(
            detection.supported_projections,
            [CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn rejects_unsupported_requests_before_sidecar_execution() {
        let missing =
            CcusageCollector::development(PathBuf::from("/missing/ccusage")).expect("collector");

        assert_code(
            missing.collect(
                daily_request(SourceKey::OpenCode),
                &TestCancellation::active(),
            ),
            CollectorFailureCode::UnsupportedSource,
        );
        assert_code(
            missing.collect(
                daily_request(SourceKey::TestUnsupported),
                &TestCancellation::active(),
            ),
            CollectorFailureCode::UnsupportedSource,
        );
        // Session projection is now supported, so it won't fail here. We removed that check.
    }

    #[test]
    fn packaged_collector_requires_a_release_manifest() {
        let directory = tempfile::tempdir().expect("packaged resource directory");
        let error = CcusageCollector::packaged(directory.path()).expect_err("missing manifest");

        assert_eq!(error.code, CollectorFailureCode::BinaryMissing);
    }

    #[test]
    fn packaged_payload_executable_lives_through_collection() {
        let collector = packaged_payload_collector();

        let result = collector
            .collect(
                daily_request(SourceKey::ClaudeCode),
                &TestCancellation::active(),
            )
            .expect("payload-backed collection result");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 2);
    }

    #[test]
    fn keeps_binary_process_and_output_failures_distinguishable() {
        assert_code(
            CcusageCollector::development(PathBuf::from("/missing/ccusage"))
                .expect("collector")
                .collect(
                    daily_request(SourceKey::ClaudeCode),
                    &TestCancellation::active(),
                ),
            CollectorFailureCode::BinaryMissing,
        );
        assert_code(
            CcusageCollector::development(fixture_path("fake-collector-old.sh"))
                .expect("collector")
                .collect(
                    daily_request(SourceKey::ClaudeCode),
                    &TestCancellation::active(),
                ),
            CollectorFailureCode::VersionMismatch,
        );

        for (name, expected) in [
            ("nonzero", CollectorFailureCode::NonzeroExit),
            ("non-utf8", CollectorFailureCode::NonUtf8Output),
            ("invalid-json", CollectorFailureCode::InvalidJson),
            ("incompatible", CollectorFailureCode::IncompatibleEnvelope),
            ("stdout-limit", CollectorFailureCode::StdoutLimitExceeded),
            ("stderr-limit", CollectorFailureCode::StderrLimitExceeded),
        ] {
            assert_code(
                fake_collector(name).collect(
                    daily_request(SourceKey::ClaudeCode),
                    &TestCancellation::active(),
                ),
                expected,
            );
        }
    }

    #[test]
    fn enforces_timeout_and_cancellation_through_the_adapter() {
        let timeout_collector = fake_collector_with_limits(
            "timeout",
            ProcessLimits::test(Duration::from_millis(30), 1024 * 1024, 1024),
        );
        assert_code(
            timeout_collector.collect(
                daily_request(SourceKey::ClaudeCode),
                &TestCancellation::active(),
            ),
            CollectorFailureCode::TimedOut,
        );

        assert_code(
            fake_collector("valid").collect(
                daily_request(SourceKey::ClaudeCode),
                &TestCancellation::cancelled(),
            ),
            CollectorFailureCode::Cancelled,
        );
    }

    #[test]
    #[ignore = "set BURNLY_CCUSAGE_DEV_BINARY to run the pinned local sidecar smoke test"]
    fn smoke_tests_opt_in_real_sidecar_shape() {
        let binary = std::env::var_os("BURNLY_CCUSAGE_DEV_BINARY")
            .expect("BURNLY_CCUSAGE_DEV_BINARY must point to a ccusage 20.0.19 binary");
        let collector = CcusageCollector::development(PathBuf::from(binary)).expect("collector");
        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.collector.as_str(), "ccusage");
        assert_eq!(descriptor.runtime_version, "20.0.19");
    }

    fn fake_collector(name: &str) -> FakeCollector {
        fake_collector_with_limits(name, ProcessLimits::collection())
    }

    fn fake_collector_with_limits(name: &str, limits: ProcessLimits) -> FakeCollector {
        let directory = tempfile::tempdir().expect("collector fixture directory");
        let process_directory = directory.path().join("process");
        fs::create_dir(&process_directory).expect("collector process fixture directory");
        symlink(fixture_data_path(), directory.path().join("claude-daily"))
            .expect("collector data fixture symlink");
        let executable = process_directory.join(format!("ccusage-{name}"));
        symlink(fixture_path("fake-collector.sh"), &executable).expect("sidecar symlink");
        let collector = CcusageCollector::development(executable)
            .expect("collector")
            .with_limits(limits);
        FakeCollector {
            _directory: directory,
            collector,
        }
    }

    fn packaged_payload_collector() -> FakeCollector {
        let directory = tempfile::tempdir().expect("packaged collector directory");
        let target = BinaryTarget::current().expect("supported test target");
        let sidecar_directory = directory.path().join("sidecars").join("ccusage");
        fs::create_dir_all(&sidecar_directory).expect("sidecar directory");

        let executable_name = target.executable_name();
        let script = packaged_payload_script();
        let checksum = sha256(&script);

        let executable = sidecar_directory.join(executable_name);
        fs::write(&executable, b"mutated by package tooling").expect("mutated executable");
        let payload = sidecar_directory.join(format!("{executable_name}.payload"));
        let mut payload_file = fs::File::create(payload).expect("payload file");
        payload_file
            .write_all(b"BURNLY-CCUSAGE-PAYLOAD-V1\n")
            .expect("payload header");
        payload_file.write_all(&script).expect("payload bytes");

        fs::write(
            sidecar_directory.join("manifest.json"),
            format!(
                r#"{{
                    "collectorKey":"ccusage",
                    "displayName":"ccusage",
                    "expectedVersion":"20.0.19",
                    "sourceRevision":"caf89e8c0291a2acec09e01ff609e6253f6dd81b",
                    "adapterVersion":1,
                    "entries":[{{
                        "target":"{}",
                        "rustTargetTriple":"{}",
                        "packageName":"{}",
                        "executableName":"{}",
                        "integrity":{{"kind":"release_sha256","sha256":"{}"}}
                    }}]
                }}"#,
                target.as_str(),
                target.rust_target_triple(),
                target.package_name(),
                executable_name,
                checksum
            ),
        )
        .expect("manifest");

        let collector = CcusageCollector::packaged(directory.path())
            .expect("packaged collector")
            .with_limits(ProcessLimits::collection());
        FakeCollector {
            _directory: directory,
            collector,
        }
    }

    fn packaged_payload_script() -> Vec<u8> {
        let valid_daily = fixture_data_path().join("valid.json");
        format!(
            r#"#!/bin/sh
case "$1" in
  --version)
    printf 'ccusage 20.0.19\n'
    exit 0
    ;;
  claude)
    if [ "$2" = "daily" ]; then
      cat '{}'
      exit 0
    fi
    ;;
esac
exit 7
"#,
            valid_daily.display()
        )
        .into_bytes()
    }

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn daily_request(source: SourceKey) -> CollectionRequest {
        daily_request_with_timezone(source, "UTC")
    }

    fn daily_request_with_timezone(source: SourceKey, timezone: &str) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new("collection-1").expect("collection id"),
            source,
            CollectionScope::Full,
            timezone,
            timestamp(),
        )
        .expect("daily request")
    }

    fn session_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new("collection-2").expect("collection id"),
            source,
            CollectionScope::Full,
            timestamp(),
        )
    }

    fn timestamp() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 14, 12, 0, 0)
            .single()
            .expect("timestamp")
    }

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("tests/fixtures/collectors/ccusage/process")
            .join(name)
    }

    fn fixture_data_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("tests/fixtures/collectors/ccusage/claude-daily")
    }

    fn assert_code(
        result: Result<CollectionResult, CollectorFailure>,
        expected: CollectorFailureCode,
    ) {
        assert_eq!(result.expect_err("collector failure").code, expected);
    }
}
