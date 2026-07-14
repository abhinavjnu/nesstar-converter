#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "============================================"
echo "  Nesstar Converter — Linux Build"
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

echo "→ Running PyInstaller (single-file mode)..."
pyinstaller --clean --noconfirm gui/nesstar_converter.spec

echo "→ Executable size: $(du -sh dist/NesstarConverter | cut -f1)"

# Clean up build env
deactivate
rm -rf .build_env build

echo ""
echo "============================================"
echo "  BUILD SUCCESSFUL!"
echo "  Output: dist/NesstarConverter"
echo "============================================"
echo ""
echo "To run:  ./dist/NesstarConverter"
echo "To distribute: share the single executable."
