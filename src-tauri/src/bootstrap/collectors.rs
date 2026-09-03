use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::ports::collector::Collector;
use crate::infrastructure::collectors::antigravity::AntigravityCollector;
use crate::infrastructure::collectors::ccusage::CcusageCollector;
use crate::infrastructure::collectors::cline::ClineCollector;
use crate::infrastructure::collectors::commandcode::{
    default_commandcode_home, CommandCodeCollector,
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
    let packaged_resource_directory =
        resources::resolve_packaged_resource_directory(resource_directory);
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

    Ok(Arc::new(RoutedCollector::new(CollectorRoutes {
        ccusage: ccusage_collector,
        opencode: opencode_collector,
        cline: cline_collector,
        zcode: zcode_collector,
        antigravity: antigravity_collector,
        grok: grok_collector,
        commandcode: commandcode_collector,
        zed: zed_collector,
    })))
}
