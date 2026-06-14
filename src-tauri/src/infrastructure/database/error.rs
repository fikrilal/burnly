use std::io;
use std::path::PathBuf;

use rusqlite::Error as SqliteError;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceErrorKind {
    InvalidPath,
    CreateDirectory,
    Open,
    Configure,
    PolicyMismatch,
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database path has no parent directory: {path}")]
    InvalidPath { path: PathBuf },

    #[error("failed to create database directory: {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to open database: {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: SqliteError,
    },

    #[error("failed to configure database setting: {setting}")]
    Configure {
        setting: &'static str,
        #[source]
        source: SqliteError,
    },

    #[error("database setting {setting} expected {expected}, found {actual}")]
    PolicyMismatch {
        setting: &'static str,
        expected: String,
        actual: String,
    },
}

impl PersistenceError {
    pub fn kind(&self) -> PersistenceErrorKind {
        match self {
            Self::InvalidPath { .. } => PersistenceErrorKind::InvalidPath,
            Self::CreateDirectory { .. } => PersistenceErrorKind::CreateDirectory,
            Self::Open { .. } => PersistenceErrorKind::Open,
            Self::Configure { .. } => PersistenceErrorKind::Configure,
            Self::PolicyMismatch { .. } => PersistenceErrorKind::PolicyMismatch,
        }
    }

    pub(super) fn invalid_path(path: PathBuf) -> Self {
        Self::InvalidPath { path }
    }

    pub(super) fn create_directory(path: PathBuf, source: io::Error) -> Self {
        Self::CreateDirectory { path, source }
    }

    pub(super) fn open(path: PathBuf, source: SqliteError) -> Self {
        Self::Open { path, source }
    }

    pub(super) fn configure(setting: &'static str, source: SqliteError) -> Self {
        Self::Configure { setting, source }
    }

    pub(super) fn policy_mismatch(setting: &'static str, expected: String, actual: String) -> Self {
        Self::PolicyMismatch {
            setting,
            expected,
            actual,
        }
    }
}
