use std::path::{Path, PathBuf};

pub(crate) fn resolve_grok_home(override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("GROK_HOME").map(PathBuf::from))
        .or_else(default_home_grok_dir)
        .unwrap_or_else(|| PathBuf::from(".grok"))
}

#[allow(dead_code, reason = "default grok home is used by the adapter chunk")]
pub(crate) fn default_grok_home() -> PathBuf {
    resolve_grok_home(None)
}

pub(crate) fn unified_log_path(grok_home: &Path) -> PathBuf {
    grok_home.join("logs").join("unified.jsonl")
}

pub(crate) fn sessions_root(grok_home: &Path) -> PathBuf {
    grok_home.join("sessions")
}

fn default_home_grok_dir() -> Option<PathBuf> {
    home_directory().map(|directory| directory.join(".grok"))
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
        let resolved = resolve_grok_home(Some(Path::new("/override/grok")));

        assert_eq!(resolved, PathBuf::from("/override/grok"));
    }

    #[test]
    fn resolves_unified_log_under_grok_home() {
        let grok_home = PathBuf::from("/tmp/grok");

        assert_eq!(
            unified_log_path(&grok_home),
            PathBuf::from("/tmp/grok/logs/unified.jsonl")
        );
    }
}
