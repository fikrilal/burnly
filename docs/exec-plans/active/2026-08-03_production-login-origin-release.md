# 2026-08-03 Production Login Origin Release

## Objective

Ensure packaged desktop builds open the production web login during sign-in and
publish the fix as v0.1.23.

## Acceptance Criteria

- The built-in web-origin fallback is `https://burnly.dev`.
- The API fallback remains `https://api.burnly.dev`.
- The desktop loopback callback remains `http://127.0.0.1:39201/callback`.
- Release validation and GitHub artifact publication succeed.

## Risk Class

`medium`

## Impact Areas

- Desktop OAuth browser handoff
- Cloud API configuration
- Release packaging and updater metadata

## Checklist

- [x] Change the production web-origin fallback.
- [x] Update cloud configuration documentation and release notes.
- [x] Bump package and Rust versions to v0.1.23.
- [x] Run release verification.
- [ ] Publish v0.1.23.

## Verification

- `pnpm release:version v0.1.23` — passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — passed.
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::cloud::config::tests` — passed; 2 tests.
- `pnpm prettier --check package.json .github/release-notes/v0.1.23.md docs/engineering/desktop-cloud-core.md docs/exec-plans/active/2026-08-03_production-login-origin-release.md` — passed.
- `pnpm release-workflow:check` — passed.
- `pnpm packaging:check` — passed.
- `pnpm release-artifacts:test` — passed.
- `pnpm updater-metadata:test` — passed.
