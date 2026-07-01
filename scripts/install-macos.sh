#!/bin/sh
set -eu

REPO="${BURNLY_REPO:-fikrilal/burnly}"
VERSION="${BURNLY_VERSION:-latest}"
APP_PATH="${BURNLY_APP_PATH:-/Applications/Burnly.app}"
OS_NAME="${BURNLY_UNAME_S:-$(uname -s)}"
ARCH_NAME="${BURNLY_UNAME_M:-$(uname -m)}"

case "$OS_NAME" in
  Darwin) ;;
  Linux)
    echo "This is the macOS installer, but this machine is running Linux." >&2
    echo "Use the Linux installer instead:" >&2
    echo "  curl -fsSL https://github.com/$REPO/releases/latest/download/install-linux.sh | sh" >&2
    exit 1
    ;;
  *)
    echo "Unsupported operating system for the macOS installer: $OS_NAME" >&2
    exit 1
    ;;
esac

case "$ARCH_NAME" in
  arm64 | aarch64)
    ARCHITECTURE="aarch64"
    ;;
  x86_64 | amd64)
    ARCHITECTURE="x86_64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH_NAME" >&2
    exit 1
    ;;
esac

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_command awk
require_command curl
require_command ditto
require_command hdiutil
require_command mktemp
require_command shasum
require_command sudo
require_command xattr

if [ "$VERSION" = "latest" ]; then
  VERSION="$(
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
      sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
      sed -n '1p'
  )"
  if [ -z "$VERSION" ]; then
    echo "Could not resolve the latest Burnly release." >&2
    exit 1
  fi
  RELEASE_BASE_URL="https://github.com/$REPO/releases/latest/download"
else
  RELEASE_BASE_URL="https://github.com/$REPO/releases/download/$VERSION"
fi

case "$VERSION" in
  v*)
    RELEASE_VERSION="${VERSION#v}"
    ;;
  *)
    echo "Burnly release tag must use the vX.Y.Z format: $VERSION" >&2
    exit 1
    ;;
esac

if [ -z "$RELEASE_VERSION" ] || [ "$RELEASE_VERSION" = "$VERSION" ]; then
  echo "Could not parse Burnly release version from $VERSION" >&2
  exit 1
fi

ASSET_NAME="burnly-v$RELEASE_VERSION-macos-$ARCHITECTURE.dmg"
TMP_DIR="$(mktemp -d)"
MOUNT_DIR="$TMP_DIR/mount"
MOUNTED="false"

cleanup() {
  if [ "$MOUNTED" = "true" ]; then
    hdiutil detach "$MOUNT_DIR" -quiet >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

mkdir -p "$MOUNT_DIR"

echo "Downloading Burnly $VERSION for macos-$ARCHITECTURE..."
curl -fL "$RELEASE_BASE_URL/$ASSET_NAME" -o "$TMP_DIR/$ASSET_NAME"
curl -fL "$RELEASE_BASE_URL/SHA256SUMS" -o "$TMP_DIR/SHA256SUMS"

if ! grep "  $ASSET_NAME\$" "$TMP_DIR/SHA256SUMS" >"$TMP_DIR/SHA256SUMS.selected"; then
  echo "SHA256SUMS does not contain $ASSET_NAME" >&2
  exit 1
fi

EXPECTED_SHA256="$(awk '{print $1}' "$TMP_DIR/SHA256SUMS.selected")"
ACTUAL_SHA256="$(shasum -a 256 "$TMP_DIR/$ASSET_NAME" | awk '{print $1}')"
if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
  echo "Checksum verification failed for $ASSET_NAME" >&2
  echo "Expected: $EXPECTED_SHA256" >&2
  echo "Actual:   $ACTUAL_SHA256" >&2
  exit 1
fi

echo "Mounting $ASSET_NAME..."
hdiutil attach "$TMP_DIR/$ASSET_NAME" -nobrowse -readonly -mountpoint "$MOUNT_DIR" -quiet
MOUNTED="true"

SOURCE_APP="$MOUNT_DIR/Burnly.app"
if [ ! -d "$SOURCE_APP" ]; then
  echo "Burnly.app was not found inside $ASSET_NAME" >&2
  exit 1
fi

copy_app() {
  rm -rf "$APP_PATH"
  ditto "$SOURCE_APP" "$APP_PATH"
}

copy_app_with_sudo() {
  echo "Copying to $APP_PATH requires administrator permission."
  sudo rm -rf "$APP_PATH"
  sudo ditto "$SOURCE_APP" "$APP_PATH"
}

echo "Installing Burnly to $APP_PATH..."
if ! copy_app 2>/dev/null; then
  copy_app_with_sudo
fi

if ! xattr -dr com.apple.quarantine "$APP_PATH" 2>/dev/null; then
  echo "Removing quarantine requires administrator permission."
  sudo xattr -dr com.apple.quarantine "$APP_PATH"
fi

hdiutil detach "$MOUNT_DIR" -quiet
MOUNTED="false"

echo "Burnly installed to $APP_PATH."
echo "Open it from Applications or Spotlight."
