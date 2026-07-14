#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "============================================"
echo "  Nesstar Converter — macOS Build"
echo "============================================"
echo ""

# Clean previous builds
echo "→ Cleaning previous builds..."
rm -rf build dist

# Create a clean virtual environment for the build
echo "→ Creating clean build environment..."
python3 -m venv .build_env
source .build_env/bin/activate

echo "→ Installing dependencies..."
pip install --upgrade pip --quiet
pip install -e ".[gui,build]" --quiet

echo "→ Running PyInstaller..."
pyinstaller --clean --noconfirm gui/nesstar_converter.spec

echo "→ Bundle size: $(du -sh 'dist/Nesstar Converter.app' | cut -f1)"

# Clean up build env
deactivate
rm -rf .build_env build

echo ""
echo "============================================"
echo "  BUILD SUCCESSFUL!"
echo "  Output: dist/Nesstar Converter.app"
echo "============================================"
echo ""
echo "To run:  open 'dist/Nesstar Converter.app'"
echo "To distribute: zip the .app and share it."
