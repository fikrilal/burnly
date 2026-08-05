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
use crate::infrastructure::collectors::routed::RoutedCollector;
use crate::infrastructure::collectors::zcode::ZCodeCollector;
use crate::infrastructure::database::{
    Database, SqliteAntigravityUsageCacheStore, SqliteDiagnosticStore, SqliteGrokUsageCacheStore,
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
    let usage_cache = Arc::new(SqliteAntigravityUsageCacheStore::new(usage_cache_database));
    let antigravity_collector = Arc::new(AntigravityCollector::with_diagnostic_recorder(
        diagnostic_recorder,
        usage_cache,
    ));
    let commandcode_collector = Arc::new(
        CommandCodeCollector::from_data_dir(default_commandcode_home()).with_diagnostic_recorder(
            Arc::new(SqliteDiagnosticStore::new(
                Database::open(database_path).map_err(StartupError::Persistence)?,
            )),
        ),
    );

    Ok(Arc::new(RoutedCollector::new(
        ccusage_collector,
        cline_collector,
        zcode_collector,
        antigravity_collector,
        grok_collector,
        commandcode_collector,
    )))
}
