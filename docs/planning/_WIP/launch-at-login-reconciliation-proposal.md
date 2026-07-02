# Launch At Login Reconciliation Engineering Proposal

## Status

Engineering proposal.

This proposal covers fixing launch-at-login drift between Burnly's persisted
setting and the operating system autostart registration across Linux, Windows,
and macOS. It is not an execution plan and does not approve implementation by
itself.

## Context

Burnly exposes a `Launch at login` setting. The setting is persisted in SQLite
as:

```text
app_settings.launch_at_login
```

Native OS registration is handled by `tauri-plugin-autostart`, backed by
`auto-launch`.

The previous launch-at-login hardening work made settings updates safer:

- native apply failures do not persist a successful setting,
- persistence failures after native changes attempt rollback,
- debug builds do not expose launch-at-login as supported,
- packaged builds expose the capability.

That work only covered the update path. It did not make startup reconcile the
persisted setting against the actual OS registration.

## Observed Failure

On July 1, 2026, local Linux state showed:

```text
app_settings.launch_at_login = 1
```

but no Burnly entry existed in:

```text
~/.config/autostart/
```

Burnly therefore believed launch-at-login was enabled, while GNOME had nothing
to launch after reboot.

The installed app desktop entry existed:

```text
~/.local/share/applications/burnly.desktop
```

but that is not an autostart entry. It only makes the app discoverable through
desktop launchers.

There was also stale residue:

```text
~/.local/share/applications/Burnly.desktop
```

with:

```ini
[Desktop Entry]
Type=Application
Name=Burnly
Hidden=true
```

That file is not in `~/.config/autostart`, so it does not start Burnly on login.
It does, however, show that stale launch metadata can exist after earlier
install/update attempts.

## Root Cause

The current runtime only calls native autostart apply when the setting changes:

```text
SettingsService::update
  -> SettingsRuntime::prepare_update
     -> DesktopSettingsRuntime::apply_launch_at_login(...)
```

During normal startup, Burnly reads persisted settings but does not verify that
the OS registration still matches:

```text
SQLite says enabled
OS autostart file/registry/launch-agent is missing
=> Burnly starts with stale setting and does not repair it
```

This drift can happen after:

- reinstalling or moving the app,
- AppImage path changes,
- Windows installer path changes,
- macOS app bundle copy/move,
- OS cleanup tools,
- manual deletion of startup entries,
- older buggy versions writing incomplete startup metadata,
- plugin behavior changes across platform releases.

## Recommendation

Add startup reconciliation for launch-at-login in packaged builds.

If persisted `launch_at_login` is `true`, Burnly should ensure native
launch-at-login registration exists and points at the current installed app.

If persisted `launch_at_login` is `false`, Burnly should not aggressively remove
unknown OS entries on startup. Disable should remain user-driven through the
Settings UI. Startup can optionally log diagnostics if an enabled OS entry is
found while the DB setting is false, but it should not mutate the OS in that
case for the first implementation.

## Product Policy

Source of truth for user intent:

```text
app_settings.launch_at_login
```

Source of truth for actual launch behavior:

```text
OS autostart registration
```

Burnly must keep them aligned when user intent is enabled.

Startup behavior:

| Persisted setting | OS registration | Startup action                                           |
| ----------------- | --------------- | -------------------------------------------------------- |
| `true`            | missing         | create registration for the current installed app        |
| `true`            | stale path      | replace registration with the current installed app path |
| `true`            | valid           | no-op                                                    |
| `false`           | missing         | no-op                                                    |
| `false`           | present         | no-op initially; optionally report diagnostics           |

Startup repair failure should not prevent Burnly from starting. Launch-at-login
is a convenience capability, not a core runtime dependency.

## Cross-Platform Registration Targets

### Linux

`tauri-plugin-autostart` uses `auto-launch` to write:

```text
~/.config/autostart/<app_name>.desktop
```

Observed `auto-launch` Linux behavior writes:

```ini
[Desktop Entry]
Type=Application
Version=1.0
Name=<app_name>
Comment=<app_name>startup script
Exec=<app_path> <args>
StartupNotify=false
Terminal=false
```

For Burnly AppImage builds, the plugin uses Tauri's AppImage environment path
when available. That should resolve to the installed AppImage path:

```text
~/.local/share/burnly/Burnly.AppImage
```

Linux reconciliation should call the plugin instead of manually writing the
file so app path selection stays consistent with Tauri packaging behavior.

Required Linux evidence:

- `~/.config/autostart/<app_name>.desktop` exists after enabling or startup
  reconciliation.
- `Exec=` points at the installed AppImage, not a dev binary or stale AppImage.
- reboot/login starts Burnly and the tray icon appears when the host provides a
  StatusNotifier/AppIndicator implementation.

### Windows

`tauri-plugin-autostart` uses `auto-launch` to write a user-level Run entry:

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Run\<app_name>
```

Windows reconciliation should call the plugin to create or update the registry
value when `launch_at_login = true`.

Required Windows evidence:

- Registry value exists under the current user Run key.
- Registry value points at the installed Burnly executable, not a dev path.
- reboot/login starts Burnly.
- single-instance behavior prevents a second visible instance when the user
  launches Burnly manually after login.

### macOS

Burnly initializes the plugin with:

```rust
tauri_plugin_autostart::MacosLauncher::LaunchAgent
```

For macOS, reconciliation should call the plugin to create or update the user
LaunchAgent when `launch_at_login = true`.

The app is currently distributed as an unsigned DMG preview. Gatekeeper affects
first launch and quarantine, but once the user has installed and opened Burnly,
launch-at-login should still be controlled by the user-level LaunchAgent.

Required macOS evidence:

- LaunchAgent exists after enabling or startup reconciliation.
- LaunchAgent points at the installed `/Applications/Burnly.app/...` executable
  or the path selected by the plugin.
- reboot/login starts Burnly as a menu-bar app.
- Burnly remains outside the Dock when launched at login.

## Proposed Architecture

Keep launch-at-login OS interaction behind the existing desktop runtime
boundary.

Current update path:

```text
SettingsService
  -> SettingsRuntime
     -> DesktopSettingsRuntime
        -> tauri_plugin_autostart
```

Add a startup reconciliation path owned by runtime bootstrap:

```text
setup_runtime
  -> read app_settings
  -> construct DesktopSettingsRuntime
  -> reconcile_launch_at_login_on_startup(settings.launch_at_login)
  -> continue startup regardless of repair failure
```

Recommended shape:

```rust
impl<R: Runtime> DesktopSettingsRuntime<R> {
    fn reconcile_launch_at_login_on_startup(
        &self,
        enabled: bool,
    ) -> Result<(), RuntimeSettingError> {
        if !enabled {
            return Ok(());
        }
        if !launch_at_login_supported() {
            return Ok(());
        }
        self.apply_launch_at_login(true)
    }
}
```

This intentionally uses `enable()` even if the plugin reports already enabled.
The desired property is idempotent repair: the registration should exist and
point at the current app path after startup.

If the plugin exposes `is_enabled()`, Burnly can use it for diagnostics, but it
should not depend on it for correctness unless the plugin can also detect stale
paths. A simple `enable()` call is more robust because it overwrites the Linux
desktop file and Windows/macOS registration with current values.

## Error Handling

Startup reconciliation failure should:

- not fail app startup,
- not flip `app_settings.launch_at_login` to false,
- be logged with a stable diagnostic message,
- optionally update capability status in memory in a future iteration.

Recommended first implementation:

```text
eprintln!("Burnly launch-at-login reconciliation failed: {error:?}");
```

Do not surface an in-tray error banner unless we add a general settings health
surface. A startup warning is enough for MVP because the user can still toggle
the setting off/on manually.

## Privacy And Safety

The repair path must not inspect user data, shell history, or unrelated desktop
entries.

It may inspect or write only the OS registration managed by
`tauri-plugin-autostart` for Burnly.

Do not delete arbitrary `Burnly.desktop` files from user application directories
as part of startup reconciliation. Stale cleanup should be explicit installer
maintenance, not runtime magic.

## Tests

Add focused Rust tests around the policy, not the OS-specific plugin side
effects.

Recommended unit-testable helper:

```rust
fn should_reconcile_launch_at_login_on_startup(
    persisted_enabled: bool,
    supported: bool,
) -> bool
```

Expected matrix:

| Persisted enabled | Runtime supported | Should repair |
| ----------------- | ----------------- | ------------- |
| `true`            | `true`            | `true`        |
| `true`            | `false`           | `false`       |
| `false`           | `true`            | `false`       |
| `false`           | `false`           | `false`       |

Add a mock runtime test that proves startup repair failure is non-fatal. If
mocking `tauri_plugin_autostart` directly is awkward, keep the helper pure and
cover `DesktopSettingsRuntime` with a narrow integration smoke in packaged
runtime evidence.

Existing settings update tests should remain unchanged:

- stale revisions do not apply native side effects,
- native apply failures do not persist settings,
- persistence failures attempt native rollback,
- debug builds keep launch-at-login unavailable.

## Runtime Evidence

Because launch-at-login is OS behavior, final proof requires installed runtime
evidence on every supported OS.

Required evidence per platform:

- enable launch-at-login from Settings,
- verify native OS registration exists,
- remove the native OS registration while keeping SQLite enabled,
- launch Burnly again,
- verify startup reconciliation re-creates the registration,
- reboot or log out/in,
- verify Burnly starts automatically.

Platform-specific checks:

```text
Linux:
  ~/.config/autostart/<app_name>.desktop
  Exec=<installed AppImage path>

Windows:
  HKCU\Software\Microsoft\Windows\CurrentVersion\Run\<app_name>
  value=<installed Burnly exe path>

macOS:
  ~/Library/LaunchAgents/<Burnly/plugin label>.plist
  Program/ProgramArguments point at installed Burnly app executable
```

## Implementation Chunks

This can be implemented in one focused execution plan.

Recommended checklist:

- Add pure startup reconciliation policy helper and tests.
- Add `DesktopSettingsRuntime::reconcile_launch_at_login_on_startup`.
- Call reconciliation during `setup_runtime` after settings are read and the
  runtime object exists.
- Make reconciliation failure non-fatal and logged.
- Add or update desktop runtime evidence checklist/docs.
- Run focused Rust tests.
- Run `pnpm verify:fast`.
- Collect Linux installed evidence locally.
- Queue Windows and macOS installed evidence for agents/users on those systems.

## Open Decisions

- Whether the Settings UI should display a warning when persisted setting is
  enabled but startup repair failed.
- Whether installer scripts should clean stale `~/.local/share/applications/Burnly.desktop`
  files that contain only `Hidden=true`.
- Whether Burnly should expose a diagnostics command for current native
  launch-at-login registration status.
