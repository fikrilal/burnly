#!/bin/sh
set -eu

REPO="${BURNLY_REPO:-fikrilal/burnly}"
VERSION="${BURNLY_VERSION:-latest}"
INSTALL_DIR="${BURNLY_INSTALL_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/burnly}"
BIN_DIR="${BURNLY_BIN_DIR:-$HOME/.local/bin}"
APPLICATIONS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

case "$(uname -m)" in
  x86_64 | amd64)
    ARCHITECTURE="x86_64"
    ;;
  aarch64 | arm64)
    ARCHITECTURE="aarch64"
    ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
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
require_command sha256sum
require_command mktemp

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
  burnly-v*)
    RELEASE_VERSION="${VERSION#burnly-v}"
    ;;
  *)
    echo "Burnly release tag must use the vX.Y.Z format: $VERSION" >&2
    exit 1
    ;;
esac

ASSET_NAME="burnly-v$RELEASE_VERSION-linux-$ARCHITECTURE.AppImage"

if [ -z "$RELEASE_VERSION" ] || [ "$RELEASE_VERSION" = "$VERSION" ]; then
  echo "Could not parse Burnly release version from $VERSION" >&2
  exit 1
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

echo "Downloading Burnly $VERSION for linux-$ARCHITECTURE..."
curl -fL "$RELEASE_BASE_URL/$ASSET_NAME" -o "$TMP_DIR/$ASSET_NAME"
curl -fL "$RELEASE_BASE_URL/SHA256SUMS" -o "$TMP_DIR/SHA256SUMS"

if ! grep "  $ASSET_NAME\$" "$TMP_DIR/SHA256SUMS" >"$TMP_DIR/SHA256SUMS.selected"; then
  echo "SHA256SUMS does not contain $ASSET_NAME" >&2
  exit 1
fi

(cd "$TMP_DIR" && sha256sum -c SHA256SUMS.selected)

mkdir -p "$INSTALL_DIR" "$BIN_DIR" "$APPLICATIONS_DIR"
cp "$TMP_DIR/$ASSET_NAME" "$INSTALL_DIR/Burnly.AppImage"
chmod 755 "$INSTALL_DIR/Burnly.AppImage"

cat >"$BIN_DIR/burnly" <<EOF
#!/bin/sh
exec "$INSTALL_DIR/Burnly.AppImage" "\$@"
EOF
chmod 755 "$BIN_DIR/burnly"

cat >"$APPLICATIONS_DIR/burnly.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Burnly
Comment=Local AI coding-tool usage tracker
Exec=$INSTALL_DIR/Burnly.AppImage
Terminal=false
Categories=Development;Utility;
StartupNotify=false
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
fi

echo "Burnly installed:"
echo "  AppImage: $INSTALL_DIR/Burnly.AppImage"
echo "  Command:  $BIN_DIR/burnly"
echo "  Desktop:  $APPLICATIONS_DIR/burnly.desktop"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    echo
    echo "Add $BIN_DIR to PATH to run Burnly with: burnly"
    ;;
esac
