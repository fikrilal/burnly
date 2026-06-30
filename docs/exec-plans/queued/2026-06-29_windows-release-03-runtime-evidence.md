# 2026-06-29 Windows Release 03 Runtime Evidence

## Objective

Verify Burnly runs correctly on Windows from the packaged `.exe` installer,
including tray behavior, refresh, launch-at-login, bundled `ccusage`, SQLite
storage, and updater execution.

## Acceptance Criteria

- Windows `.exe` installs Burnly on a real Windows machine or VM.
- Burnly launches and opens the tray panel.
- Packaged `ccusage` sidecar is found and can refresh usage data.
- SQLite database is created under the expected Windows app data location.
- Launch-at-login can be toggled and does not create a broken startup entry.
- Manual update check can detect a newer release.
- Update install and restart works from one released test version to another.
- Runtime evidence is recorded in this plan.

## Risk Class

`high`

## Impact Areas

- Windows runtime packaging
- Tray integration
- Sidecar execution
- SQLite paths
- Autostart integration
- Updater runtime

## Design Review

- What complexity is being introduced?
  - Windows shell/runtime behavior differs from Linux for tray, startup, paths,
    process execution, and installer security prompts.
- Which decisions are hidden inside the owning module?
  - Platform-specific path and launch-at-login behavior should remain behind
    Rust infrastructure/platform modules.
- Is each new interface simpler than its implementation?
  - The frontend should continue using the same IPC contracts.
- What special cases exist, and can the design eliminate them?
  - Windows filesystem paths and startup registration are platform-specific and
    should not leak into application/domain code.
- Why is each new abstraction needed now?
  - No new abstraction is expected unless runtime evidence reveals repeated
    platform branching.
- Can an existing module absorb this responsibility cleanly?
  - Existing platform modules and runtime services should absorb fixes.

## Checklist

- [x] Install the packaged Windows `.exe`.
- [x] Launch Burnly from Start menu or installed shortcut.
- [x] Open tray panel and verify runtime is available.
- [x] Trigger refresh and inspect refresh/import run state.
- [x] Verify packaged `ccusage` sidecar path and execution.
- [x] Verify database path and schema health.
- [ ] Toggle launch-at-login and verify restart/login behavior.
- [ ] Test updater from an older Windows build to a newer one.
- [x] Add a concrete Windows evidence contract and validator.
- [x] Correct platform behavior policy for Windows x64 updater evidence and
      deferred Windows ARM64 support.
- [ ] Record real Windows evidence and any platform fixes.
- [x] Run relevant gates after fixes.

## Test Plan

- Behavior and invariants to prove:
  - Packaged app refresh succeeds on Windows.
  - Sidecar lookup does not rely on Linux resource paths.
  - Tray panel can reach desktop runtime.
  - Update install restarts into the newer version.
  - Launch-at-login points at the installed app, not a dev server or stale path.
- Lowest stable test layer:
  - Manual Windows runtime evidence plus targeted tests for any code fixes.
- Failure paths:
  - Sidecar missing.
  - Runtime unavailable.
  - Refresh stuck in running state.
  - Updater unavailable or bad signature.
  - Launch-at-login broken after reboot.
- Fixtures or fakes:
  - Older and newer Windows release artifacts.
- Runtime or platform evidence:
  - Required. Record OS version, install path, app version, update path, and
    refresh status.
- Relevant commands:
  - `pnpm verify`
  - `pnpm verify:windows` if available on the Windows environment
  - Windows package install/update commands recorded during evidence collection

## Decisions

- Windows public release cannot proceed without real Windows runtime/update
  evidence.
- Windows x64 is the only Windows release target in this phase.
- Windows ARM64 remains deferred until it has a release workflow target and
  runtime evidence.
- Runtime evidence must be validated by
  `pnpm windows-runtime:evidence:check <evidence.json>`.

## Verification

- Command: `pnpm windows-runtime:evidence:check <temporary passing fixture>`
- Outcome: passed
- Command:
  `pnpm windows-runtime:evidence:check docs/engineering/evidence/windows-runtime-evidence.template.json`
- Outcome: failed as expected because the template contains pending
  placeholders, not real runtime evidence
- Command: `pnpm platform-behavior:test && pnpm platform-behavior:check`
- Outcome: passed
- Command: `pnpm format:check`
- Outcome: passed
- Command: `pnpm verify`
- Outcome: passed; duplication report printed existing non-failing clones
- Command: `pnpm install --frozen-lockfile`
- Outcome: passed on Windows x64.
- Command: `pnpm verify:windows`
- Outcome: initially failed because `rustc` was not installed or on `PATH`.
- Command:
  `winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements --silent`
- Outcome: passed; installed rustup, then repaired the pinned
  `1.95.0-x86_64-pc-windows-msvc` toolchain with
  `rustup toolchain install 1.95.0-x86_64-pc-windows-msvc --profile default --force`.
- Command: `pnpm verify:windows`
- Outcome: failed at `cargo clippy` because MSVC `link.exe` was missing.
- Command:
  `winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements --silent --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --norestart"`
- Outcome: passed; installed Visual Studio 2022 Build Tools with the C++
  workload.
- Command:
  `cmd --% /d /s /c "call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 && set PATH=%USERPROFILE%\.cargo\bin;%PATH% && pnpm rust:clippy"`
- Outcome: passed after fixing the non-Linux unused `appdir` parameter in the
  packaged resource directory resolver.
- Command:
  `cmd --% /d /s /c "call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 && set PATH=%USERPROFILE%\.cargo\bin;%PATH% && pnpm verify:windows"`
- Outcome: passed. Lint still reports existing warnings; duplication report
  still prints existing non-failing clones.
- Command:
  `gh release list --repo fikrilal/burnly --limit 10` and release asset
  inspection for `v0.1.2`, `v0.1.1`, and `burnly-v0.1.0`
- Outcome: passed; current GitHub releases contain Linux artifacts only. No
  public Windows release assets are available yet. This does not block local
  Windows installer/runtime evidence, but it does mean the updater path still
  needs an older/newer signed Windows test feed or draft release.
- Command: `pnpm install --frozen-lockfile; pnpm rebuild esbuild`
- Outcome: passed after setting the repo-local pnpm build-script approval
  `allowBuilds.esbuild: true`.
- Command:
  `cmd --% /d /s /c "call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 && set "PATH=%USERPROFILE%\.cargo\bin;%PATH%" && set "CI=true" && set "BURNLY_SIDECAR_TARGET=x86_64-pc-windows-msvc" && pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis"`
- Outcome: passed; produced
  `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Burnly_0.1.2_x64-setup.exe`.
- Command:
  `pnpm release:stage x86_64-pc-windows-msvc "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Burnly_0.1.2_x64-setup.exe"`
- Outcome: passed; staged
  `src-tauri/target/release-artifacts/burnly-v0.1.2-windows-x86_64.exe`.
- Command:
  `pnpm windows-smoke:exe "src-tauri/target/release-artifacts/burnly-v0.1.2-windows-x86_64.exe"`
- Outcome: passed; installer size was `6197984` bytes.
- Command:
  `Start-Process src-tauri/target/release-artifacts/burnly-v0.1.2-windows-x86_64.exe -ArgumentList /S -Wait`
- Outcome: passed; installed current-user app to
  `C:\Users\fikrilal\AppData\Local\Burnly`.
- Command: launch installed app from
  `C:\Users\fikrilal\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Burnly.lnk`
- Outcome: passed; shortcut targets
  `C:\Users\fikrilal\AppData\Local\Burnly\burnly.exe`, and `burnly.exe`
  launched from the installed path.
- Command:
  `C:\Users\fikrilal\AppData\Local\Burnly\sidecars\ccusage\ccusage.exe --version`
- Outcome: passed; output was `ccusage 20.0.14`.
- Command: SQLite inspection of
  `C:\Users\fikrilal\AppData\Roaming\app.burnly.desktop\burnly.sqlite3`
- Outcome: passed; `pragma integrity_check` returned `ok`, schema tables were
  present, launch refresh run `1` succeeded, and all six initial import runs
  succeeded.
- Command: manual tray refresh followed by SQLite inspection
- Outcome: passed; refresh run `2` had trigger `manual` and status
  `succeeded`, proving the tray panel reached the desktop runtime and refresh
  IPC worked in the installed app.

## Runtime Evidence

- Required before completing this phase.
- Windows environment confirmed:
  - OS: `Microsoft Windows NT 10.0.26200.0`
  - PowerShell: `5.1.26100.8457`
  - Node: `v22.13.0`
  - pnpm: `10.9.0`
  - Rust host: `x86_64-pc-windows-msvc`
- Runtime install/update evidence is blocked because no signed Windows x64
  older/newer installer pair and updater feed have been exercised yet. Existing
  public releases `v0.1.2`, `v0.1.1`, and `burnly-v0.1.0` publish Linux assets
  only; local Windows install/runtime evidence is not blocked by that and is
  now partially collected.
- Local installer evidence collected:
  - Installer:
    `C:\Development\_SIDE\burnly\src-tauri\target\release-artifacts\burnly-v0.1.2-windows-x86_64.exe`
  - Install path: `C:\Users\fikrilal\AppData\Local\Burnly`
  - Start menu shortcut:
    `C:\Users\fikrilal\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Burnly.lnk`
  - App data path:
    `C:\Users\fikrilal\AppData\Roaming\app.burnly.desktop`
  - Database path:
    `C:\Users\fikrilal\AppData\Roaming\app.burnly.desktop\burnly.sqlite3`
  - Packaged sidecar:
    `C:\Users\fikrilal\AppData\Local\Burnly\sidecars\ccusage\ccusage.exe`
  - Observed sidecar version: `ccusage 20.0.14`
  - Latest observed refresh/import status: `succeeded`
- Launch-at-login evidence is still pending. Current DB setting remains
  `launch_at_login = 0`, and `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
  has no Burnly startup value yet.
- Updater evidence is still pending. It requires an installed older Windows
  build and a newer signed Windows build discoverable through updater metadata.
- Do not move this plan to completed until Windows release artifacts are
  produced, installed, exercised, recorded in a real evidence JSON, and
  validated with `pnpm windows-runtime:evidence:check <evidence.json>`.

## Follow-Up Debt

- Phase 4 should resolve code-signing and public install documentation.
