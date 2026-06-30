# Windows Runtime Evidence

Windows release support is not public-ready until a real Windows x64 installed
runtime run produces passing evidence.

Use `docs/engineering/evidence/windows-runtime-evidence.template.json` as the
starting point, save the filled file outside the template path, then validate it:

```powershell
pnpm windows-runtime:evidence:check docs/engineering/evidence/windows-runtime-evidence.<version>.json
```

Do not mark the Windows runtime evidence execution plan complete until this
command passes against a real evidence file.

## Required Environment

- Windows x64.
- Installer artifact named `burnly-v<version>-windows-x86_64.exe`.
- A release or draft release that contains signed updater metadata.
- Two different versions for update testing: an installed older version and a
  newer available version.

## Evidence Steps

Record the Windows version:

```powershell
[System.Environment]::OSVersion.VersionString
```

Install the `.exe`, launch Burnly from the Start menu or installed shortcut, and
record:

- Installed app path.
- App data path.
- SQLite database path.
- App version before update.

Open the tray panel and verify:

- The panel reaches the desktop runtime.
- Refresh can be triggered and reaches a successful terminal state.
- Packaged `ccusage` executes and reports the pinned version.
- SQLite exists at the expected app data path and has a healthy schema.
- Launch-at-login can be enabled, survives reboot/login, and points at the
  installed app.

Run update evidence from the older installed version:

- Trigger a manual update check.
- Record the newer detected version.
- Install and restart into the update.
- Record the final app version after restart.

## Evidence Contract

Every check in the evidence JSON must have `"status": "passed"` and concrete
notes. The validator rejects pending checks, missing paths, missing versions,
failed refresh status, and update evidence where the final version does not
match the detected newer version.
