# Antigravity Windows Collector Handoff

## Context

The Linux Antigravity collector is working for Burnly, but Windows users report
no Antigravity usage. This is expected with the current code: the collector is
registered on all platforms, but runtime endpoint discovery only has a Linux
implementation.

Current branch/state at handoff time:

- Branch: `main`
- HEAD: `27d4b14 chore(release): v0.1.12`
- Tag: `v0.1.12`
- `main` is aligned with `origin/main`
- There are uncommitted tray UI changes:
  - `src/features/tray/TrayPanel.tsx`
  - `src/features/tray/TrayPanel.test.tsx`
- Those uncommitted changes fix tray height by changing the tray surface from
  `min-h-screen` to `h-screen`, so long model lists scroll inside the panel
  instead of making the panel visually grow. Do not discard them unless the user
  explicitly asks.

## Confirmed Diagnosis

Antigravity collector registration happens unconditionally:

- `src-tauri/src/bootstrap.rs`
- `build_refresh_coordinator`
- `AntigravityCollector::new()`

Runtime discovery is the blocker:

- `src-tauri/src/infrastructure/collectors/antigravity/discovery.rs`
- `current_processes()` returns real processes only on Linux.
- On non-Linux platforms it returns `Vec::new()`, so discovery always finds no
  runtime endpoints.

Relevant code:

```rust
#[cfg(target_os = "linux")]
fn current_processes() -> Vec<ProcessSnapshot> {
    linux::current_processes()
}

#[cfg(not(target_os = "linux"))]
fn current_processes() -> Vec<ProcessSnapshot> {
    Vec::new()
}
```

Result on Windows:

- `RuntimeDiscovery::current().discover()` returns an empty endpoint list.
- `AntigravityCollector::detect()` reports `antigravity.runtime_unavailable`.
- `AntigravityCollector::collect()` returns source-not-found/runtime-missing.
- No Antigravity rows are imported.

## Existing Linux Architecture

Files:

- `src-tauri/src/infrastructure/collectors/antigravity/discovery.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/conversation_index.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/runtime_client.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/usage_extractor.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/mapper.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`

Discovery model:

1. Enumerate local processes.
2. Classify a process as one of:
   - `antigravity`
   - `antigravity-ide`
   - `antigravity-cli`
3. Extract CSRF token from `--csrf_token` when present.
4. Find loopback listening ports owned by that process.
5. Probe the local runtime through `RuntimeClient`.
6. Use local conversation artifacts under `.gemini/<variant>/conversations`.
7. Request stream frames for each conversation id and extract token counters.

Current product variants:

- `AntigravityProductVariant::App` -> `antigravity`
- `AntigravityProductVariant::Ide` -> `antigravity-ide`
- `AntigravityProductVariant::Cli` -> `antigravity-cli`

## Windows Work Needed

### 1. Add Windows process discovery

Implement a Windows-specific process snapshot provider in
`src-tauri/src/infrastructure/collectors/antigravity/discovery.rs`.

Target shape should match the existing internal type:

```rust
ProcessSnapshot {
    process_id: u32,
    executable: Option<PathBuf>,
    command: Vec<String>,
    listeners: Vec<LocalListener>,
}
```

Recommended discovery strategy:

1. Enumerate running processes with command lines.
2. Enumerate TCP listening endpoints with owning process IDs.
3. Join listening endpoints to process IDs.
4. Keep loopback listeners only, as the existing discovery layer does.

Implementation options:

- Prefer a Rust crate already acceptable in the project if present.
- If adding a dependency, keep it narrow and justify it. Good candidates to
  inspect:
  - Windows API through `windows-sys`
  - `sysinfo` if command line + process ID are enough, but port ownership still
    needs Windows network table access.
- Avoid shelling out to `netstat`, `wmic`, or PowerShell for production logic.
  Those are brittle, localized, and slow. They are acceptable only for manual
  local investigation notes.

Windows APIs likely needed:

- Process enumeration / command line:
  - WMI/CIM is easier but adds runtime fragility.
  - Toolhelp/process APIs plus command-line lookup is more work.
- TCP listener ownership:
  - `GetExtendedTcpTable` / `MIB_TCPTABLE_OWNER_PID`
  - Include IPv4 first; IPv6 can be added if needed.

Minimum viable Windows collector can support IPv4 loopback first:

- `127.0.0.1:<port>`
- Add IPv6 `::1` after the basic route works.

### 2. Make classifier Windows-aware

Current classifier has Linux path checks:

```rust
command_contains(&process.command, "/opt/antigravity-ide/Antigravity-IDE")
command_contains(&process.command, "/opt/antigravity/Antigravity-x64")
```

Add Windows-aware signals without breaking Linux:

- IDE:
  - `--app_data_dir antigravity-ide`
  - executable or command path containing `Antigravity-IDE`
  - language server executable name may include `language_server_windows_x64`
- CLI:
  - executable ending with `agy.exe` or `agy`
  - command containing `antigravity-cli`
- App / 2.0:
  - `--app_data_dir antigravity`
  - `--override_ide_name antigravity`
  - executable or command path containing `Antigravity`

Be careful: generic `command_contains("Antigravity")` can classify IDE as App if
ordered incorrectly. Keep IDE checks first, then CLI, then App.

### 3. Add Windows data root lookup

Current conversation index default uses `HOME/.gemini`:

```rust
std::env::var_os("HOME")
    .map(PathBuf::from)
    .map(Self::from_home)
```

On Windows, inspect where Antigravity stores the `.gemini` equivalent. Good
places to check on the Windows machine:

- `%USERPROFILE%\.gemini`
- `%APPDATA%\gemini`
- `%LOCALAPPDATA%\Google\...`
- `%LOCALAPPDATA%\Antigravity...`
- Antigravity process args for `--user-data-dir`, `--app_data_dir`, or similar.

The collector expects variant directories:

```text
<data_root>/antigravity/conversations/*.pb|*.db
<data_root>/antigravity-ide/conversations/*.pb|*.db
<data_root>/antigravity-cli/conversations/*.pb|*.db
```

If Windows also uses `%USERPROFILE%\.gemini`, the code may only need a fallback
from `USERPROFILE` when `HOME` is missing. If it uses app-data directories, add a
platform-specific default resolver.

Suggested shape:

```rust
impl ConversationIndex {
    pub(crate) fn default() -> Self {
        default_data_root()
            .map(Self::from_data_root)
            .unwrap_or_else(|| Self::from_data_root(".gemini"))
    }
}
```

With platform-specific `default_data_root()` helpers.

### 4. Preserve privacy

The Linux implementation redacts CLI prompt args before building snapshots:

- `--prompt`
- `--prompt-interactive`

Windows implementation must preserve this behavior. Do not log full command
lines or local project paths in collection failures.

### 5. Tests to add

Add unit tests at the stable layer, not OS integration tests only.

Recommended tests:

- Classifier recognizes Windows IDE command shape.
- Classifier recognizes Windows CLI `agy.exe`.
- Classifier recognizes Windows App/2.0 command shape.
- Windows process snapshot builder redacts prompt arguments.
- Windows TCP listener table parser maps PID to loopback port.
- Non-loopback listener is ignored.
- Empty Windows discovery still returns `antigravity.runtime_unavailable` without
  crashing.
- Conversation index default root can resolve Windows-style home/appdata paths.

Existing test style is in `discovery.rs` and `conversation_index.rs`.

### 6. Manual Windows investigation commands

These are for local diagnosis only, not production implementation:

PowerShell process command lines:

```powershell
Get-CimInstance Win32_Process |
  Where-Object { $_.CommandLine -match "antigravity|agy|language_server" } |
  Select-Object ProcessId, ExecutablePath, CommandLine |
  Format-List
```

Listening ports with owning PID:

```powershell
Get-NetTCPConnection -State Listen |
  Where-Object { $_.LocalAddress -eq "127.0.0.1" -or $_.LocalAddress -eq "::1" } |
  Select-Object LocalAddress, LocalPort, OwningProcess
```

Find conversation artifacts:

```powershell
Get-ChildItem -Path $env:USERPROFILE,$env:APPDATA,$env:LOCALAPPDATA `
  -Recurse -Force -ErrorAction SilentlyContinue `
  -Include *.pb,*.db |
  Where-Object { $_.FullName -match "antigravity|gemini|conversation" } |
  Select-Object FullName, LastWriteTime, Length
```

After implementing, run Burnly refresh and inspect database:

```powershell
sqlite3 "$env:LOCALAPPDATA\\app.burnly.desktop\\burnly.sqlite3" `
  "select source, model_name, total_tokens from daily_model_usage d join daily_usage u on u.id = d.daily_usage_id where u.source = 'antigravity' order by total_tokens desc;"
```

Adjust DB path if Tauri uses a different app data directory on Windows.

## Verification Commands

From repo root:

```sh
pnpm vitest run src/features/tray/TrayPanel.test.tsx
pnpm verify:fast
pnpm verify
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
```

On Windows specifically, also run:

```powershell
pnpm verify:fast
pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis
```

If only testing collector logic before packaging:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
```

## Release Note If Fixed

Suggested release note:

```md
## What changed

- Added Windows runtime discovery for the experimental Antigravity collector.
- Burnly can now collect Antigravity usage on Windows when Antigravity is running
  and local conversation artifacts are present.
- Preserved local-only collection behavior; no proxying or account integration
  is required.

## Verification

- Passed Antigravity collector tests.
- Passed `pnpm verify`.
- Validated on a real Windows install with Antigravity running.
```

## Known Non-Goal

Do not intercept or proxy Antigravity network traffic. The accepted approach is
local runtime discovery + local RPC + local conversation artifact IDs only.

