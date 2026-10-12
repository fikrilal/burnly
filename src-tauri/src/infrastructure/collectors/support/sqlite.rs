use std::path::Path;
#[cfg(test)]
use std::sync::{Arc, Condvar, Mutex};
#[cfg(test)]
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

#[cfg(test)]
pub(crate) struct SourceOpenGate {
    entered: Mutex<bool>,
    entered_cv: Condvar,
    release: Mutex<bool>,
    release_cv: Condvar,
    armed: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl SourceOpenGate {
    pub(crate) fn install() -> Arc<Self> {
        let gate = Arc::new(Self {
            entered: Mutex::new(false),
            entered_cv: Condvar::new(),
            release: Mutex::new(false),
            release_cv: Condvar::new(),
            armed: std::sync::atomic::AtomicBool::new(true),
        });
        *INSTALLED_SOURCE_OPEN_GATE.lock().expect("source open gate") = Some(gate.clone());
        gate
    }

    pub(crate) fn wait_until_entered(&self) {
        let mut entered = self.entered.lock().expect("source open entered");
        let start = std::time::Instant::now();
        while !*entered {
            let remaining = Duration::from_secs(5).saturating_sub(start.elapsed());
            if remaining.is_zero() {
                break;
            }
            let (guard, timeout) = self
                .entered_cv
                .wait_timeout(entered, remaining)
                .expect("source open wait");
            entered = guard;
            if timeout.timed_out() {
                break;
            }
        }
        assert!(*entered, "collector did not open a source database");
    }

    pub(crate) fn release(&self) {
        let mut release = self.release.lock().expect("source open release");
        *release = true;
        self.release_cv.notify_all();
    }
}

#[cfg(test)]
static INSTALLED_SOURCE_OPEN_GATE: Mutex<Option<Arc<SourceOpenGate>>> = Mutex::new(None);

#[cfg(test)]
pub(crate) fn clear_source_open_gate() {
    *INSTALLED_SOURCE_OPEN_GATE.lock().expect("source open gate") = None;
}

pub(in crate::infrastructure::collectors) fn open_external_read_only(
    path: impl AsRef<Path>,
) -> Result<Connection, rusqlite::Error> {
    wait_for_installed_source_open_gate();
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

fn wait_for_installed_source_open_gate() {
    #[cfg(test)]
    {
        let gate = INSTALLED_SOURCE_OPEN_GATE
            .lock()
            .expect("source open gate")
            .clone();
        if let Some(gate) = gate {
            if gate.armed.swap(false, std::sync::atomic::Ordering::SeqCst) {
                {
                    let mut entered = gate.entered.lock().expect("source open entered");
                    *entered = true;
                    gate.entered_cv.notify_all();
                }
                let mut release = gate.release.lock().expect("source open release");
                while !*release {
                    release = gate
                        .release_cv
                        .wait(release)
                        .expect("source open release wait");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn opens_existing_database_without_write_access() {
        let database = NamedTempFile::new().expect("temp database");
        let writable = Connection::open(database.path()).expect("writable database");
        writable
            .execute("CREATE TABLE fixture (id INTEGER PRIMARY KEY)", [])
            .expect("schema");
        drop(writable);

        let readonly = open_external_read_only(database.path()).expect("readonly database");

        let error = readonly
            .execute("INSERT INTO fixture (id) VALUES (1)", [])
            .expect_err("readonly write rejected");
        assert_eq!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        );
    }

    #[test]
    fn missing_database_is_not_created() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("missing.sqlite");

        let error = open_external_read_only(&path).expect_err("missing database");

        assert!(!path.exists());
        assert_eq!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::CannotOpen)
        );
    }
}
