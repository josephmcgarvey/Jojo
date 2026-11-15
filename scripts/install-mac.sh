#!/bin/bash
# Installs or upgrades Jojo on macOS from the latest GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/josephmcgarvey/Jojo/master/scripts/install-mac.sh | bash
#
# The .dmg is downloaded with curl, which does not set the
# com.apple.quarantine attribute that browsers set, so Gatekeeper does not
# block the first launch of the unsigned app. Set JOJO_VERSION (for example
# JOJO_VERSION=0.2.5) to install a specific release instead of the latest.
#
# Running sessions are not affected: they belong to the jojod daemon, which
# keeps running while the app is quit and replaced.
set -euo pipefail

REPO="josephmcgarvey/Jojo"
APP="/Applications/Jojo.app"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "This installer is for macOS. Windows installers are at https://github.com/$REPO/releases" >&2
  exit 1
fi

case "$(uname -m)" in
  arm64) ARCH="aarch64" ;;
  x86_64) ARCH="x64" ;;
  *) echo "Unsupported processor: $(uname -m)" >&2; exit 1 ;;
esac

if [ -n "${JOJO_VERSION:-}" ]; then
  API="https://api.github.com/repos/$REPO/releases/tags/v${JOJO_VERSION#v}"
else
  API="https://api.github.com/repos/$REPO/releases/latest"
fi

# The release's .dmg for this processor; its name carries the version.
URL="$(curl -fsSL "$API" | grep -o "\"browser_download_url\": *\"[^\"]*_${ARCH}\.dmg\"" | sed 's/.*"\(https[^"]*\)"$/\1/' | head -n 1)"
if [ -z "$URL" ]; then
  echo "No Jojo_*_${ARCH}.dmg found in the release ($API)." >&2
  exit 1
fi

WORK="$(mktemp -d)"
MOUNT="$WORK/mount"
cleanup() {
  hdiutil detach "$MOUNT" -quiet 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

echo "Downloading $(basename "$URL")"
curl -fL --progress-bar -o "$WORK/Jojo.dmg" "$URL"

mkdir -p "$MOUNT"
hdiutil attach "$WORK/Jojo.dmg" -nobrowse -quiet -mountpoint "$MOUNT"
if [ ! -d "$MOUNT/Jojo.app" ]; then
  echo "The disk image does not contain Jojo.app." >&2
  exit 1
fi

# Quit the running app so its files can be replaced; the daemon and its
# sessions keep running.
if pgrep -x Jojo >/dev/null 2>&1; then
  echo "Quitting Jojo (sessions keep running)"
  osascript -e 'quit app "Jojo"' >/dev/null 2>&1 || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    pgrep -x Jojo >/dev/null 2>&1 || break
    sleep 0.5
  done
fi

echo "Installing to $APP"
rm -rf "$APP"
ditto "$MOUNT/Jojo.app" "$APP"
# A copy made from a quarantined image would carry the attribute; remove it
# in case the image was downloaded some other way.
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

echo "Installed $(basename "$URL" .dmg). Opening Jojo."
open "$APP"
