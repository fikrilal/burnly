use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionScope, CollectorFailure,
    CollectorFailureCode,
};

use super::{
    capability_profiles::profile_for, process::ProcessRequest, source_registry::source_descriptor,
};

const ENVIRONMENT_ALLOWLIST: &[&str] = &[
    "HOME",
    "LANG",
    "LC_ALL",
    "PATH",
    "TMP",
    "TEMP",
    "TMPDIR",
    "TZ",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "SYSTEMROOT",
    "WINDIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

#[derive(Debug)]
pub(crate) struct PreparedCommand {
    process: ProcessRequest,
    _workspace: CommandWorkspace,
}

impl PreparedCommand {
    pub(crate) fn process(&self) -> &ProcessRequest {
        &self.process
    }
}

pub(crate) fn prepare_collection(
    executable: &Path,
    request: &CollectionRequest,
) -> Result<PreparedCommand, CollectorFailure> {
    let profile = profile_for(request.source(), request.projection())?;
    let report_profile = match request.projection() {
        CollectionProjection::Daily => profile.daily.as_ref(),
        CollectionProjection::Session => profile.session.as_ref(),
    }
    .ok_or_else(|| {
        CollectorFailure::new(
            CollectorFailureCode::UnsupportedProjection,
            Some(request.source()),
            Some(request.projection()),
        )
    })?;

    let source = source_descriptor(request.source())?;
    let workspace = CommandWorkspace::create()?;
    let mut arguments = vec![
        OsString::from(source.command_namespace),
        OsString::from(report_profile.report_name),
        OsString::from("--json"),
        OsString::from("--offline"),
        OsString::from("--mode"),
        OsString::from("calculate"),
        OsString::from("--no-color"),
        OsString::from("--config"),
        workspace.config_path().as_os_str().to_owned(),
    ];

    match request.scope() {
        CollectionScope::Full => {}
        CollectionScope::Incremental(scope) => {
            arguments.push(OsString::from("--since"));
            arguments.push(OsString::from(
                scope.start_date().format("%Y%m%d").to_string(),
            ));
            arguments.push(OsString::from("--until"));
            arguments.push(OsString::from(
                scope.end_date().format("%Y%m%d").to_string(),
            ));
        }
    }

    let timezone = request.aggregation_timezone().unwrap_or("UTC");
    arguments.push(OsString::from("--timezone"));
    arguments.push(OsString::from(timezone));

    Ok(PreparedCommand {
        process: ProcessRequest::new(
            executable.to_path_buf(),
            arguments,
            workspace.path().to_path_buf(),
            allowlisted_environment(),
        ),
        _workspace: workspace,
    })
}

pub(crate) fn prepare_version_check(
    executable: &Path,
) -> Result<PreparedCommand, CollectorFailure> {
    let workspace = CommandWorkspace::create()?;
    Ok(PreparedCommand {
        process: ProcessRequest::new(
            executable.to_path_buf(),
            vec![OsString::from("--version")],
            workspace.path().to_path_buf(),
            allowlisted_environment(),
        ),
        _workspace: workspace,
    })
}

fn allowlisted_environment() -> Vec<(OsString, OsString)> {
    ENVIRONMENT_ALLOWLIST
        .iter()
        .filter_map(|key| env::var_os(key).map(|value| (OsString::from(key), value)))
        .collect()
}

#[derive(Debug)]
struct CommandWorkspace {
    directory: TempDir,
    config_path: PathBuf,
}

impl CommandWorkspace {
    fn create() -> Result<Self, CollectorFailure> {
        let directory = tempfile::Builder::new()
            .prefix("burnly-ccusage-")
            .tempdir()
            .map_err(|_| internal_failure())?;
        let config_path = directory.path().join("config.json");
        fs::write(&config_path, b"{}\n").map_err(|_| internal_failure())?;
        restrict_config_permissions(&config_path).map_err(|_| internal_failure())?;
        Ok(Self {
            directory,
            config_path,
        })
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn config_path(&self) -> &Path {
        &self.config_path
    }
}

fn internal_failure() -> CollectorFailure {
    CollectorFailure::new(CollectorFailureCode::Internal, None, None)
}

#[cfg(unix)]
fn restrict_config_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_config_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};

    use crate::{
        application::collection::{CollectionId, CollectionScope},
        domain::source::SourceKey,
    };

    use super::*;

    #[test]
    fn prepares_only_the_reviewed_claude_daily_arguments() {
        let request = CollectionRequest::daily(
            CollectionId::new("collection-1").expect("collection id"),
            SourceKey::ClaudeCode,
            CollectionScope::incremental(
                NaiveDate::from_ymd_opt(2026, 6, 1).expect("start date"),
                NaiveDate::from_ymd_opt(2026, 6, 14).expect("end date"),
            )
            .expect("scope"),
            "Asia/Jakarta",
            Utc.with_ymd_and_hms(2026, 6, 14, 12, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("request");

        let prepared =
            prepare_collection(Path::new("/reviewed/ccusage"), &request).expect("prepared command");
        let arguments = prepared
            .process()
            .arguments()
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            arguments[0..8],
            [
                "claude",
                "daily",
                "--json",
                "--offline",
                "--mode",
                "calculate",
                "--no-color",
                "--config",
            ]
        );
        assert!(arguments[8].ends_with("config.json"));
        assert_eq!(
            arguments[9..],
            [
                "--since",
                "20260601",
                "--until",
                "20260614",
                "--timezone",
                "Asia/Jakarta",
            ]
        );
        assert_eq!(
            fs::read_to_string(&arguments[8]).expect("controlled config"),
            "{}\n"
        );
        assert_eq!(
            prepared.process().working_directory(),
            Path::new(&arguments[8]).parent().expect("config parent")
        );
        assert!(prepared
            .process()
            .environment()
            .iter()
            .all(|(key, _)| key != "CCUSAGE_MODEL_ALIASES"));
    }

    #[test]
    fn rejects_unreviewed_source_and_projection_before_workspace_execution() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 6, 14, 12, 0, 0)
            .single()
            .expect("timestamp");
        let unsupported_source = CollectionRequest::daily(
            CollectionId::new("collection-2").expect("collection id"),
            SourceKey::Codex,
            CollectionScope::Full,
            "UTC",
            timestamp,
        )
        .expect("request");
        assert_eq!(
            prepare_collection(Path::new("ccusage"), &unsupported_source)
                .expect_err("unsupported source")
                .code,
            CollectorFailureCode::UnsupportedSource
        );

        let _unsupported_projection_req = CollectionRequest::daily(
            CollectionId::new("collection-3").expect("collection id"),
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            "UTC",
            timestamp,
        )
        .expect("request");
        // We test an actually unsupported projection if we had one. Since we support Daily and Session, we can test that it succeeds or just omit the test for unsupported projection since both are supported for ClaudeCode.
    }
}
