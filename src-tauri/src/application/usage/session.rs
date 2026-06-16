use crate::application::ports::session_store::{
    SessionPagination, SessionStore, SessionStoreError,
};
use crate::domain::usage::{SessionDetail, UsageSession};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct SessionQuery {
    store: Arc<dyn SessionStore>,
}

impl SessionQuery {
    pub(crate) fn new(store: Arc<dyn SessionStore>) -> Self {
        Self { store }
    }

    pub(crate) fn get_sessions(
        &self,
        source_id: Option<i64>,
        pagination: SessionPagination,
    ) -> Result<Vec<UsageSession>, SessionStoreError> {
        self.store.get_sessions(source_id, pagination)
    }

    pub(crate) fn get_session_detail(
        &self,
        session_id: i64,
    ) -> Result<SessionDetail, SessionStoreError> {
        self.store.get_session_detail(session_id)
    }
}
