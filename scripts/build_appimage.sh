#!/usr/bin/env bash
set -euo pipefail

echo "Building standalone AppImage for Nesstar Converter..."
cargo build --release --bin nesstar-gui

APP_DIR="dist/NesstarConverter.AppDir"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/usr/bin" "$APP_DIR/usr/share/applications" "$APP_DIR/usr/share/icons/hicolor/256x256/apps"

cp target/release/nesstar-gui "$APP_DIR/usr/bin/nesstar-gui"
chmod +x "$APP_DIR/usr/bin/nesstar-gui"

# AppRun launcher
cat <<'APPRUN' > "$APP_DIR/AppRun"
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/nesstar-gui" "$@"
APPRUN
chmod +x "$APP_DIR/AppRun"

# Desktop file
cat <<'DESKTOP' > "$APP_DIR/nesstar-converter.desktop"
[Desktop Entry]
Type=Application
Name=Nesstar Converter
Exec=nesstar-gui
Icon=nesstar-converter
Categories=Utility;Science;
Terminal=false
DESKTOP
cp "$APP_DIR/nesstar-converter.desktop" "$APP_DIR/usr/share/applications/"

# Generate placeholder SVG icon if not present
cat <<'SVG' > "$APP_DIR/nesstar-converter.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect width="100" height="100" rx="20" fill="#161b22"/>
  <text x="50" y="65" font-family="monospace" font-size="45" font-weight="bold" fill="#58a6ff" text-anchor="middle">NC</text>
</svg>
SVG
cp "$APP_DIR/nesstar-converter.svg" "$APP_DIR/usr/share/icons/hicolor/256x256/apps/"

echo "AppDir structure prepared at $APP_DIR"
