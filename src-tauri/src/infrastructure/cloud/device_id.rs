//! Stable install device identity for burnly-api device binding.
//!
//! Survives logout. Reinstall may create a new id.

use std::fs;
use std::path::PathBuf;

use uuid::Uuid;

const DEVICE_ID_FILE: &str = "cloud_device_id";

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum DeviceIdError {
    #[error("device id storage failed")]
    Storage,
}

#[derive(Debug, Clone)]
pub(crate) struct DeviceIdentity {
    path: PathBuf,
    device_name: String,
}

impl DeviceIdentity {
    pub(crate) fn new(data_directory: impl Into<PathBuf>, device_name: impl Into<String>) -> Self {
        let data_directory = data_directory.into();
        Self {
            path: data_directory.join(DEVICE_ID_FILE),
            device_name: device_name.into(),
        }
    }

    pub(crate) fn device_name(&self) -> &str {
        &self.device_name
    }

    pub(crate) fn get_or_create_device_id(&self) -> Result<String, DeviceIdError> {
        if let Ok(existing) = fs::read_to_string(&self.path) {
            let trimmed = existing.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_owned());
            }
        }

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| DeviceIdError::Storage)?;
        }
        let id = format!("dev_{}", Uuid::new_v4());
        fs::write(&self.path, &id).map_err(|_| DeviceIdError::Storage)?;
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reuses_persisted_device_id() {
        let dir = tempdir().expect("tempdir");
        let identity = DeviceIdentity::new(dir.path(), "test-host");
        let first = identity.get_or_create_device_id().expect("create");
        let second = identity.get_or_create_device_id().expect("reuse");
        assert_eq!(first, second);
        assert!(first.starts_with("dev_"));
        assert_eq!(identity.device_name(), "test-host");
    }
}
