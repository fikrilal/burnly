use thiserror::Error;

pub(crate) trait WindowActions: Send + Sync {
    fn open_details(&self) -> Result<(), WindowActionError>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum WindowActionError {
    #[error("failed to open the details window")]
    OpenDetails,
}
