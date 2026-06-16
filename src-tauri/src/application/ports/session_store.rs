use crate::domain::usage::{SessionDetail, UsageSession};

#[derive(Debug, Clone)]
pub(crate) struct SessionPagination {
    pub limit: u32,
    pub after_activity_ms: Option<i64>,
}

pub(crate) enum SessionStoreError {
    Backend,
    NotFound,
}

pub(crate) trait SessionStore: Send + Sync {
    fn get_sessions(
        &self,
        source_id: Option<i64>,
        pagination: SessionPagination,
    ) -> Result<Vec<UsageSession>, SessionStoreError>;

    fn get_session_detail(&self, session_id: i64) -> Result<SessionDetail, SessionStoreError>;
}
