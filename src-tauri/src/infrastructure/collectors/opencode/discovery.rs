//! OpenCode data-root and database-path resolution.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const DATABASE_NAME: &str = "opencode.db";

pub(crate) fn resolve_opencode_database(override_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = override_path {
        return Some(path.to_path_buf());
    }

    resolve_from_environment(&EnvironmentPaths {
        opencode_db: std::env::var_os("OPENCODE_DB"),
        xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
        home: std::env::var_os("HOME"),
        user_profile: std::env::var_os("USERPROFILE"),
    })
}

pub(crate) fn default_opencode_database() -> Option<PathBuf> {
    resolve_opencode_database(None)
}

struct EnvironmentPaths {
    opencode_db: Option<OsString>,
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
    user_profile: Option<OsString>,
}

fn resolve_from_environment(environment: &EnvironmentPaths) -> Option<PathBuf> {
    non_empty_path(environment.opencode_db.as_deref())
        .map(Path::to_path_buf)
        .or_else(|| {
            non_empty_path(environment.xdg_data_home.as_deref())
                .filter(|path| path.is_absolute())
                .map(opencode_database_under)
        })
        .or_else(|| {
            non_empty_path(environment.home.as_deref())
                .or_else(|| non_empty_path(environment.user_profile.as_deref()))
                .map(|profile| {
                    profile
                        .join(".local")
                        .join("share")
                        .join("opencode")
                        .join(DATABASE_NAME)
                })
        })
}

fn opencode_database_under(data_home: &Path) -> PathBuf {
    data_home.join("opencode").join(DATABASE_NAME)
}

fn non_empty_path(value: Option<&OsStr>) -> Option<&Path> {
    value.filter(|value| !value.is_empty()).map(Path::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_override_has_highest_precedence() {
        let path = resolve_opencode_database(Some(Path::new("/fixture/custom.db")));

        assert_eq!(path, Some(PathBuf::from("/fixture/custom.db")));
    }

    #[test]
    fn environment_database_override_wins_over_data_roots() {
        let path = resolve_from_environment(&EnvironmentPaths {
            opencode_db: Some(OsString::from("/fixture/explicit.db")),
            xdg_data_home: Some(OsString::from("/fixture/xdg")),
            home: Some(OsString::from("/fixture/home")),
            user_profile: None,
        });

        assert_eq!(path, Some(PathBuf::from("/fixture/explicit.db")));
    }

    #[test]
    fn absolute_xdg_data_home_wins_over_profile_fallback() {
        let path = resolve_from_environment(&EnvironmentPaths {
            opencode_db: None,
            xdg_data_home: Some(OsString::from("/fixture/xdg")),
            home: Some(OsString::from("/fixture/home")),
            user_profile: None,
        });

        assert_eq!(
            path,
            Some(PathBuf::from("/fixture/xdg/opencode/opencode.db"))
        );
    }

    #[test]
    fn relative_xdg_data_home_is_ignored() {
        let path = resolve_from_environment(&EnvironmentPaths {
            opencode_db: None,
            xdg_data_home: Some(OsString::from("relative/data")),
            home: Some(OsString::from("/fixture/home")),
            user_profile: None,
        });

        assert_eq!(
            path,
            Some(PathBuf::from(
                "/fixture/home/.local/share/opencode/opencode.db"
            ))
        );
    }

    #[test]
    fn user_profile_is_used_when_home_is_unavailable() {
        let path = resolve_from_environment(&EnvironmentPaths {
            opencode_db: None,
            xdg_data_home: None,
            home: None,
            user_profile: Some(OsString::from("C:/Users/fixture")),
        });

        assert_eq!(
            path,
            Some(PathBuf::from(
                "C:/Users/fixture/.local/share/opencode/opencode.db"
            ))
        );
    }

    #[test]
    fn missing_environment_has_no_implicit_working_directory_fallback() {
        let path = resolve_from_environment(&EnvironmentPaths {
            opencode_db: None,
            xdg_data_home: None,
            home: None,
            user_profile: None,
        });

        assert_eq!(path, None);
    }
}
