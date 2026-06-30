# Windows Release Handoff

## Current Repository State

Branch: `development`

Latest local commits:

- `f101b51 test(release): add Windows runtime evidence contract`
- `7884471 feat(release): add Windows updater metadata`
- `e3134be feat(release): add Windows exe artifact build`

At handoff time, `development` is ahead of `origin/development` by 3 commits.
Push these commits or otherwise make them available before starting Windows
validation.

## What Is Already Done

Windows x64 release artifact support is implemented:

- Release workflow builds `x86_64-pc-windows-msvc` on `windows-2022`.
- Artifact name is canonical: `burnly-v<version>-windows-x86_64.exe`.
- `pnpm windows-smoke:exe` validates the staged installer shape.

Windows updater metadata support is implemented:

- Release workflow signs the Windows NSIS `.exe`.
- Release publish generates cross-platform `latest.json`.
- `latest-linux.json` remains a compatibility alias for older Linux builds.
- New app builds use `latest.json` as the updater endpoint.
- Updater metadata includes the Windows platform key `windows-x86_64`.

Windows runtime evidence tooling is implemented:

- Active plan:
  `docs/exec-plans/active/2026-06-29_windows-release-03-runtime-evidence.md`
- Evidence guide:
  `docs/engineering/windows-runtime-evidence.md`
- Evidence template:
  `docs/engineering/evidence/windows-runtime-evidence.template.json`
- Validator:
  `pnpm windows-runtime:evidence:check <evidence.json>`

Linux-side verification already passed:

- `pnpm updater-metadata:test`
- `pnpm release-artifacts:test`
- `pnpm release-workflow:test && pnpm release-workflow:check`
- `pnpm platform-behavior:test && pnpm platform-behavior:check`
- `pnpm verify`

## Current Blocker

Exec 3 is not complete. It requires real Windows x64 installed runtime
evidence. Do not move the active plan to completed until the Windows evidence
JSON passes validation.

The missing evidence must prove:

- Windows `.exe` installs Burnly.
- Burnly launches from Start menu or installed shortcut.
- Tray panel can reach the desktop runtime.
- Packaged `ccusage` sidecar executes and refresh succeeds.
- SQLite database exists at the expected Windows app data path.
- Launch-at-login can be enabled and points at the installed app.
- Manual update check detects a newer version.
- Update install and restart lands on the detected newer version.

## Windows Evidence Procedure

Use a real Windows x64 machine or VM. Linux cannot close this phase.

1. Make sure the Windows checkout includes the three commits listed above.

2. Install dependencies:

   ```powershell
   pnpm install --frozen-lockfile
   ```

3. Run the Windows local gate:

   ```powershell
   pnpm verify:windows
   ```

4. Obtain an older and newer signed Windows release pair.

   The evidence requires an installed older version and a newer version
   discoverable through the GitHub updater metadata. The installer artifact must
   be named:

   ```text
   burnly-v<version>-windows-x86_64.exe
   ```

5. Copy the template:

   ```powershell
   Copy-Item `
     docs/engineering/evidence/windows-runtime-evidence.template.json `
     docs/engineering/evidence/windows-runtime-evidence.<version>.json
   ```

6. Fill the copied JSON with real observed values.

   Required fields include:

   - `environment.windowsVersion`
   - `artifact.version`
   - `artifact.installerFileName`
   - `artifact.source`
   - `install.installPath`
   - `install.appDataPath`
   - `install.databasePath`
   - every `checks.*.status` as `"passed"`
   - every `checks.*.notes` with concrete notes
   - `checks.refresh.latestImportStatus` as `"success"`
   - `checks.ccusageSidecar.observedVersion`
   - `checks.manualUpdateCheck.fromVersion`
   - `checks.manualUpdateCheck.detectedVersion`
   - `checks.updateInstallRestart.finalVersion`

7. Validate the evidence:

   ```powershell
   pnpm windows-runtime:evidence:check docs/engineering/evidence/windows-runtime-evidence.<version>.json
   ```

8. If a Windows bug is found, fix it on Windows, run targeted tests, then run:

   ```powershell
   pnpm verify:windows
   ```

9. Update the active exec plan:

   - Check off completed Windows runtime checklist items.
   - Add exact command outcomes under `Verification`.
   - Add the evidence file path and key observations under `Runtime Evidence`.

10. Only after evidence passes, move the plan to completed:

    ```powershell
    git mv `
      docs/exec-plans/active/2026-06-29_windows-release-03-runtime-evidence.md `
      docs/exec-plans/completed/2026-06-29_windows-release-03-runtime-evidence.md
    ```

## Final Exec Dependency

Do not start final exec 4 until exec 3 is complete.

Queued final plan:

`docs/exec-plans/queued/2026-06-29_windows-release-04-public-release-hardening.md`

Exec 4 depends on phase 3 evidence and covers:

- README Windows install documentation.
- Release notes Windows asset documentation.
- Public release asset validation.
- Windows code-signing posture.
- Final public release hardening.

The likely MVP decision is to ship Windows unsigned only if the docs clearly
warn about SmartScreen/trust prompts. If signing is required, configure and
verify Windows signing before public release.

## Important Constraints

- Keep only one active execution plan.
- Do not mark exec 3 complete without real Windows evidence.
- Do not claim Windows public-ready before exec 4 is done.
- Windows ARM64 remains deferred.
- Frontend must not call Tauri updater APIs directly; updater access stays
  behind Burnly IPC.
- Do not commit or push unless the user explicitly asks.
