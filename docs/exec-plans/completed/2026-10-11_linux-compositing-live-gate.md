# 2026-10-11 Linux compositing live gate

## Objective

Set `WEBKIT_DISABLE_COMPOSITING_MODE=1` on the Linux display-backend path only, then keep or revert it from a packaged tray run. The same AppImage must meet the unchanged-refresh read gate.

## Acceptance Criteria

- The variable is set before GTK when the display-backend reason is a Linux selection. It is not set for `NotLinux`.
- macOS and Windows do not take the Linux branch.
- A second unchanged catch-up refresh adds less than 100 MB to `burnly` `read_bytes`.
- A one-artifact change adds less than that artifact's size plus 100 MB.
- About 60 seconds later, cgroup `memory.current` is not held up by multi-gigabyte `inactive_file` from Burnly re-reads.
- Keep the variable only when the tray still rounds, dismisses, and shows today's numbers, and either `libLLVM` and `libnvidia-eglcore` unmap from `burnly` and `WebKitWebProcess` or the four-process idle RSS sum drops at least 100 MB from 493 MB. Otherwise revert the variable and record the revert.

## Risk Class

`high`.

## Impact Areas

- Linux display backend
- Packaged AppImage
- Tray panel

## Design Review

- The variable is applied in the existing backend selection function, which already runs before GTK.
- `NotLinux` is the only non-Linux decision, so the same gate covers macOS and Windows.
- The transparent window and CSS stay. A failed trial reverts only the variable.

## Checklist

- [x] Set the variable on Linux backend selections.
- [x] Leave it unset for `NotLinux`.
- [x] Build and install the trial AppImage without deleting the previous bundle or user databases.
- [x] Measure the trial. Second unchanged refresh read 79,732,736 bytes. Idle RSS did not fall, and the GPU libraries stayed mapped.
- [x] Revert the variable and remeasure that AppImage. Second unchanged refresh read 12,464,128 bytes. One 49,152-byte conversation file added 49,152 bytes.

## Test Plan

- Behavior and invariants to prove: no backend path sets `WEBKIT_DISABLE_COMPOSITING_MODE`.
- Lowest stable test layer: `apply_linux_compositing_mode` in `display_backend` tests.
- Failure paths: a failed assert restores the previous environment value.
- Fixtures or fakes: none. The keep-or-revert decision uses the packaged app on this desktop.
- Runtime or platform evidence: `{SCRATCH}/live-runtime.log` and `{SCRATCH}/tray.png` when a screenshot works.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml --lib compositing_mode_stays_unset_on_every_backend_path`, then `pnpm verify` and `pnpm tauri build --bundles appimage`.

## Decisions

- The trial started enabled. The packaged log reverted it. `libLLVM` and `libnvidia-eglcore` stayed mapped in `burnly` and `WebKitWebProcess`. About 60 seconds after refresh, the process RSS sum was 531,916 KB, above the 493 MB baseline, so the 100 MB drop was missed. The tray still rounded and showed today's 1,606,298,589 tokens. Click-away from an XWayland window left the panel mapped. The variable is now unset on every backend.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib compositing_mode_stays_unset_on_every_backend_path -- --test-threads=8`
- Outcome: pass, 2026-10-11. 1 passed. The earlier trial test passed before the revert.
- Command: `pnpm verify`
- Outcome: pass, 2026-10-12, after the revert. Rust lib 819 passed, 0 failed, 3 ignored. ESLint 5 warnings, 0 errors. Exit 0.
- Command: packaged live measurement of the trial AppImage `b4d732cbca33599bbf5d3305d62cf5688a282725c95bdf88d9e1d767a3c06d00`
- Outcome: second unchanged catch-up added 79,732,736 read bytes. One ignored `.db-shm` touch added 16,384 and is not the artifact proof. After 60 seconds, cgroup `memory.current` was 672,518,144 and `inactive_file` was 256,491,520. RSS sum was 531,916 KB. `libLLVM` count 4 and `libnvidia-eglcore` count 5 stayed mapped. Tray text matched the token total and the corners were transparent. The variable was reverted.
- Command: packaged live measurement of the reverted AppImage `06dc6b9985b1e1063e64133f911ed35136d4748cd299df20ee336de722d7ee30`
- Outcome: pass, 2026-10-12. Second unchanged catch-up added 12,464,128 read bytes. One 49,152-byte conversation file added 49,152 read bytes and its checkpoint mtime matched. After 60 seconds, `memory.current` was 554,057,728 and `inactive_file` was 211,939,328. RSS sum was 534,960 KB. The panel showed 1,272,770 tokens, matching 2026-10-12 Asia/Jakarta, with transparent corners. `opencode.db` stayed 2,453,688,320 bytes.

## Runtime Evidence

- Trial: second unchanged refresh 79,732,736 bytes, RSS sum 531,916 KB, libraries still mapped, `inactive_file` 256,491,520. Variable reverted.
- Reverted bundle: second unchanged refresh 12,464,128 bytes, one-file refresh 49,152 bytes, `inactive_file` 211,939,328, RSS sum 534,960 KB.

## Follow-Up Debt

- None.
