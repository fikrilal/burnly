use std::path::{Path, PathBuf};

pub(super) fn default_cline_data_dir() -> PathBuf {
    std::env::var_os("CLINE_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| default_home_data_dir(".cline"))
        .unwrap_or_else(|| PathBuf::from(".cline"))
}

pub(super) fn default_zcode_data_dir() -> PathBuf {
    std::env::var_os("ZCODE_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| default_home_data_dir(".zcode"))
        .unwrap_or_else(|| PathBuf::from(".zcode"))
}

fn default_home_data_dir(name: &str) -> Option<PathBuf> {
    home_data_dir(
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os("USERPROFILE").map(PathBuf::from),
        name,
    )
}

pub(super) fn home_data_dir(
    home: Option<PathBuf>,
    userprofile: Option<PathBuf>,
    name: &str,
) -> Option<PathBuf> {
    home.or(userprofile).map(|directory| directory.join(name))
}

pub(super) fn resolve_packaged_resource_directory(resource_directory: PathBuf) -> PathBuf {
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
    resolve_packaged_resource_directory_for_appdir(resource_directory, appdir.as_deref())
}

pub(super) fn resolve_packaged_resource_directory_for_appdir(
    resource_directory: PathBuf,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] appdir: Option<&Path>,
) -> PathBuf {
    if packaged_sidecar_manifest_exists(&resource_directory) {
        return resource_directory;
    }

    #[cfg(target_os = "linux")]
    if let Some(appdir) = appdir {
        let product_resource_directory = appdir.join("usr").join("lib").join("Burnly");
        if product_resource_directory != resource_directory
            && packaged_sidecar_manifest_exists(&product_resource_directory)
        {
            return product_resource_directory;
        }
    }

    resource_directory
}

fn packaged_sidecar_manifest_exists(resource_directory: &Path) -> bool {
    resource_directory
        .join("sidecars")
        .join("ccusage")
        .join("manifest.json")
        .is_file()
}
