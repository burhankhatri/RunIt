#!/bin/sh
# Install or update RunIt on macOS:
#   curl -fsSL https://raw.githubusercontent.com/burhankhatri/RunIt/main/install.sh | sh
set -eu
REPO="${RUNIT_REPO:-burhankhatri/RunIt}"
fail() { echo "RunIt: $*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || fail "this installer is for macOS. On Windows, run in PowerShell: irm https://raw.githubusercontent.com/$REPO/main/install.ps1 | iex"
[ "$(sw_vers -productVersion | cut -d. -f1)" -ge 14 ] || fail "RunIt needs macOS 14 or newer"
# A shell running under Rosetta reports Intel on an Apple silicon Mac.
if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = 1 ]; then ARCH=arm64; else ARCH=x86_64; fi
FILE="RunIt-macos-$ARCH.zip"
BASE="${RUNIT_BASE:-https://github.com/$REPO/releases/latest/download}"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
echo "Downloading RunIt"
curl -fL --retry 3 --progress-bar -o "$TMP/$FILE" "$BASE/$FILE" || fail "the download failed"
curl -fsSL --retry 3 -o "$TMP/SHA256SUMS" "$BASE/SHA256SUMS" || fail "the checksum list could not be downloaded"
(cd "$TMP" && grep " $FILE\$" SHA256SUMS | shasum -a 256 -c - >/dev/null 2>&1) || fail "the download does not match its checksum"
ditto -xk "$TMP/$FILE" "$TMP/app"
codesign --verify --deep --strict "$TMP/app/RunIt.app" 2>/dev/null || fail "the app is damaged"

DIR=/Applications
[ -w "$DIR" ] || { DIR="$HOME/Applications"; mkdir -p "$DIR"; }
osascript -e 'if application id "com.burhankhatri.runit" is running then tell application id "com.burhankhatri.runit" to quit' >/dev/null 2>&1 || true
rm -rf "$DIR/RunIt.app"
ditto "$TMP/app/RunIt.app" "$DIR/RunIt.app"
echo "Installed $DIR/RunIt.app"
[ -n "${RUNIT_NO_OPEN:-}" ] || open "$DIR/RunIt.app"
