# Release Automation

## Workflow Boundaries

`verify.yml` runs the complete repository gate on pinned Ubuntu, macOS, and
Windows runners for pull requests, pushes to `development` or `main`, and manual
runs. It has read-only repository permissions and receives no release secrets.

`release.yml` runs for version tags and manual dry runs. Its validation job runs
the complete gate and requires a release tag to equal `v` plus the version in
`package.json` before publication is possible.

## Build Matrix

The release matrix builds all six native target triples from
`src-tauri/release-targets.json`:

- macOS ARM64 and x86_64 DMGs
- Windows ARM64 and x86_64 NSIS installers
- Linux ARM64 and x86_64 AppImages

Each job uses a native GitHub-hosted runner. Build jobs have no publication
permission. They receive updater signing secrets only for Tauri's updater
artifact signing, stage one canonical artifact, preserve signature files when
present, produce a target checksum manifest, run Linux AppImage smoke on Linux
jobs, upload an immutable workflow artifact for 14 days, and request GitHub
build-provenance attestation.

## Publication

Publication is a separate job with `contents: write`. It runs only for a pushed
version tag or an explicitly approved manual run, and only after validation and
every matrix build succeed. It downloads all six artifacts, verifies every size
and SHA-256 against its manifest, writes `SHA256SUMS`, generates and verifies
Linux `latest-linux.json` updater metadata from staged AppImage signatures,
rejects duplicate release tags, and creates a draft release.

Public release promotion remains outside this workflow. A failed, cancelled, or
missing matrix job cannot publish a partial release.

## Updater Signing

Linux updater signing uses Tauri's updater signing flow. Release jobs require:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

The private key must exist only as a release secret. The public key belongs in
source once the runtime updater plugin is configured in Phase 4. Updater
metadata publishes inline signatures from staged `.AppImage.sig` files; missing
or mismatched signatures fail metadata generation or verification.

## Pinning And Caching

GitHub Actions are referenced by full commit SHA with the reviewed release tag
in a comment. Node, pnpm, and Rust versions are exact. Node's package-manager
cache is keyed from `pnpm-lock.yaml`; Rust build output is not restored from an
untrusted cross-platform cache.
