# Packaged Sidecars

Burnly packages a pinned native `ccusage` executable as a private application
resource. React has no process or filesystem authority over it.

## Release Identity

- Version: `20.0.19`
- Source tag: `v20.0.19`
- Source revision: `caf89e8c0291a2acec09e01ff609e6253f6dd81b`
- Distribution: official `@ccusage/ccusage-*` npm platform packages

The release manifest at
`src-tauri/sidecars/ccusage/release-manifest.json` records the Rust target
triple, package name, executable name, and binary SHA-256 for:

- macOS arm64 and x86_64
- Linux arm64 and x86_64
- Windows arm64 and x86_64

Every release target must be present exactly once. Missing, duplicate, unknown,
or unverified targets fail the collector harness.

## Build Staging

`pnpm sidecar:prepare`:

1. Resolves the current Rust target triple, or the explicit
   `BURNLY_SIDECAR_TARGET` used by a cross-target build.
2. Locates the matching optional native dependency installed with exact
   `ccusage` version `20.0.19`.
3. Verifies package name, package version, and binary SHA-256.
4. Runs `ccusage --version` when the target is executable on the build host.
5. Stages only the selected executable and the release manifest into the
   ignored runtime resource directory.
6. Stages `ccusage.payload`, a Burnly-header-wrapped copy of the reviewed
   executable bytes. This payload exists because AppImage tooling rewrites the
   direct ELF executable and breaks Bun-packed `ccusage`.

Tauri's `beforeBuildCommand` runs this before the frontend build. The resource
map places the staged files at:

```text
$RESOURCE/sidecars/ccusage/manifest.json
$RESOURCE/sidecars/ccusage/ccusage[.exe]
$RESOURCE/sidecars/ccusage/ccusage[.exe].payload
```

On Linux AppImage builds, Tauri's runtime resource resolver and the AppImage
bundler can disagree on whether the application resource directory is named
from the lowercase executable/package identity or the configured product name.
Burnly first uses Tauri's resolved resource directory. If the packaged sidecar
manifest is absent there and the process is running from an AppImage, Burnly
falls back to `$APPDIR/usr/lib/Burnly` only when the sidecar manifest exists at
that exact product resource path.

The Rust adapter independently rechecks SHA-256 and runtime version before each
collector operation. It prefers the direct packaged executable when its bytes
match the release manifest. If package tooling mutates that executable, the
adapter verifies the wrapped payload bytes against the same manifest checksum,
materializes an executable temporary copy, and runs that copy. Release startup
therefore fails closed if resources are missing, modified, mismatched, or built
for an unsupported target.

## Development

Development may use `BURNLY_CCUSAGE_DEV_BINARY` with the unverified development
manifest. That state is explicit and cannot satisfy release integrity policy.

## Verification

- `pnpm sidecar:check` verifies the installed host package without staging.
- `pnpm collectors:fixtures` validates manifest completeness and metadata.
- `pnpm linux-smoke:appimage <path>` verifies AppImage sidecar payload
  materialization, checksum, and runtime version.
- Rust tests verify target mapping, checksum-before-version behavior, location
  policy, missing binaries, and incompatible versions.
- Packaged evidence must inspect the actual installer contents and execute the
  packaged binary on each supported platform.

## References

- [Tauri external binaries](https://v2.tauri.app/develop/sidecar/)
- [Tauri bundle resources](https://v2.tauri.app/reference/config/#resources)
- [ccusage repository](https://github.com/ccusage/ccusage)
