#!/bin/bash
set -e

echo "=== Building Release Binary ==="
cargo build --release --bin nesstar-gui

echo "=== Creating macOS App Bundle ==="
APP_DIR="dist/Nesstar Converter.app"

# Remove existing bundle to start fresh
rm -rf "$APP_DIR"

# Create directories
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy executable
cp target/release/nesstar-gui "$APP_DIR/Contents/MacOS/Nesstar Converter"

# Copy icon
if [ -f "dist/icon-windowed.icns" ]; then
    cp dist/icon-windowed.icns "$APP_DIR/Contents/Resources/icon-windowed.icns"
elif [ -f "gui/icon-windowed.icns" ]; then
    cp gui/icon-windowed.icns "$APP_DIR/Contents/Resources/icon-windowed.icns"
else
    # Fallback to copy from the build_mac source if it exists
    find dist -name "*.icns" -exec cp {} "$APP_DIR/Contents/Resources/icon-windowed.icns" \; || true
fi

# If we still don't have it, try copy from the Pyside package
if [ ! -f "$APP_DIR/Contents/Resources/icon-windowed.icns" ]; then
    cp .verify_env/lib/python3.14/site-packages/PySide6/scripts/deploy_lib/pyside_icon.icns "$APP_DIR/Contents/Resources/icon-windowed.icns" || true
fi

# Create Info.plist
cat <<EOF > "$APP_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>Nesstar Converter</string>
    <key>CFBundleIconFile</key>
    <string>icon-windowed.icns</string>
    <key>CFBundleIdentifier</key>
    <string>com.abhinavjnu.nesstar-converter-rust</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Nesstar Converter</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo "=== macOS App Bundle Created Successfully at $APP_DIR ==="
du -sh "$APP_DIR"
