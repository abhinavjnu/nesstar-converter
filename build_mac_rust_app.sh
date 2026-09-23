#!/bin/bash
set -e

APP_DIR="dist/Nesstar Converter.app"

echo "=== Creating macOS App Bundle ==="
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

echo "=== Building Release Binary ==="
rustup target add x86_64-apple-darwin aarch64-apple-darwin 2>/dev/null || true

if cargo build --release --target x86_64-apple-darwin --bin nesstar-gui && \
   cargo build --release --target aarch64-apple-darwin --bin nesstar-gui; then
    echo "=== Creating Universal Binary (x86_64 + arm64) ==="
    lipo -create -output "$APP_DIR/Contents/MacOS/Nesstar Converter" \
        target/x86_64-apple-darwin/release/nesstar-gui \
        target/aarch64-apple-darwin/release/nesstar-gui
else
    echo "=== Falling back to host architecture build ==="
    cargo build --release --bin nesstar-gui
    cp target/release/nesstar-gui "$APP_DIR/Contents/MacOS/Nesstar Converter"
fi

chmod +x "$APP_DIR/Contents/MacOS/Nesstar Converter"

# Copy icon
if [ -f "dist/icon-windowed.icns" ]; then
    cp dist/icon-windowed.icns "$APP_DIR/Contents/Resources/icon-windowed.icns"
elif [ -f "gui/icon-windowed.icns" ]; then
    cp gui/icon-windowed.icns "$APP_DIR/Contents/Resources/icon-windowed.icns"
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
    <string>1.0.9</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

if command -v codesign >/dev/null 2>&1; then
    echo "=== Ad-hoc Signing App Bundle ==="
    codesign --force --deep --sign - "$APP_DIR"
fi

echo "=== macOS App Bundle Created Successfully at $APP_DIR ==="
du -sh "$APP_DIR"
