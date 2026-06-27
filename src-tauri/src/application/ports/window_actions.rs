use thiserror::Error;

pub(crate) trait WindowActions: Send + Sync {
    fn hide_tray_panel(&self) -> Result<(), WindowActionError>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum WindowActionError {
    #[error("failed to hide the tray panel")]
    HideTrayPanel,
}
