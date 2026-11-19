#!/bin/sh
# Builds target/release/Seer.app from the release binary.
#
# The app links the FFmpeg libraries found by pkg-config at build time
# (Homebrew's, on a development Mac), so the bundle runs on this machine.
# A build for distribution must embed its own LGPL FFmpeg dylibs instead.
set -eu
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP=target/release/Seer.app
ICONSET=target/release/Seer.iconset

cargo build --release -p seer

rm -rf "$APP" "$ICONSET"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ICONSET"
cp target/release/seer "$APP/Contents/MacOS/seer"

for size in 16 32 128 256 512; do
    sips -z "$size" "$size" app-icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) app-icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Seer.icns"
rm -rf "$ICONSET"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Seer</string>
    <key>CFBundleDisplayName</key><string>Seer</string>
    <key>CFBundleIdentifier</key><string>codes.imran.seer</string>
    <key>CFBundleExecutable</key><string>seer</string>
    <key>CFBundleIconFile</key><string>Seer</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.video</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signature, enough to run locally. Distribution needs a Developer ID.
codesign --force --sign - "$APP" >/dev/null 2>&1
echo "Built $APP ($VERSION)"
