"""Regression contract for the redistributable Rust migration fixtures."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from nesstar_converter import (
    _parse_resource_layouts,
    convert_nesstar,
    extract_block,
    extract_block_resource_indexed,
    find_metadata_sections,
    match_ddi_to_slots,
    parse_ddi,
    read_metadata_slots,
)

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures"


def _expected(stem: str):
    return json.loads((FIXTURES / "expected" / f"{stem}.json").read_text(encoding="utf-8"))


def _rows(frame):
    return [{column: str(row[column]) for column in frame.columns} for _, row in frame.iterrows()]


def test_fixture_generator_is_deterministic():
    result = subprocess.run(
        [sys.executable, "tools/generate_rust_fixtures.py", "--check"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr


def test_manifest_hashes_dimensions_and_synthetic_provenance():
    manifest = json.loads((FIXTURES / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["license"] == "CC0-1.0"
    assert manifest["fixtures"]["metadata-scan"]["rows"] == 4
    assert manifest["fixtures"]["resource-index"]["columns"] == 10
    assert set(manifest["fixtures"]["resource-index"]["encodings"]) >= {"nibble", "uint40", "compact_double"}
    for relative, expected_hash in manifest["files"].items():
        assert hashlib.sha256((FIXTURES / relative).read_bytes()).hexdigest() == expected_hash


def test_metadata_scan_fixture_matches_python_oracle_including_order():
    ddi_path = FIXTURES / "synthetic" / "metadata-scan.ddi.xml"
    data = (FIXTURES / "synthetic" / "metadata-scan.Nesstar").read_bytes()
    blocks = parse_ddi(str(ddi_path))
    block = blocks["F1"]
    start = find_metadata_sections(data, blocks)["F1"]
    merged = match_ddi_to_slots(block["ddi_vars"], read_metadata_slots(data, start, 3))
    assert _rows(extract_block(data, block, merged, start)) == _expected("metadata-scan")


def test_resource_index_fixture_matches_python_oracle_for_all_compact_encodings():
    ddi_path = FIXTURES / "synthetic" / "resource-index.ddi.xml"
    data = (FIXTURES / "synthetic" / "resource-index.Nesstar").read_bytes()
    blocks = parse_ddi(str(ddi_path))
    layout = _parse_resource_layouts(data, blocks)["F2"]
    assert _rows(extract_block_resource_indexed(data, blocks["F2"], layout)) == _expected("resource-index")


@pytest.mark.parametrize(
    ("name", "message"),
    [
        ("bad-magic.Nesstar", "Not a valid Nesstar file"),
        ("truncated-metadata.Nesstar", "Could not find any data blocks"),
        ("truncated-resource.Nesstar", "Could not find any data blocks"),
    ],
)
def test_malformed_variants_return_errors_not_success(tmp_path, name, message):
    ddi = FIXTURES / "synthetic" / ("metadata-scan.ddi.xml" if "metadata" in name or "magic" in name else "resource-index.ddi.xml")
    with pytest.raises((ValueError, RuntimeError), match=message):
        convert_nesstar(str(FIXTURES / "malformed" / name), str(ddi), str(tmp_path), formats=["json"], verbose=False)
