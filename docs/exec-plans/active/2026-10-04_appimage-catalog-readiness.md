# 2026-10-04 AppImage Catalog Readiness

## Objective

Make the released Linux AppImage pass the AppImageHub (appimage.github.io)
catalog test so that pull request AppImage/appimage.github.io#7254 can merge.

Two blocking defects were confirmed against the real `v0.1.32` release asset:

1. `.DirIcon` is an absolute symlink into the build directory
   (`/home/runner/work/burnly/burnly/...`), so it dangles wherever the AppImage
   is mounted. The catalog test reports `FATAL: .DirIcon is missing`.
2. The AppImage requires `GLIBC_2.38`/`GLIBC_2.39` (the executable itself, plus
   48 version errors across bundled GTK/WebKit libraries). The catalog test runs
   on `ubuntu-22.04` (glibc 2.35), so the application cannot start there at all.

## Acceptance Criteria

- `@tauri-apps/cli` is at `>=2.11.4`, and a locally built AppImage has
  `.DirIcon` and `.desktop` root symlinks that are relative and resolve inside
  the AppDir.
- `pnpm linux-smoke:appimage` fails when any symlink inside the extracted AppDir
  resolves outside it.
- `pnpm linux-smoke:appimage` fails when any ELF file in the AppImage requires a
  `GLIBC_*` symbol version above `2.35`, naming the offending file and version.
- Both Linux AppImage matrix entries build on the 22.04 baseline
  (`ubuntu-22.04`, `ubuntu-22.04-arm`), and the recorded runner in the platform
  behavior matrix matches.
- `pnpm release-workflow:check` fails if a Linux AppImage matrix entry stops
  using the 22.04 baseline, and `pnpm release-workflow:test` still catches drift.
- `pnpm verify:fast` and `pnpm verify` pass.
- Out of scope, deliberately deferred: the AppImage filename warning (the
  artifact name contains `linux`), the missing `LICENSE` file, and AppStream
  metainfo. Re-testing PR #7254 requires a new tagged release, which is an
  operator step, not part of this plan.

## Risk Class

`medium`.

The build environment for both Linux artifacts changes, so the shipped AppImage
bundles a different WebKitGTK/GTK (jammy 2.50.4 instead of noble's) for every
Linux user. An older baseline is strictly more compatible, but the bundled
WebKit version is a behavior change that only a release smoke run can confirm.

## Impact Areas

- `package.json`, `pnpm-lock.yaml`
- `.github/workflows/release.yml`
- `docs/engineering/platform-behavior-matrix.json`
- `scripts/smoke-linux-appimage.mjs`
- `scripts/harness/check-release-workflows.mjs`

## Design Review

- What complexity is being introduced? Two artifact checks in the existing
  Linux AppImage smoke test. No new abstraction, no new script, no new npm
  script entry.
- Which decisions are hidden inside the owning module? The glibc ceiling is a
  property of the catalog we submit to, so the constant and its justification
  live beside the check that enforces it, not in the workflow.
- Is each new interface simpler than its implementation? Yes. The smoke script
  already extracts the AppDir; the checks consume that directory and return
  plain failures.
- What special cases exist, and can the design eliminate them? Absolute
  symlinks are the general defect, not `.DirIcon` specifically, so the check
  asserts containment for every symlink rather than special-casing one path.
  This also covers the `.desktop` link that the same upstream fix corrected.
- Why is each new abstraction needed now? No new abstraction is added. The
  runner baseline is enforced in the existing release workflow harness, which
  already owns workflow policy.
- Can an existing module absorb this responsibility cleanly? Yes. The smoke
  script owns artifact verification and the release workflow harness owns
  workflow policy; both changes stay inside them.

## Checklist

- [x] Write the execution plan.
- [x] Bump `@tauri-apps/cli` to `2.12.1` and update the lockfile.
- [x] Move both Linux AppImage matrix entries to the 22.04 baseline.
- [x] Record the new runners in the platform behavior matrix.
- [x] Add symlink containment and glibc ceiling checks to the AppImage smoke test.
- [x] Pin the 22.04 baseline in the release workflow harness, with self-test coverage.
- [x] Prove the negative path: both new checks fail on the real `v0.1.32` artifact.
- [x] Prove the pass path: the checks accept a conforming artifact, including at
      the ceiling boundary.
- [x] Rebuild the AppImage locally and confirm the `.DirIcon` fix.
- [x] Run focused checks, `pnpm verify:fast`, and `pnpm verify`, and record outcomes.

## Test Plan

- Behavior and invariants to prove:
  - a symlink whose target leaves the AppDir fails the smoke test
  - a relative, resolving symlink passes
  - an ELF requiring more than glibc 2.35 fails and names the file and version
  - an ELF requiring 2.35 or less passes
  - the workflow harness rejects a Linux matrix entry on a non-22.04 runner
- Lowest stable test layer: the artifact checks run against the extracted
  AppDir, so they are proven against a real artifact rather than a unit stub.
  The workflow rule is proven by the existing `--self-test` mutation pattern.
- Failure paths: dangling symlink, absolute symlink, missing `objdump`,
  non-ELF files, files with no dynamic symbols (the bundled ccusage sidecar is
  a static-pie binary with zero `DT_NEEDED` entries and must not fail the scan).
- Fixtures or fakes: the real `v0.1.32` release asset for the negative path and
  the locally rebuilt artifact for the pass path. The self-test covers the
  decision logic (symlink containment, ELF detection, version comparison) with
  synthetic fixtures, so it stays host-independent. No mocks.
- Runtime or platform evidence: a local `pnpm tauri build --bundles appimage`
  to confirm the CLI bump produces a relative `.DirIcon`.
- Relevant commands:
  - `pnpm install`
  - `pnpm linux-smoke:appimage <artifact>` (negative path against `v0.1.32`)
  - `pnpm release-workflow:check && pnpm release-workflow:test`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Bump the Tauri **CLI**, not the `tauri` crate: the AppDir layout (and
  therefore `.DirIcon`) is produced by the bundler embedded in
  `@tauri-apps/cli`, so `2.12.1` is the minimal fix. Upstream
  tauri-apps/tauri#15596 ("make .desktop and .DirIcon relative symlinks") landed
  before 2.11.4.
- Build on `ubuntu-22.04` rather than bundling glibc with `ld-linux`, because
  the catalog's "self-contained" requirement is advisory while running on the
  oldest supported LTS is mandatory, and shipping our own loader risks breaking
  hosts in ways the current bundle does not.
- Enforce the ceiling as a hard failure rather than a warning. A local build on
  a newer distribution (this machine is Ubuntu 26.04, glibc 2.43) will fail the
  check by design, because such an artifact genuinely cannot be published. The
  message names the catalog requirement so the failure is self-explanatory.
- Keep the artifact name unchanged for now. The `linux` in
  `burnly-v<version>-linux-<arch>.AppImage` is a catalog _warning_, not an
  error, and renaming reaches into `release-targets.json`, `install-linux.sh`,
  the updater manifest, and four harness checks. Not worth coupling to this fix.
- Do not bundle glibc, do not add a boolean flag to skip the checks: the
  release pipeline is the only caller that matters and it builds on the
  baseline.
- Keep the smoke self-test free of binutils and host binaries. It uses a
  synthetic ELF header instead of `process.execPath`, because `harness:check`
  also runs on Windows through `verify:windows`, where the Node.js executable is
  a PE file. The `objdump` integration is exercised only by the release smoke
  run against a real AppImage, so the fast gate does not start requiring
  binutils. Verified by running the self-test with a failing `objdump` first on
  `PATH`.

## Verification

- Command: `pnpm install`
  - Outcome: `@tauri-apps/cli` moved `2.11.3` -> `2.12.1`; lockfile updated.
- Command: `pnpm linux-smoke:appimage:test`
  - Outcome: passed. Covers relative/absolute/escaping/dangling symlinks, ELF
    detection, and glibc version comparison including the `2.35` boundary.
- Command: `pnpm release-workflow:check`
  - Outcome: passed.
- Command: `pnpm release-workflow:test`
  - Outcome: passed; the self-test mutates a Linux matrix entry to
    `ubuntu-24.04` and requires the baseline failure to be reported.
- Command: `pnpm linux-smoke:appimage /tmp/pr-real/burnly.AppImage` (the real
  released `v0.1.32` asset)
  - Outcome: failed as intended —
    `AppImage symlink .DirIcon points outside the AppDir: /home/runner/work/...`.
- Command: same artifact with the symlink assertion neutralised in a scratch
  copy, to reach the later check
  - Outcome: failed as intended — `AppImage requires GLIBC_2.39 ...`, naming
    `usr/bin/burnly` first, then `libprintbackend-cups.so`,
    `libXcursor.so.1`, `libatk-bridge-2.0.so.0` and others.
- Command: `pnpm tauri build --bundles appimage`
  - Outcome: passed with CLI `2.12.1`. `.DirIcon` is now
    `-> Burnly.png` (relative) instead of an absolute
    `/home/runner/work/burnly/...` target.
- Command: `pnpm linux-smoke:appimage` on that local build
  - Outcome: symlink containment passed over 25 symlinks, then the glibc check
    failed at `GLIBC_2.43`. This is the documented local-build behavior: this
    machine is Ubuntu 26.04, so its artifact genuinely cannot run on the
    catalog baseline.
- Command: same artifact with the ceiling set to the artifact's own highest
  requirement (`2.43`) in a scratch copy
  - Outcome: passed end to end and reported
    `glibcRequired: 2.43`, `symlinksChecked: 25`, proving the pass path and the
    boundary comparison (`equal` is allowed) on real ELF files.
- Command: `bash appdir-lint.sh <AppDir>` (AppImage/AppImages lint from the
  catalog checklist)
  - Outcome: `Lint found no fatal issues`; only the advisory warning that no
    AppStream appdata file is present.
- Command: `desktop-file-validate <AppDir>/usr/share/applications/Burnly.desktop`
  - Outcome: passed with no output.
- Command: `pnpm verify:fast` and `pnpm verify`
  - Outcome: first run failed on two `preserve-caught-error` lint errors in the
    new smoke-test code; fixed by attaching `cause`. Rerun passed both:
    `verify:fast` exit 0, `verify` exit 0, with 119 frontend tests across 20
    files and 792 Rust tests (0 failed, 3 ignored), `cargo fmt` clean and
    `clippy -D warnings` clean. `harness:check` reported
    `Linux AppImage smoke self-test passed.`
- Command: `PATH=/tmp/fakebin:$PATH node scripts/smoke-linux-appimage.mjs --self-test`
  with a failing `objdump` stub first on `PATH`
  - Outcome: passed, confirming the fast gate does not depend on binutils.

## Runtime Evidence

- The negative path is the real `v0.1.32` release asset downloaded from GitHub
  Releases, matching what the catalog test fetches (90,585,592 bytes, and its
  `.DirIcon` target is the CI build directory).
- The pass path is the locally rebuilt artifact, checked over its 25 real
  symlinks and 171 ELF files.
- `.DirIcon` before the CLI bump:
  `-> /home/runner/work/burnly/burnly/src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/appimage/Burnly.AppDir/Burnly.png`.
  After: `-> Burnly.png`.
- The 22.04 build itself is not reproducible on this machine (Ubuntu 26.04), so
  the claim that a 22.04-built AppImage stays at or under `GLIBC_2.35` rests on
  the build environment rather than on a measured artifact. The first release
  run on the new runners is what proves it, and the smoke test now blocks the
  release if it does not hold.
- The bundled ccusage sidecar is a static-pie executable with zero `DT_NEEDED`
  entries, so it imposes no glibc floor of its own.
- Visual confirmation of the catalog test itself requires a tagged release.

## Follow-Up Debt

- `ubuntu-22.04` runners will be retired when Ubuntu 22.04 reaches end of
  standard support (April 2027). The catalog baseline and the build baseline
  move together, so both need revisiting then.
- The AppImage filename still contains `linux` and the repository still has no
  `LICENSE`; both are catalog cosmetics with real downstream coupling.
- No AppStream metainfo is shipped, so the catalog page uses the automated
  screenshot instead of ours.
