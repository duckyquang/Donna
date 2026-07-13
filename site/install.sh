#!/bin/sh
# Donna one-line installer for macOS.
#
#   curl -fsSL https://duckyquang.github.io/Donna/install.sh | sh
#
# Why this exists: Donna is a free, unsigned app, and browser downloads get
# macOS's quarantine flag (the "damaged" dialog). curl downloads don't, so this
# path installs with no security dialogs at all. The script downloads the
# latest release from GitHub, installs Donna.app into /Applications (or
# ~/Applications if that isn't writable), and opens it.
#
# Environment knobs:
#   DONNA_INSTALL_DIR  override the install directory
#   DONNA_NO_OPEN      set to 1 to skip launching Donna at the end
set -eu

REPO="duckyquang/Donna"

case "$(uname -s)" in
  Darwin) ;;
  *)
    echo "This installer is for macOS. Linux and Windows builds live at:"
    echo "  https://github.com/$REPO/releases/latest"
    exit 1
    ;;
esac

case "$(uname -m)" in
  arm64) ARCH="aarch64" ;;
  x86_64) ARCH="x64" ;;
  *)
    echo "Unsupported architecture: $(uname -m)"
    exit 1
    ;;
esac

ASSET="Donna_${ARCH}.app.tar.gz"
URL="https://github.com/$REPO/releases/latest/download/$ASSET"

DEST="${DONNA_INSTALL_DIR:-/Applications}"
if [ ! -w "$DEST" ]; then
  DEST="$HOME/Applications"
  mkdir -p "$DEST"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading Donna ($ARCH)..."
curl -fL --progress-bar "$URL" -o "$TMP/$ASSET"

echo "Unpacking..."
tar -xzf "$TMP/$ASSET" -C "$TMP"
[ -d "$TMP/Donna.app" ] || { echo "Unexpected archive layout; aborting."; exit 1; }

# Quit a running Donna so the bundle can be replaced cleanly.
if pgrep -xq Donna 2>/dev/null || pgrep -xq donna 2>/dev/null; then
  osascript -e 'tell application "Donna" to quit' >/dev/null 2>&1 || true
  sleep 1
fi

rm -rf "$DEST/Donna.app"
mv "$TMP/Donna.app" "$DEST/Donna.app"

# Belt and braces: clear any quarantine flag (harmless when absent).
xattr -cr "$DEST/Donna.app" 2>/dev/null || true

echo "Donna installed at $DEST/Donna.app"
if [ -z "${DONNA_NO_OPEN:-}" ]; then
  echo "Opening..."
  open "$DEST/Donna.app"
fi
