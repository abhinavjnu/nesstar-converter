#!/usr/bin/env bash
set -euo pipefail

# Script to build a Debian (.deb) package for Nesstar Converter
# Can be run locally to package the compiled binary

VERSION="1.0.7"
ARCH="amd64"
PKG_DIR="/tmp/nesstar-deb"
OUTPUT_DIR="/home/abhinav/Downloads"
BINARY_SOURCE="/home/abhinav/.local/bin/NesstarConverter"

if [ ! -f "$BINARY_SOURCE" ]; then
    echo "Error: Binary not found at $BINARY_SOURCE"
    exit 1
fi

echo "Creating package structure..."
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/applications"

# Copy binary
cp "$BINARY_SOURCE" "$PKG_DIR/usr/bin/NesstarConverter"
chmod 755 "$PKG_DIR/usr/bin/NesstarConverter"

# Create Debian control file
cat <<EOT > "$PKG_DIR/DEBIAN/control"
Package: nesstar-converter
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: Abhinav Kumar <abhinavjnu@github.com>
Description: Convert Nesstar binary survey files to CSV, TXT, Parquet, and Stata DTA formats.
 A native desktop app for converting proprietary Nesstar survey files
 into open analytics formats. No terminal or Python required.
EOT

# Create Desktop entry
cat <<EOT > "$PKG_DIR/usr/share/applications/nesstar-converter.desktop"
[Desktop Entry]
Type=Application
Name=Nesstar Converter
Comment=Convert Nesstar binary survey files to open formats
Exec=NesstarConverter
Icon=system-run
Terminal=false
Categories=Utility;
Keywords=nesstar;converter;parquet;stata;csv;ddi;
EOT

echo "Building Debian package..."
dpkg-deb --build "$PKG_DIR" "$OUTPUT_DIR/nesstar-converter_${VERSION}_${ARCH}.deb"

echo "Cleaning up temporary files..."
rm -rf "$PKG_DIR"

echo "Debian package successfully built at $OUTPUT_DIR/nesstar-converter_${VERSION}_${ARCH}.deb"
