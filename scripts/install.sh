#!/bin/sh
set -eu

REPO="${BURNLY_REPO:-fikrilal/burnly}"
VERSION="${BURNLY_VERSION:-latest}"
OS_NAME="${BURNLY_UNAME_S:-$(uname -s)}"

case "$OS_NAME" in
  Linux)
    INSTALLER_NAME="install-linux.sh"
    ;;
  Darwin)
    INSTALLER_NAME="install-macos.sh"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    echo "Burnly on Windows uses the PowerShell installer:" >&2
    echo "  irm https://github.com/$REPO/releases/latest/download/install.ps1 | iex" >&2
    echo >&2
    echo "For a pinned release:" >&2
    echo "  \$env:BURNLY_VERSION='vX.Y.Z'; irm https://github.com/$REPO/releases/download/vX.Y.Z/install.ps1 | iex" >&2
    exit 1
    ;;
  *)
    echo "Unsupported operating system: $OS_NAME" >&2
    echo "Supported installers: Linux, macOS, and Windows PowerShell." >&2
    exit 1
    ;;
esac

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_command curl
require_command mktemp

if [ "$VERSION" = "latest" ]; then
  RELEASE_BASE_URL="https://github.com/$REPO/releases/latest/download"
else
  RELEASE_BASE_URL="https://github.com/$REPO/releases/download/$VERSION"
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

curl -fsSL "$RELEASE_BASE_URL/$INSTALLER_NAME" -o "$TMP_DIR/$INSTALLER_NAME"
sh "$TMP_DIR/$INSTALLER_NAME"
