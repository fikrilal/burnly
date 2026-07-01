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
- Linux AppImage, Windows x64, and macOS preview installs ship signed update
  metadata; macOS first install is an unsigned `.dmg` preview (Apple Silicon
  and Intel).

## Supported Sources

Burnly reads local usage from supported AI coding tools. Support levels are
explicit because each tool stores usage differently.

| Tool        | Status            | Collection path                        | Notes                                                                       |
| ----------- | ----------------- | -------------------------------------- | --------------------------------------------------------------------------- |
| Claude Code | Supported         | Bundled `ccusage` collector            | Local usage only.                                                           |
| Codex       | Supported         | Bundled `ccusage` collector            | Local usage only.                                                           |
| OpenCode    | Supported         | Bundled `ccusage` collector            | Local usage only.                                                           |
| Pi          | Supported         | Bundled `ccusage` collector            | Local usage only. Model labels keep the `[pi]` prefix from `ccusage`.       |
| Cline CLI   | Experimental      | Native Burnly collector for `~/.cline` | Reads local session/message usage metrics. Data format may change upstream. |
| Antigravity | Not supported yet | Roadmap research                       | Local inspection did not find reliable token usage data.                    |
| Cursor      | Not supported yet | Roadmap                                | Needs local usage-data investigation.                                       |
| Windsurf    | Not supported yet | Roadmap                                | Needs local usage-data investigation.                                       |
| Aider       | Not supported yet | Roadmap                                | Needs local usage-data investigation.                                       |
| Roo Code    | Not supported yet | Roadmap                                | Needs local usage-data investigation.                                       |
| Continue    | Not supported yet | Roadmap                                | Needs local usage-data investigation.                                       |
| Gemini CLI  | Not planned       | Deprecated upstream                    | Reconsider only if a maintained successor exposes reliable local usage.     |

Burnly does not read prompts, responses, source code, or file contents.

## Install

### Linux

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

### Windows Preview

Download the Windows x64 installer from the official GitHub release:

```text
burnly-vX.Y.Z-windows-x86_64.exe
```

The Windows preview installer is unsigned for the MVP. Windows may show an
unknown publisher or SmartScreen warning. Only download Burnly from the
official GitHub releases page:

```text
https://github.com/fikrilal/burnly/releases
```

### macOS Preview

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/latest/download/install-macos.sh | sh
```

The installer downloads the matching `.dmg` for your CPU architecture, verifies
`SHA256SUMS`, copies `Burnly.app` to `/Applications`, and clears the quarantine
attribute required for unsigned preview builds. If `/Applications` needs
administrator permission, the installer asks through `sudo`.

For a pinned release, pass a version tag:

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/download/vX.Y.Z/install-macos.sh | BURNLY_VERSION=vX.Y.Z sh
```

Manual install is also available by downloading the `.dmg` for your CPU
architecture from the official GitHub release:

```text
burnly-vX.Y.Z-macos-aarch64.dmg   # Apple Silicon
burnly-vX.Y.Z-macos-x86_64.dmg    # Intel
```

Open the `.dmg` and drag `Burnly.app` to your Applications folder. Burnly lives
in the menu bar; it has no Dock icon.

The macOS preview is unsigned and not notarized for the MVP. Because the build
is not signed with an Apple Developer ID, macOS Gatekeeper quarantines it and
may report that the app "is damaged" or "cannot be opened". Clear the quarantine
attribute once after copying the app to Applications:

```bash
xattr -dr com.apple.quarantine /Applications/Burnly.app
```

Only download Burnly from the official GitHub releases page:

```text
https://github.com/fikrilal/burnly/releases
```

## Updates

Burnly release artifacts are signed for the Tauri updater. When an update is
available, use the Settings tab to check, install, and restart into the new
version.

In-app updates are available on Linux, Windows, and macOS. On macOS, the `.dmg`
is only the first-install artifact; Burnly updates itself from a signed
`.app.tar.gz` updater archive.

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

On Windows, app data lives under:

```text
%APPDATA%\app.burnly.desktop
```

On macOS, app data lives under:

```text
~/Library/Application Support/app.burnly.desktop
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

On Windows, uninstall Burnly from Windows Settings or run:

```text
%LOCALAPPDATA%\Burnly\uninstall.exe
```

To erase local Burnly data too, remove:

```text
%APPDATA%\app.burnly.desktop
```

On macOS, remove the app and, optionally, its data:

```bash
rm -rf /Applications/Burnly.app
rm -rf ~/Library/Application\ Support/app.burnly.desktop
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
