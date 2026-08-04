//! Command Code data root resolution.

use std::path::{Path, PathBuf};

pub(crate) fn resolve_commandcode_home(override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("COMMANDCODE_HOME").map(PathBuf::from))
        .or_else(default_home_commandcode_dir)
        .unwrap_or_else(|| PathBuf::from(".commandcode"))
}

#[allow(dead_code, reason = "used by a later adapter chunk")]
pub(crate) fn default_commandcode_home() -> PathBuf {
    resolve_commandcode_home(None)
}

pub(crate) fn projects_root(commandcode_home: &Path) -> PathBuf {
    commandcode_home.join("projects")
}

fn default_home_commandcode_dir() -> Option<PathBuf> {
    home_directory().map(|directory| directory.join(".commandcode"))
}

fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_explicit_override_before_environment() {
        let resolved = resolve_commandcode_home(Some(Path::new("/override/commandcode")));

        assert_eq!(resolved, PathBuf::from("/override/commandcode"));
    }

    #[test]
    fn resolves_projects_root_under_commandcode_home() {
        let commandcode_home = PathBuf::from("/tmp/commandcode");

        assert_eq!(
            projects_root(&commandcode_home),
            PathBuf::from("/tmp/commandcode/projects")
        );
    }
}
