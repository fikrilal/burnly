//! DeepSeek Harness data-root resolution.
//!
//! The harness keeps all user data under one root. Resolution precedence is
//! an explicit override, a non-empty `DSH_HOME`, the current user's `~/.dsh`
//! directory, and finally a relative `.dsh` fallback for unusual environments.

use std::path::{Path, PathBuf};

pub(crate) fn resolve_deepseek_harness_home(override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .or_else(|| {
            std::env::var_os("DSH_HOME")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().to_string_lossy().trim().is_empty())
        })
        .or_else(default_home_deepseek_harness_dir)
        .unwrap_or_else(|| PathBuf::from(".dsh"))
}

#[allow(
    dead_code,
    reason = "default home is consumed once the collector is wired in a later chunk"
)]
pub(crate) fn default_deepseek_harness_home() -> PathBuf {
    resolve_deepseek_harness_home(None)
}

pub(crate) fn sessions_root(deepseek_harness_home: &Path) -> PathBuf {
    deepseek_harness_home.join("sessions")
}

fn default_home_deepseek_harness_dir() -> Option<PathBuf> {
    home_directory().map(|directory| directory.join(".dsh"))
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
        let resolved = resolve_deepseek_harness_home(Some(Path::new("/override/deepseek-harness")));

        assert_eq!(resolved, PathBuf::from("/override/deepseek-harness"));
    }

    #[test]
    fn resolves_sessions_root_under_home() {
        let home = PathBuf::from("/tmp/deepseek-harness");

        assert_eq!(
            sessions_root(&home),
            PathBuf::from("/tmp/deepseek-harness/sessions")
        );
    }

    #[test]
    fn honors_non_empty_dsh_home_and_ignores_blank_override() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let configured_home = temp.path().join("configured-dsh-home");
        let previous = std::env::var_os("DSH_HOME");

        std::env::set_var("DSH_HOME", &configured_home);
        let resolved = resolve_deepseek_harness_home(None);
        assert_eq!(resolved, configured_home);

        std::env::set_var("DSH_HOME", "   ");
        let blank = resolve_deepseek_harness_home(None);
        match previous {
            Some(value) => std::env::set_var("DSH_HOME", value),
            None => std::env::remove_var("DSH_HOME"),
        }

        assert_ne!(blank, PathBuf::from("   "));
        assert!(!blank.as_os_str().is_empty());
    }
}
