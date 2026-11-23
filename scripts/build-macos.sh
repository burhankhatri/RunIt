#!/bin/sh
# Build RunIt.app. Usage: scripts/build-macos.sh [arm64|x86_64] ; output in dist/
set -eu
cd "$(dirname "$0")/.."
ARCH="${1:-$(uname -m)}"
case "$ARCH" in arm64) TRIPLE=aarch64-apple-darwin ;; x86_64) TRIPLE=x86_64-apple-darwin ;; *) echo "unknown arch $ARCH" >&2; exit 1 ;; esac
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TARGET="${CARGO_TARGET_DIR:-target}"
export MACOSX_DEPLOYMENT_TARGET=14.0

scripts/fetch-engine.sh "macos-$ARCH"
cargo build --release --target "$TRIPLE" -p runit-ffi -p runit-cli
LIB="$(cd "$TARGET/$TRIPLE/release" && pwd)"
SCRATCH="${SWIFT_BUILD_DIR:-apps/macos/.build}"
# -lrunit would prefer the dylib, so name the static library outright.
swift build -c release --package-path apps/macos --scratch-path "$SCRATCH" --triple "$ARCH-apple-macosx14.0" -Xlinker "$LIB/librunit.a"
BIN=$(swift build -c release --package-path apps/macos --scratch-path "$SCRATCH" --triple "$ARCH-apple-macosx14.0" --show-bin-path)

APP="${DIST:-dist}/RunIt.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/fonts"
cp "$BIN/RunIt" "$APP/Contents/MacOS/RunIt"
cp "$LIB/runit" "$APP/Contents/MacOS/runit-cli"
cp apps/shared/fonts/* "$APP/Contents/Resources/fonts/"
cp apps/macos/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
cp -R "engines/macos-$ARCH" "$APP/Contents/Resources/engine"
sed "s/@VERSION@/$VERSION/g" apps/macos/Info.plist > "$APP/Contents/Info.plist"
find "$APP" -name '._*' -delete
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
echo "$APP"
