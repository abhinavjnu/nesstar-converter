#!/usr/bin/env python3
"""Basic usage example for nesstar-converter."""

from pathlib import Path
from nesstar_converter import convert_nesstar, show_info

# --- Step 1: Inspect a Nesstar file ---
# The ddi.xml file will be auto-detected from the same directory
NESSTAR_FILE = "path/to/your/file.Nesstar"
DDI_FILE = "path/to/your/ddi.xml"

print("=== File Info ===")
show_info(NESSTAR_FILE, DDI_FILE)

# --- Step 2: Convert to CSV ---
print("\n=== Converting to CSV ===")
report = convert_nesstar(
    NESSTAR_FILE,
    DDI_FILE,
    "./output",
    formats=["csv"],
    year="2023-24",
)

# --- Step 3: Check results ---
for block_name, info in report["blocks"].items():
    print(f"  {block_name}: {info['rows']:,} rows × {info['columns']} columns")
    for fmt, finfo in info["files"].items():
        print(f"    → {finfo['path']} ({finfo['size_mb']} MB)")
