//! In-memory cloud token store for tests and non-persistent runtimes.

use std::sync::Mutex;

use crate::application::ports::cloud_token_store::{
    CloudTokenStore, CloudTokenStoreError, StoredCloudSession,
};

#[derive(Default)]
pub(crate) struct MemoryCloudTokenStore {
    inner: Mutex<Option<StoredCloudSession>>,
}

impl MemoryCloudTokenStore {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

impl CloudTokenStore for MemoryCloudTokenStore {
    fn load(&self) -> Result<Option<StoredCloudSession>, CloudTokenStoreError> {
        self.inner
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| CloudTokenStoreError::Backend)
    }

    fn save(&self, session: &StoredCloudSession) -> Result<(), CloudTokenStoreError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| CloudTokenStoreError::Backend)?;
        *guard = Some(session.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), CloudTokenStoreError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| CloudTokenStoreError::Backend)?;
        *guard = None;
        Ok(())
    }
}
