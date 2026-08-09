# Burnly

Burnly is a local tray app for tracking AI coding-tool token usage.

It reads usage from supported local tools, stores the result on your machine,
and gives you a quick answer to: "how much have I burned today?"

## Features

- Tray-first usage summary for daily AI coding-tool activity.
- Local usage history stored in SQLite.
- Source and model breakdowns for supported tools.
- Estimated cost: provider-reported where available, ccusage-calculated for
  ccusage sources, and Burnly-calculated from an embedded models.dev pricing
  snapshot for sources that report no cost (Grok, Antigravity, ZCode).
- Launch-at-login and close-to-tray settings.
- Linux AppImage, Windows x64, and macOS preview installs ship signed update
  metadata; macOS first install is an unsigned `.dmg` preview (Apple Silicon
  and Intel).

## Supported Sources

Burnly reads local usage from supported AI coding tools. Support levels are
explicit because each tool stores usage differently.

| Tool         | Status            | Collection path                                  | Notes                                                                                                                                                                                                                                                                                        |
| ------------ | ----------------- | ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code  | Supported         | Bundled `ccusage` collector                      | Local usage only.                                                                                                                                                                                                                                                                            |
| Codex        | Supported         | Bundled `ccusage` collector                      | Local usage only.                                                                                                                                                                                                                                                                            |
| OpenCode     | Supported         | Bundled `ccusage` collector                      | Local usage only.                                                                                                                                                                                                                                                                            |
| Pi           | Supported         | Bundled `ccusage` collector                      | Local usage only. Model labels keep the `[pi]` prefix from `ccusage`.                                                                                                                                                                                                                        |
| Cline CLI    | Experimental      | Native Burnly collector for `~/.cline`           | Reads local session/message usage metrics. Data format may change upstream.                                                                                                                                                                                                                  |
| ZCode        | Experimental      | Native Burnly collector                          | Reads local SQLite usage data. Data format may change upstream.                                                                                                                                                                                                                              |
| Antigravity  | Experimental      | Native Burnly collector                          | Three variants: 2.0, IDE, and CLI. CLI reads local SQLite/protobuf metadata. App/IDE use runtime metadata when available, experimental SQLite fallback, then cached usage.                                                                                                                   |
| Grok Build   | Experimental      | Native Burnly collector for `~/.grok`            | Reads `shell.turn.inference_done` rows from `unified.jsonl` plus session metadata. Totals are per inference call, not per user turn. Cached prompt tokens count toward tray totals. Cost is Burnly-calculated from an embedded models.dev pricing snapshot. Data format may change upstream. |
| Command Code | Experimental      | Native Burnly collector for `~/.commandcode`     | Reads per-message `usage` blocks from `projects/**/<session>.jsonl` transcripts. Cost is provider-computed `costUsd`. Legacy pre-1.11 transcripts carry no usage and are skipped. Data format may change upstream.                                                                           |
| Zed          | Planned           | Native Burnly collector for `~/.local/share/zed` | Reads agent thread token usage from `threads.db` (zstd thread JSON). Per-request history in the telemetry log. Experimental collector in development.                                                                                                                                        |
| Cursor       | Not supported yet | Roadmap                                          | Needs local usage-data investigation.                                                                                                                                                                                                                                                        |
| Windsurf     | Not supported yet | Roadmap                                          | Needs local usage-data investigation.                                                                                                                                                                                                                                                        |
| Aider        | Not supported yet | Roadmap                                          | Needs local usage-data investigation.                                                                                                                                                                                                                                                        |
| Roo Code     | Not supported yet | Roadmap                                          | Needs local usage-data investigation.                                                                                                                                                                                                                                                        |
| Continue     | Not supported yet | Roadmap                                          | Needs local usage-data investigation.                                                                                                                                                                                                                                                        |
| Gemini CLI   | Not planned       | Deprecated upstream                              | Reconsider only if a maintained successor exposes reliable local usage.                                                                                                                                                                                                                      |

Burnly does not read prompts, responses, source code, or file contents.

## Install

### Linux and macOS Quick Install

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/latest/download/install.sh | sh
```

The universal installer detects Linux or macOS, downloads the matching
platform installer, verifies release artifacts through that installer, and
installs Burnly for your CPU architecture.

For a pinned release, pass a version tag:

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/download/vX.Y.Z/install.sh | BURNLY_VERSION=vX.Y.Z sh
```

On Linux, Burnly is installed as an AppImage under your user data directory
with a `burnly` command and desktop entry. On macOS, Burnly is copied to
`/Applications/Burnly.app` and the quarantine attribute is cleared for the
unsigned preview build.

Direct platform installers are also available:

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/latest/download/install-linux.sh | sh
curl -fsSL https://github.com/fikrilal/burnly/releases/latest/download/install-macos.sh | sh
```

### Windows Preview

Run PowerShell:

```powershell
irm https://github.com/fikrilal/burnly/releases/latest/download/install.ps1 | iex
```

For a pinned release:

```powershell
$env:BURNLY_VERSION = "vX.Y.Z"; irm https://github.com/fikrilal/burnly/releases/download/vX.Y.Z/install.ps1 | iex
```

The PowerShell installer downloads the Windows x64 installer, verifies
`SHA256SUMS`, and starts the normal installer UI.

Manual install is also available by downloading the Windows x64 installer from
the official GitHub release:

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

The installer downloads the matching `.dmg` for your CPU architecture, verifies
`SHA256SUMS`, copies `Burnly.app` to `/Applications`, and clears the quarantine
attribute required for unsigned preview builds. If `/Applications` needs
administrator permission, the installer asks through `sudo`.

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

For Antigravity, refresh behavior depends on the variant:

- **CLI** (`agy`): Burnly reads usage from local conversation databases under
  `~/.gemini/antigravity-cli/conversations/` (or `GEMINI_CLI_HOME` when set).
  A running `agy` process is not required after the session is written to disk.
- **2.0 and IDE**: Burnly prefers live runtime metadata while the app is
  running. When runtime metadata is unavailable, Burnly may use an experimental
  SQLite/protobuf fallback or previously cached usage records instead of clearing
  stored totals.

Burnly never reads prompts, responses, source files, or network traffic from
Antigravity.

For Grok Build, refresh reads local inference telemetry from
`~/.grok/logs/unified.jsonl` (or `GROK_HOME` when set) and joins session
metadata from `~/.grok/sessions/**/summary.json`. A running `grok` process is
not required after inference rows are written to the unified log. If the log is
temporarily unreadable or truncated, Burnly may recover usage from a durable
normalized cache instead of clearing stored totals.

Burnly never reads Grok chat transcripts (`chat_history.jsonl`,
`updates.jsonl`), prompt history, terminal logs, `auth.json`, or other
conversation-bearing files.

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
