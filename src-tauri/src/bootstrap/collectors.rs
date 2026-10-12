use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::collection::{
    CollectionRequest, CollectionResult, CollectorDescriptor, CollectorFailure,
    CollectorFailureCode, DetectionRequest, DetectionResult,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::infrastructure::collectors::antigravity::AntigravityCollector;
use crate::infrastructure::collectors::ccusage::CcusageCollector;
use crate::infrastructure::collectors::cline::ClineCollector;
use crate::infrastructure::collectors::commandcode::{
    default_commandcode_home, CommandCodeCollector,
};
use crate::infrastructure::collectors::deepseek_harness::{
    default_deepseek_harness_home, DeepSeekHarnessCollector,
};
use crate::infrastructure::collectors::grok::{
    default_grok_home, GrokCollector, GrokUsageCacheClient,
};
use crate::infrastructure::collectors::opencode::OpenCodeCollector;
use crate::infrastructure::collectors::routed::{CollectorRoutes, RoutedCollector};
use crate::infrastructure::collectors::zcode::ZCodeCollector;
use crate::infrastructure::collectors::zed::{default_zed_data_dir, ZedCollector};
use crate::infrastructure::database::{
    AntigravityBaselineRepairService, Database, SqliteAntigravityBaselineStore,
    SqliteAntigravityUsageCacheStore, SqliteDiagnosticStore, SqliteGrokUsageCacheStore,
    SqliteOpenCodeUsageLedgerStore,
};

use super::{resources, StartupError};

pub(super) fn build_collector_graph(
    resource_directory: PathBuf,
    database_path: &Path,
) -> Result<Arc<dyn Collector>, StartupError> {
    Ok(Arc::new(JobScopedCollectorGraph {
        resource_directory,
        database_path: database_path.to_path_buf(),
    }))
}

struct JobScopedCollectorGraph {
    resource_directory: PathBuf,
    database_path: PathBuf,
}

impl JobScopedCollectorGraph {
    fn open(&self) -> Result<Arc<dyn Collector>, StartupError> {
        open_collector_graph(&self.resource_directory, &self.database_path)
    }
}

impl Collector for JobScopedCollectorGraph {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        self.open()
            .map_err(|_| CollectorFailure::new(CollectorFailureCode::Internal, None, None))?
            .describe()
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        self.open()
            .map_err(|_| {
                CollectorFailure::new(CollectorFailureCode::Internal, Some(request.source), None)
            })?
            .detect(request, cancellation)
    }

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        self.open()
            .map_err(|_| {
                CollectorFailure::new(
                    CollectorFailureCode::Internal,
                    Some(request.source()),
                    Some(request.projection()),
                )
            })?
            .collect(request, cancellation)
    }
}

fn open_collector_graph(
    resource_directory: &Path,
    database_path: &Path,
) -> Result<Arc<dyn Collector>, StartupError> {
    let packaged_resource_directory =
        resources::resolve_packaged_resource_directory(resource_directory.to_path_buf());
    let ccusage_collector = Arc::new(
        match std::env::var_os("BURNLY_CCUSAGE_DEV_BINARY") {
            Some(binary) => CcusageCollector::development(binary),
            None => CcusageCollector::packaged(packaged_resource_directory),
        }
        .map_err(StartupError::Collector)?,
    );
    let diagnostics_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let diagnostic_recorder = Arc::new(SqliteDiagnosticStore::new(diagnostics_database));
    let opencode_ledger_database =
        Database::open(database_path).map_err(StartupError::Persistence)?;
    let opencode_ledger = Arc::new(SqliteOpenCodeUsageLedgerStore::new(
        opencode_ledger_database,
    ));
    let opencode_collector = Arc::new(
        OpenCodeCollector::from_default_location(opencode_ledger)
            .with_diagnostic_recorder(diagnostic_recorder.clone()),
    );
    let cline_collector = Arc::new(
        ClineCollector::from_data_dir(resources::default_cline_data_dir())
            .with_diagnostic_recorder(diagnostic_recorder.clone()),
    );
    let zcode_collector = Arc::new(
        ZCodeCollector::from_data_dir(resources::default_zcode_data_dir())
            .with_diagnostic_recorder(diagnostic_recorder.clone()),
    );
    let grok_usage_cache_database =
        Database::open(database_path).map_err(StartupError::Persistence)?;
    let grok_usage_cache = Arc::new(SqliteGrokUsageCacheStore::new(grok_usage_cache_database));
    let grok_collector = Arc::new(
        GrokCollector::from_grok_home(default_grok_home())
            .with_usage_cache(GrokUsageCacheClient::new(grok_usage_cache))
            .with_diagnostic_recorder(diagnostic_recorder.clone()),
    );
    let usage_cache_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let baseline_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let repair_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let usage_cache = Arc::new(SqliteAntigravityUsageCacheStore::new(usage_cache_database));
    let baseline_store = Arc::new(SqliteAntigravityBaselineStore::new(baseline_database));
    let repair_service = Arc::new(AntigravityBaselineRepairService::with_diagnostics(
        repair_database,
        Some(diagnostic_recorder.clone()),
    ));
    let antigravity_collector = Arc::new(
        AntigravityCollector::with_diagnostic_recorder(diagnostic_recorder.clone(), usage_cache)
            .with_baseline_store(baseline_store)
            .with_repair_service(repair_service),
    );
    let commandcode_collector = Arc::new(
        CommandCodeCollector::from_data_dir(default_commandcode_home()).with_diagnostic_recorder(
            Arc::new(SqliteDiagnosticStore::new(
                Database::open(database_path).map_err(StartupError::Persistence)?,
            )),
        ),
    );
    let zed_collector = Arc::new(
        ZedCollector::from_data_dir(default_zed_data_dir())
            .with_diagnostic_recorder(diagnostic_recorder.clone()),
    );
    let deepseek_harness_collector = Arc::new(DeepSeekHarnessCollector::from_data_dir(
        default_deepseek_harness_home(),
    ));

    Ok(Arc::new(RoutedCollector::new(CollectorRoutes {
        ccusage: ccusage_collector,
        opencode: opencode_collector,
        cline: cline_collector,
        zcode: zcode_collector,
        antigravity: antigravity_collector,
        grok: grok_collector,
        commandcode: commandcode_collector,
        deepseek_harness: deepseek_harness_collector,
        zed: zed_collector,
    })))
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::path::Path;
    use std::thread;

    use chrono::{NaiveDate, Utc};
    use tempfile::TempDir;

    use super::{build_collector_graph, open_collector_graph};
    use crate::application::collection::{CollectionId, CollectionRequest, CollectionScope};
    use crate::application::ports::collector::{CancellationSignal, Collector};
    use crate::application::ports::tray_summary_store::TraySummaryStore;
    use crate::application::usage::TraySummaryScope;
    use crate::domain::source::SourceKey;
    use crate::infrastructure::collectors::opencode::OpenCodeCollector;
    use crate::infrastructure::collectors::{clear_source_open_gate, SourceOpenGate};
    use crate::infrastructure::database::{
        Database, SqliteOpenCodeUsageLedgerStore, SqliteTraySummaryStore,
    };

    fn sqlite_fds(path: &Path) -> usize {
        let Ok(prefix) = path.canonicalize() else {
            return 0;
        };
        let Ok(prefix) = prefix.into_os_string().into_string() else {
            return 0;
        };
        let Ok(entries) = fs::read_dir("/proc/self/fd") else {
            return 0;
        };
        entries
            .filter_map(Result::ok)
            .filter(|entry| {
                let Ok(link) = fs::read_link(entry.path()) else {
                    return false;
                };
                let Ok(link) = link.into_os_string().into_string() else {
                    return false;
                };
                link == prefix || link.starts_with(&format!("{prefix}-"))
            })
            .count()
    }

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    struct CcusageBinaryGuard {
        previous: Option<std::ffi::OsString>,
    }

    impl CcusageBinaryGuard {
        fn missing_binary() -> Self {
            let previous = std::env::var_os("BURNLY_CCUSAGE_DEV_BINARY");
            std::env::set_var("BURNLY_CCUSAGE_DEV_BINARY", "/missing/ccusage");
            Self { previous }
        }
    }

    impl Drop for CcusageBinaryGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("BURNLY_CCUSAGE_DEV_BINARY", value),
                None => std::env::remove_var("BURNLY_CCUSAGE_DEV_BINARY"),
            }
        }
    }

    struct SourceOpenGateGuard(std::sync::Arc<SourceOpenGate>);

    impl Drop for SourceOpenGateGuard {
        fn drop(&mut self) {
            self.0.release();
            clear_source_open_gate();
        }
    }

    #[test]
    fn collector_sqlite_handles_close_when_the_graph_returns() {
        let _binary = CcusageBinaryGuard::missing_binary();
        let directory = TempDir::new().expect("tempdir");
        let database_path = directory.path().join("burnly.sqlite3");
        let before = sqlite_fds(&database_path);

        let graph = open_collector_graph(directory.path(), &database_path).expect("open graph");
        assert!(sqlite_fds(&database_path) > before);
        drop(graph);
        assert_eq!(sqlite_fds(&database_path), before);

        let wrapper =
            build_collector_graph(directory.path().to_path_buf(), &database_path).expect("wrapper");
        let described = wrapper.describe();
        assert_eq!(sqlite_fds(&database_path), before);
        assert!(
            described.is_err(),
            "missing ccusage binary still closes job databases"
        );
    }

    #[test]
    fn tray_summary_reads_while_collector_io_holds_its_own_connection() {
        let directory = TempDir::new().expect("tempdir");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut setup = Database::open(&database_path).expect("setup database");
        setup.migrate_to_latest().expect("migrate");
        drop(setup);

        let source_path = directory.path().join("opencode.db");
        File::create(&source_path).expect("source file");

        let tray = SqliteTraySummaryStore::new(Database::open(&database_path).expect("tray"));
        let collector = OpenCodeCollector::from_database_path(
            source_path,
            std::sync::Arc::new(SqliteOpenCodeUsageLedgerStore::new(
                Database::open(&database_path).expect("ledger"),
            )),
        );
        assert!(sqlite_fds(&database_path) >= 2);

        let gate = SourceOpenGate::install();
        let _gate_guard = SourceOpenGateGuard(gate.clone());
        let handle = thread::spawn(move || {
            let _ = collector.collect(
                CollectionRequest::session(
                    CollectionId::new("tray-during-io").expect("id"),
                    SourceKey::OpenCode,
                    CollectionScope::Full,
                    Utc::now(),
                ),
                &NeverCancelled,
            );
        });
        gate.wait_until_entered();
        let scope =
            TraySummaryScope::new(NaiveDate::from_ymd_opt(2026, 10, 11).expect("today"), "UTC")
                .expect("scope");
        tray.read_tray_summary(&scope)
            .expect("tray summary during collector I/O");
        gate.release();
        handle.join().expect("collector thread");
        drop(tray);
        assert_eq!(sqlite_fds(&database_path), 0);
    }
}
