use std::{
    ffi::OsString,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crate::application::{
    collection::{CollectorFailure, CollectorFailureCode, CollectorFailureContext},
    ports::collector::CancellationSignal,
};

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const TERMINATION_GRACE: Duration = Duration::from_millis(100);
const STDERR_SUMMARY_CHARS: usize = 512;

#[derive(Debug)]
pub(crate) struct ProcessRequest {
    program: PathBuf,
    arguments: Vec<OsString>,
    working_directory: PathBuf,
    environment: Vec<(OsString, OsString)>,
}

impl ProcessRequest {
    pub(crate) fn new(
        program: PathBuf,
        arguments: Vec<OsString>,
        working_directory: PathBuf,
        environment: Vec<(OsString, OsString)>,
    ) -> Self {
        Self {
            program,
            arguments,
            working_directory,
            environment,
        }
    }

    #[cfg(test)]
    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    #[cfg(test)]
    pub(crate) fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    #[cfg(test)]
    pub(crate) fn environment(&self) -> &[(OsString, OsString)] {
        &self.environment
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProcessLimits {
    timeout: Duration,
    stdout_bytes: usize,
    stderr_bytes: usize,
}

impl ProcessLimits {
    pub(crate) const fn collection() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            stdout_bytes: 16 * 1024 * 1024,
            stderr_bytes: 256 * 1024,
        }
    }

    pub(crate) const fn version_check() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            stdout_bytes: 64 * 1024,
            stderr_bytes: 16 * 1024,
        }
    }

    #[cfg(test)]
    pub(crate) const fn test(timeout: Duration, stdout_bytes: usize, stderr_bytes: usize) -> Self {
        Self {
            timeout,
            stdout_bytes,
            stderr_bytes,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ProcessOutput {
    pub stdout: String,
    pub stderr_summary: Option<String>,
    pub context: CollectorFailureContext,
}

pub(crate) fn execute(
    request: &ProcessRequest,
    cancellation: &dyn CancellationSignal,
    limits: ProcessLimits,
) -> Result<ProcessOutput, CollectorFailure> {
    if cancellation.is_cancelled() {
        return Err(failure(
            CollectorFailureCode::Cancelled,
            Instant::now(),
            0,
            0,
            None,
        ));
    }

    let redactions = redaction_values(request);
    let started = Instant::now();
    let mut child = spawn(request).map_err(|error| {
        let code = if error.kind() == io::ErrorKind::NotFound {
            CollectorFailureCode::BinaryMissing
        } else {
            CollectorFailureCode::SpawnFailed
        };
        failure(code, started, 0, 0, None)
    })?;
    let stdout = child.stdout.take().expect("stdout is configured as piped");
    let stderr = child.stderr.take().expect("stderr is configured as piped");
    let stdout_capture = Capture::start(stdout, limits.stdout_bytes);
    let stderr_capture = Capture::start(stderr, limits.stderr_bytes);

    let status = loop {
        let stop_code = if cancellation.is_cancelled() {
            Some(CollectorFailureCode::Cancelled)
        } else if stdout_capture.exceeded() {
            Some(CollectorFailureCode::StdoutLimitExceeded)
        } else if stderr_capture.exceeded() {
            Some(CollectorFailureCode::StderrLimitExceeded)
        } else if started.elapsed() >= limits.timeout {
            Some(CollectorFailureCode::TimedOut)
        } else {
            None
        };

        if let Some(code) = stop_code {
            terminate_and_reap(&mut child);
            let (stdout, _) = stdout_capture.finish();
            let (stderr, _) = stderr_capture.finish();
            return Err(failure(code, started, stdout.len(), stderr.len(), None));
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_error) => {
                terminate_and_reap(&mut child);
                let (stdout, _) = stdout_capture.finish();
                let (stderr, _) = stderr_capture.finish();
                return Err(failure(
                    CollectorFailureCode::SpawnFailed,
                    started,
                    stdout.len(),
                    stderr.len(),
                    None,
                ));
            }
        }
    };

    let (stdout, stdout_exceeded) = stdout_capture.finish();
    let (stderr, stderr_exceeded) = stderr_capture.finish();
    let exit_code = status.code();
    let context = context(started, stdout.len(), stderr.len(), exit_code);

    if cancellation.is_cancelled() {
        return Err(
            CollectorFailure::new(CollectorFailureCode::Cancelled, None, None)
                .with_context(context),
        );
    }
    if stdout_exceeded {
        return Err(
            CollectorFailure::new(CollectorFailureCode::StdoutLimitExceeded, None, None)
                .with_context(context),
        );
    }
    if stderr_exceeded {
        return Err(
            CollectorFailure::new(CollectorFailureCode::StderrLimitExceeded, None, None)
                .with_context(context),
        );
    }

    let stdout = String::from_utf8(stdout).map_err(|_| {
        CollectorFailure::new(CollectorFailureCode::NonUtf8Output, None, None).with_context(context)
    })?;
    let stderr = String::from_utf8(stderr).map_err(|_| {
        CollectorFailure::new(CollectorFailureCode::NonUtf8Output, None, None).with_context(context)
    })?;
    if !status.success() {
        return Err(
            CollectorFailure::new(CollectorFailureCode::NonzeroExit, None, None)
                .with_context(context),
        );
    }

    Ok(ProcessOutput {
        stdout,
        stderr_summary: summarize_stderr(&stderr, &redactions),
        context,
    })
}

fn spawn(request: &ProcessRequest) -> io::Result<Child> {
    let mut command = Command::new(&request.program);
    command
        .args(&request.arguments)
        .current_dir(&request.working_directory)
        .env_clear()
        .envs(request.environment.iter().cloned())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);
    command.spawn()
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

struct Capture {
    handle: thread::JoinHandle<Vec<u8>>,
    exceeded: Arc<AtomicBool>,
}

impl Capture {
    fn start(mut reader: impl Read + Send + 'static, limit: usize) -> Self {
        let exceeded = Arc::new(AtomicBool::new(false));
        let thread_exceeded = Arc::clone(&exceeded);
        let handle = thread::spawn(move || {
            let mut captured = Vec::with_capacity(limit.min(64 * 1024));
            let mut buffer = [0_u8; 8192];
            loop {
                let count = match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                let remaining = limit.saturating_sub(captured.len());
                captured.extend_from_slice(&buffer[..count.min(remaining)]);
                if count > remaining {
                    thread_exceeded.store(true, Ordering::Release);
                }
            }
            captured
        });
        Self { handle, exceeded }
    }

    fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }

    fn finish(self) -> (Vec<u8>, bool) {
        let captured = self.handle.join().unwrap_or_default();
        let exceeded = self.exceeded.load(Ordering::Acquire);
        (captured, exceeded)
    }
}

fn failure(
    code: CollectorFailureCode,
    started: Instant,
    stdout_bytes: usize,
    stderr_bytes: usize,
    exit_code: Option<i32>,
) -> CollectorFailure {
    CollectorFailure::new(code, None, None).with_context(context(
        started,
        stdout_bytes,
        stderr_bytes,
        exit_code,
    ))
}

fn context(
    started: Instant,
    stdout_bytes: usize,
    stderr_bytes: usize,
    exit_code: Option<i32>,
) -> CollectorFailureContext {
    CollectorFailureContext {
        runtime_ms: Some(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        stdout_bytes: Some(stdout_bytes as u64),
        stderr_bytes: Some(stderr_bytes as u64),
        exit_code,
    }
}

fn redaction_values(request: &ProcessRequest) -> Vec<String> {
    let mut values = Vec::new();
    push_path_redactions(&mut values, &request.program);
    push_path_redactions(&mut values, &request.working_directory);
    for (key, value) in &request.environment {
        if matches!(
            key.to_str(),
            Some("HOME" | "USERPROFILE" | "APPDATA" | "LOCALAPPDATA")
        ) {
            push_path_redactions(&mut values, Path::new(value));
        }
    }
    values.retain(|value| !value.is_empty());
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
}

fn push_path_redactions(values: &mut Vec<String>, path: &Path) {
    values.push(path.to_string_lossy().into_owned());
    if let Ok(canonical) = path.canonicalize() {
        values.push(canonical.to_string_lossy().into_owned());
    }
}

fn summarize_stderr(stderr: &str, redactions: &[String]) -> Option<String> {
    let mut summary = stderr.to_owned();
    for value in redactions {
        summary = summary.replace(value, "<redacted>");
    }
    let summary = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    if summary.is_empty() {
        return None;
    }
    Some(summary.chars().take(STDERR_SUMMARY_CHARS).collect())
}

fn terminate_and_reap(child: &mut Child) {
    request_termination(child);
    let deadline = Instant::now() + TERMINATION_GRACE;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => {
                force_termination(child);
                return;
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    force_termination(child);
    let _ = child.wait();
}

#[cfg(unix)]
fn request_termination(child: &Child) {
    // SAFETY: the process group ID is derived from the child created as its leader.
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGTERM);
    }
}

#[cfg(not(unix))]
fn request_termination(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(unix)]
fn force_termination(child: &Child) {
    // SAFETY: the process group ID is derived from the child created as its leader.
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn force_termination(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, os::unix::fs::symlink, sync::atomic::AtomicBool};

    use super::*;

    struct TestCancellation(AtomicBool);

    impl TestCancellation {
        fn active() -> Self {
            Self(AtomicBool::new(false))
        }

        fn cancelled() -> Self {
            Self(AtomicBool::new(true))
        }
    }

    impl CancellationSignal for TestCancellation {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::Acquire)
        }
    }

    #[test]
    fn captures_success_with_closed_stdin_filtered_environment_and_redacted_stderr() {
        let fixture = Fixture::new();
        let request = fixture.request("success");

        let output = execute(
            &request,
            &TestCancellation::active(),
            ProcessLimits::test(Duration::from_secs(1), 1024, 1024),
        )
        .expect("successful process");

        assert_eq!(output.stdout, "stdin-closed env-filtered\n");
        assert_eq!(output.stderr_summary.as_deref(), Some("path=<redacted>"));
        assert_eq!(output.context.exit_code, Some(0));
    }

    #[test]
    fn redacts_canonical_path_aliases_reported_by_child_processes() {
        let fixture = Fixture::new();
        let real_directory = fixture.directory.path().join("real");
        let alias = fixture.directory.path().join("alias");
        fs::create_dir(&real_directory).expect("create real directory");
        symlink(&real_directory, &alias).expect("create directory alias");
        let request = ProcessRequest::new(PathBuf::from("/bin/sh"), Vec::new(), alias, Vec::new());

        let stderr = format!("path={}", real_directory.display());
        assert_eq!(
            summarize_stderr(&stderr, &redaction_values(&request)).as_deref(),
            Some("path=<redacted>"),
        );
    }

    #[test]
    fn maps_missing_and_unspawnable_binaries() {
        let fixture = Fixture::new();
        let missing = ProcessRequest::new(
            fixture.directory.path().join("missing"),
            Vec::new(),
            fixture.directory.path().to_path_buf(),
            Vec::new(),
        );
        assert_code(
            execute(
                &missing,
                &TestCancellation::active(),
                ProcessLimits::test(Duration::from_secs(1), 1024, 1024),
            ),
            CollectorFailureCode::BinaryMissing,
        );

        let unspawnable = fixture.directory.path().join("unspawnable");
        fs::write(&unspawnable, "not executable").expect("write unspawnable fixture");
        let request = ProcessRequest::new(
            unspawnable,
            Vec::new(),
            fixture.directory.path().to_path_buf(),
            Vec::new(),
        );
        assert_code(
            execute(
                &request,
                &TestCancellation::active(),
                ProcessLimits::test(Duration::from_secs(1), 1024, 1024),
            ),
            CollectorFailureCode::SpawnFailed,
        );
    }

    #[test]
    fn enforces_stdout_and_stderr_limits() {
        let fixture = Fixture::new();
        for (mode, expected) in [
            ("stdout-limit", CollectorFailureCode::StdoutLimitExceeded),
            ("stderr-limit", CollectorFailureCode::StderrLimitExceeded),
        ] {
            assert_code(
                execute(
                    &fixture.request(mode),
                    &TestCancellation::active(),
                    ProcessLimits::test(Duration::from_secs(1), 32, 32),
                ),
                expected,
            );
        }
    }

    #[test]
    fn enforces_timeout_and_pre_spawn_cancellation() {
        let fixture = Fixture::new();
        assert_code(
            execute(
                &fixture.request("sleep"),
                &TestCancellation::active(),
                ProcessLimits::test(Duration::from_millis(30), 1024, 1024),
            ),
            CollectorFailureCode::TimedOut,
        );
        assert_code(
            execute(
                &fixture.request("success"),
                &TestCancellation::cancelled(),
                ProcessLimits::test(Duration::from_secs(1), 1024, 1024),
            ),
            CollectorFailureCode::Cancelled,
        );
    }

    #[test]
    fn cancels_running_child_and_reaps_it() {
        let fixture = Fixture::new();
        let cancellation = Arc::new(TestCancellation::active());
        let trigger = Arc::clone(&cancellation);
        let thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            trigger.0.store(true, Ordering::Release);
        });

        assert_code(
            execute(
                &fixture.request("sleep"),
                cancellation.as_ref(),
                ProcessLimits::test(Duration::from_secs(2), 1024, 1024),
            ),
            CollectorFailureCode::Cancelled,
        );
        thread.join().expect("cancellation trigger");
    }

    #[test]
    fn classifies_nonzero_and_non_utf8_output() {
        let fixture = Fixture::new();
        for (mode, expected) in [
            ("nonzero", CollectorFailureCode::NonzeroExit),
            ("non-utf8", CollectorFailureCode::NonUtf8Output),
        ] {
            assert_code(
                execute(
                    &fixture.request(mode),
                    &TestCancellation::active(),
                    ProcessLimits::test(Duration::from_secs(1), 1024, 1024),
                ),
                expected,
            );
        }
    }

    fn assert_code(
        result: Result<ProcessOutput, CollectorFailure>,
        expected: CollectorFailureCode,
    ) {
        assert_eq!(result.expect_err("process failure").code, expected);
    }

    struct Fixture {
        directory: tempfile::TempDir,
        script: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("fixture directory");
            Self {
                directory,
                script: fixture_path("fake-collector.sh"),
            }
        }

        fn request(&self, mode: &str) -> ProcessRequest {
            ProcessRequest::new(
                PathBuf::from("/bin/sh"),
                vec![self.script.as_os_str().to_owned(), OsString::from(mode)],
                self.directory.path().to_path_buf(),
                vec![(
                    OsString::from("HOME"),
                    self.directory.path().as_os_str().to_owned(),
                )],
            )
        }
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("tests/fixtures/collectors/ccusage/process")
            .join(name)
    }
}
