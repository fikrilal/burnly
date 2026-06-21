use std::io;
use std::path::PathBuf;

use rusqlite::Error as SqliteError;
use rusqlite_migration::Error as MigrationError;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceErrorKind {
    InvalidPath,
    CreateDirectory,
    Open,
    Configure,
    PolicyMismatch,
    Migration,
    Backup,
    HealthCheck,
    Seed,
    Read,
    InvalidStoredValue,
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

    #[error("database migration failed")]
    Migration(#[source] MigrationError),

    #[error("database backup or restore failed")]
    Backup(#[source] SqliteError),

    #[error("database backup could not be published")]
    BackupPublish(#[source] io::Error),

    #[error("database health check failed: {check}")]
    HealthCheckQuery {
        check: &'static str,
        #[source]
        source: SqliteError,
    },

    #[error("database is unhealthy: {check} returned {detail}")]
    Unhealthy { check: &'static str, detail: String },

    #[error("failed to initialize application settings")]
    Seed(#[source] SqliteError),

    #[error("failed to read database value: {operation}")]
    Read {
        operation: &'static str,
        #[source]
        source: SqliteError,
    },

    #[error("database contains an invalid value: {field}")]
    InvalidStoredValue { field: &'static str },
}

impl PersistenceError {
    pub fn kind(&self) -> PersistenceErrorKind {
        match self {
            Self::InvalidPath { .. } => PersistenceErrorKind::InvalidPath,
            Self::CreateDirectory { .. } => PersistenceErrorKind::CreateDirectory,
            Self::Open { .. } => PersistenceErrorKind::Open,
            Self::Configure { .. } => PersistenceErrorKind::Configure,
            Self::PolicyMismatch { .. } => PersistenceErrorKind::PolicyMismatch,
            Self::Migration(_) => PersistenceErrorKind::Migration,
            Self::Backup(_) | Self::BackupPublish(_) => PersistenceErrorKind::Backup,
            Self::HealthCheckQuery { .. } | Self::Unhealthy { .. } => {
                PersistenceErrorKind::HealthCheck
            }
            Self::Seed(_) => PersistenceErrorKind::Seed,
            Self::Read { .. } => PersistenceErrorKind::Read,
            Self::InvalidStoredValue { .. } => PersistenceErrorKind::InvalidStoredValue,
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

    pub(super) fn migration(source: MigrationError) -> Self {
        Self::Migration(source)
    }

    pub(super) fn backup(source: SqliteError) -> Self {
        Self::Backup(source)
    }

    pub(super) fn backup_publish(source: io::Error) -> Self {
        Self::BackupPublish(source)
    }

    pub(super) fn health_check(check: &'static str, source: SqliteError) -> Self {
        Self::HealthCheckQuery { check, source }
    }

    pub(super) fn unhealthy(check: &'static str, detail: impl Into<String>) -> Self {
        Self::Unhealthy {
            check,
            detail: detail.into(),
        }
    }

    pub(super) fn seed(source: SqliteError) -> Self {
        Self::Seed(source)
    }

    pub(crate) fn read(operation: &'static str, source: SqliteError) -> Self {
        Self::Read { operation, source }
    }

    pub(crate) fn invalid_stored_value(field: &'static str) -> Self {
        Self::InvalidStoredValue { field }
    }
}
