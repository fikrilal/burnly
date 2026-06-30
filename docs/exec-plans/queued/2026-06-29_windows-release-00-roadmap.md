# 2026-06-29 Windows Release Roadmap

## Objective

Expand Burnly from Linux-only distribution to Windows `.exe` distribution with
Tauri updater support, without weakening the already-working Linux release path.

## Scope Split

This work should be split into four execution plans:

1. `windows-release-01-exe-artifacts.md`
   - Add Windows NSIS `.exe` release artifacts and CI smoke checks.
2. `windows-release-02-updater-metadata.md`
   - Extend signed updater metadata and validation for Windows.
3. `windows-release-03-runtime-evidence.md`
   - Verify real Windows runtime behavior on a Windows machine or VM.
4. `windows-release-04-public-release-hardening.md`
   - Make Windows release public-ready, including docs and code-signing
     decisions.

## Why Split

- Windows release touches build automation, Tauri bundling, updater metadata,
  runtime behavior, and user-facing distribution.
- Each phase has a different verification surface.
- Keeping Linux release behavior stable is easier when each phase has a narrow
  diff and a clear rollback point.

## Non-Goals

- No macOS release work.
- No Microsoft Store packaging.
- No silent installer service.
- No Windows code-signing purchase/setup unless explicitly approved in phase 4.

## Decisions

- Use Windows `.exe` installer artifacts, not MSI, for the first Windows
  distribution path.
- Windows auto-update must be supported before calling Windows distribution
  public-ready.
- Linux release/update behavior must remain supported through every phase.
