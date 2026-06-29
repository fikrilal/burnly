# Burnly

Burnly is a local tray app for tracking AI coding-tool token usage.

It reads usage from supported local tools, stores the result on your machine,
and gives you a quick answer to: "how much have I burned today?"

## Features

- Tray-first usage summary for daily AI coding-tool activity.
- Local usage history stored in SQLite.
- Source and model breakdowns for supported tools.
- Estimated cost when the upstream usage data includes enough cost information.
- Launch-at-login and close-to-tray settings.
- Linux AppImage install with signed update metadata.

## Supported Sources

Burnly currently reads local usage through the bundled `ccusage` collector for:

- Claude Code
- Codex
- OpenCode

Burnly does not read prompts, responses, source code, or file contents.

## Linux Install

Linux is the supported MVP install path.

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/latest/download/install-linux.sh | sh
```

The installer downloads the matching AppImage for your CPU architecture,
verifies `SHA256SUMS`, installs Burnly under your user data directory, creates a
`burnly` command, and writes a desktop entry.

For a pinned release, pass a version tag:

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/download/vX.Y.Z/install-linux.sh | BURNLY_VERSION=vX.Y.Z sh
```

## Updates

Burnly release artifacts are signed for the Tauri updater. When an update is
available, use the Settings tab to check, install, and restart into the new
version.

## Privacy

Burnly is local-first:

- No account is required.
- Usage data is stored locally.
- Prompts, responses, source code, and file contents are not collected.
- Project paths are not stored as user-visible project history.

On Linux, app data lives under:

```text
${XDG_DATA_HOME:-~/.local/share}/app.burnly.desktop
```

## Uninstall

Remove the installed AppImage, launcher, and desktop entry:

```bash
rm -f ~/.local/share/burnly/Burnly.AppImage
rm -f ~/.local/bin/burnly
rm -f ~/.local/share/applications/burnly.desktop
```

This does not delete your usage database. To erase local Burnly data too:

```bash
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/app.burnly.desktop"
```

## Troubleshooting

### The tray icon does not appear

Burnly depends on your Linux desktop tray/status area support. GNOME users may
need an AppIndicator or tray extension enabled.

### Usage does not refresh

Open the tray panel and use the refresh action. If the status still fails, check
that at least one supported tool has local usage data available.

### The `burnly` command is not found

Add your local bin directory to `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

## Development

Developer documentation starts at [docs/README.md](./docs/README.md).

Common local commands:

```bash
pnpm install
pnpm tauri dev
pnpm verify
```
