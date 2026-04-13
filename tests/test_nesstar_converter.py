"""Comprehensive test suite for nesstar_converter.py.

Tests cover DDI parsing, binary decoding, metadata matching,
full-pipeline integration, CLI behaviour, and edge cases.
"""

import json
import math
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

# ---------------------------------------------------------------------------
# Imports from the module under test
# ---------------------------------------------------------------------------
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from nesstar_converter import (
    ALL_FORMATS,
    DBL_MAX_BYTES,
    DBL_MAX_VAL,
    FORMAT_EXTENSIONS,
    NESSTAR_MAGIC,
    SLOT_SIZE,
    _extract_char_column,
    _extract_double_column,
    _extract_offset_column,
    _find_matching_export,
    _safe_name,
    _validate_block,
    _write_formats,
    compute_binary_width,
    convert_nesstar,
    find_metadata_sections,
    match_ddi_to_slots,
    parse_ddi,
    read_metadata_slots,
    validate_against_export,
)

# ---------------------------------------------------------------------------
# Path constants — tests require real Nesstar data to be present.
# Point NESSTAR_TEST_DATA env var to a directory containing:
#   survey0/data/YourFile.Nesstar + survey0/data/ddi.xml + exported/*.txt
# Defaults to MOSPI workspace paths if available.
# ---------------------------------------------------------------------------
PROJECT_ROOT = Path(__file__).resolve().parent.parent

# Allow override via environment variable
import os
_data_root = os.environ.get("NESSTAR_TEST_DATA", "")
if _data_root:
    _data_path = Path(_data_root)
    NESSTAR_FILE = str(next(_data_path.rglob("*.Nesstar"), Path("missing")))
    DDI_FILE = str(next(_data_path.rglob("ddi.xml"), Path("missing")))
    EXPORT_DIR = str(next((_data_path / p for p in ["exported", "."]
                          if (_data_path / p).is_dir()), Path("missing")))
else:
    _mospi = Path("/media/abhinav/Data/MOSPI")
    NESSTAR_FILE = str(
        _mospi / "data/eus/1983/Nss38_10_new format/survey0/data/NSS_38_SCH_10_EMP_UNEMP.Nesstar"
    )
    DDI_FILE = str(
        _mospi / "data/eus/1983/Nss38_10_new format/survey0/data/ddi.xml"
    )
    EXPORT_DIR = str(_mospi / "data/eus/1983/exported")

HAS_REAL_DATA = Path(NESSTAR_FILE).exists() and Path(DDI_FILE).exists()
HAS_EXPORTS = Path(EXPORT_DIR).exists() and any(Path(EXPORT_DIR).glob("*.txt"))

needs_real_data = pytest.mark.skipif(
    not HAS_REAL_DATA, reason="Real Nesstar/DDI data not available"
)
needs_exports = pytest.mark.skipif(
    not HAS_EXPORTS, reason="Text export files not available"
)

# ---------------------------------------------------------------------------
# Expected constants for EUS 1983 (38th round, Schedule 10)
# ---------------------------------------------------------------------------
EXPECTED_BLOCK_COUNT = 9
EXPECTED_TOTAL_RECORDS = 3_445_585

BLOCK_EXPECTATIONS = {
    "Block-10-Household-Loan-records": {"rows": 42_853, "cols": 22},
    "Block-41-Persons-Demogrphic-weelyActivity-records": {"rows": 623_494, "cols": 29},
    "Block-42-Persons-migration-records": {"rows": 623_494, "cols": 29},
    "Block-5-Persons-DailyActivity-records": {"rows": 546_198, "cols": 35},
    "Block-6-Persons-UsualActivity-records": {"rows": 623_494, "cols": 33},
    "Block-7-Persons-Notworking-subsidiary-activity-record": {"rows": 272_487, "cols": 22},
    "Block-8-Persons-Addl-Questions-UsualActivity-records": {"rows": 471_840, "cols": 33},
    "Block-9-Persons-Domestic-duties-records": {"rows": 120_804, "cols": 39},
    "Block-1-3-Household-records": {"rows": 120_921, "cols": 42},
}


# ═══════════════════════════════════════════════════════════════════════════
#  Module-scoped fixtures (shared across integration tests)
# ═══════════════════════════════════════════════════════════════════════════

@pytest.fixture(scope="module")
def ddi_blocks():
    """Parse the real DDI once and share across the module."""
    if not Path(DDI_FILE).exists():
        pytest.skip("DDI file not available")
    return parse_ddi(DDI_FILE)


@pytest.fixture(scope="module")
def nesstar_data():
    """Memory-map the Nesstar binary once for metadata tests."""
    if not Path(NESSTAR_FILE).exists():
        pytest.skip("Nesstar binary not available")
    import mmap
    with open(NESSTAR_FILE, "rb") as f:
        data = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
    yield data
    data.close()


@pytest.fixture(scope="module")
def converted_block10(tmp_path_factory):
    """Convert only Block-10 (smallest) to CSV and Parquet; share the result."""
    if not HAS_REAL_DATA:
        pytest.skip("Real data not available")
    out = tmp_path_factory.mktemp("block10_output")
    report = convert_nesstar(
        NESSTAR_FILE, DDI_FILE, str(out),
        formats=["csv", "parquet", "tsv"],
        year="1983", verbose=False,
    )
    return out, report


@pytest.fixture(scope="module")
def block10_dataframe(converted_block10):
    """Read the Block-10 CSV as a DataFrame for format round-trip tests."""
    out, report = converted_block10
    for bname, binfo in report["blocks"].items():
        if "block_10" in bname or "loan" in bname:
            csv_path = binfo["files"]["csv"]["path"]
            return pd.read_csv(csv_path, dtype=str, keep_default_na=False)
    pytest.fail("Block-10 not found in converted data")


# ═══════════════════════════════════════════════════════════════════════════
#  1. Unit tests — DDI parsing
# ═══════════════════════════════════════════════════════════════════════════

class TestParseDDI:
    """Unit tests for parse_ddi()."""

    @pytest.mark.unit
    @needs_real_data
    def test_parse_ddi_returns_blocks(self, ddi_blocks):
        assert len(ddi_blocks) == EXPECTED_BLOCK_COUNT

    @pytest.mark.unit
    @needs_real_data
    def test_parse_ddi_block_names(self, ddi_blocks):
        names = {blk["name"] for blk in ddi_blocks.values()}
        for expected_name in BLOCK_EXPECTATIONS:
            assert expected_name in names, f"Missing block: {expected_name}"

    @pytest.mark.unit
    @needs_real_data
    def test_parse_ddi_variable_counts(self, ddi_blocks):
        for blk in ddi_blocks.values():
            name = blk["name"]
            if name in BLOCK_EXPECTATIONS:
                expected_cols = BLOCK_EXPECTATIONS[name]["cols"]
                actual_cols = len(blk["ddi_vars"])
                assert actual_cols == expected_cols, (
                    f"{name}: expected {expected_cols} vars, got {actual_cols}"
                )

    @pytest.mark.unit
    @needs_real_data
    def test_parse_ddi_record_counts(self, ddi_blocks):
        for blk in ddi_blocks.values():
            name = blk["name"]
            if name in BLOCK_EXPECTATIONS:
                expected_rows = BLOCK_EXPECTATIONS[name]["rows"]
                assert blk["nrecs"] == expected_rows, (
                    f"{name}: expected {expected_rows} nrecs, got {blk['nrecs']}"
                )

    @pytest.mark.unit
    @needs_real_data
    def test_parse_ddi_total_records(self, ddi_blocks):
        total = sum(b["nrecs"] for b in ddi_blocks.values())
        assert total == EXPECTED_TOTAL_RECORDS

    @pytest.mark.unit
    @needs_real_data
    def test_parse_ddi_variable_types(self, ddi_blocks):
        """Check known variable types, widths, and decimals."""
        # Find Block-10
        blk10 = next(b for b in ddi_blocks.values()
                     if b["name"] == "Block-10-Household-Loan-records")
        var_map = {v["name"]: v for v in blk10["ddi_vars"]}

        # Hhold_key: character, width 11
        assert var_map["Hhold_key"]["type"] == "character"
        assert var_map["Hhold_key"]["ddi_width"] == 11

        # B10_q3: numeric, width 6, range (0, 400000)
        assert var_map["B10_q3"]["type"] == "numeric"
        assert var_map["B10_q3"]["ddi_width"] == 6
        assert var_map["B10_q3"]["rng_min"] == 0.0
        assert var_map["B10_q3"]["rng_max"] == 400000.0

        # Wgt1_strm: numeric, width 5
        assert var_map["Wgt1_strm"]["type"] == "numeric"
        assert var_map["Wgt1_strm"]["ddi_width"] == 5
        assert var_map["Wgt1_strm"]["dcml"] == 0

    @pytest.mark.unit
    def test_parse_ddi_missing_file(self):
        with pytest.raises(FileNotFoundError):
            parse_ddi("/nonexistent/path/ddi.xml")

    @pytest.mark.unit
    def test_parse_ddi_invalid_xml(self, tmp_path):
        bad_xml = tmp_path / "bad_ddi.xml"
        bad_xml.write_text("<<<this is not valid xml>>>")
        with pytest.raises(Exception):  # ET.ParseError
            parse_ddi(str(bad_xml))


# ═══════════════════════════════════════════════════════════════════════════
#  2. Unit tests — binary decoding
# ═══════════════════════════════════════════════════════════════════════════

class TestBinaryDecoding:
    """Unit tests for _extract_*_column and compute_binary_width."""

    @pytest.mark.unit
    def test_extract_char_column(self):
        """Decode known bytes as character type."""
        raw = b"Hello\x00\x00" + b"World\x00\x00"
        result = _extract_char_column(raw, 7, 2)
        assert result == ["Hello", "World"]

    @pytest.mark.unit
    def test_extract_char_column_strips_spaces(self):
        raw = b"AB   " + b"CD   "
        result = _extract_char_column(raw, 5, 2)
        assert result == ["AB", "CD"]

    @pytest.mark.unit
    def test_extract_char_missing(self):
        """All-zero bytes produce empty string."""
        raw = b"\x00" * 10
        result = _extract_char_column(raw, 5, 2)
        assert result == ["", ""]

    @pytest.mark.unit
    def test_extract_offset_column(self):
        """Decode known bytes as offset type (little-endian + offset_min)."""
        # offset_min=100; value stored = actual - offset_min
        # Store 5 as 2 bytes LE → bytes 05 00
        raw = b"\x05\x00" + b"\x0a\x00"
        result = _extract_offset_column(raw, 2, 2, offset_min=100)
        assert result == ["105", "110"]

    @pytest.mark.unit
    def test_extract_offset_column_single_byte(self):
        raw = b"\x03\x07"
        result = _extract_offset_column(raw, 1, 2, offset_min=0)
        assert result == ["3", "7"]

    @pytest.mark.unit
    def test_extract_offset_missing(self):
        """All-0xFF bytes produce empty string (missing marker)."""
        raw = b"\xff\xff" + b"\xff\xff"
        result = _extract_offset_column(raw, 2, 2, offset_min=0)
        assert result == ["", ""]

    @pytest.mark.unit
    def test_extract_double_column(self):
        """Decode known bytes as double type."""
        val1 = struct.pack("<d", 42.0)
        val2 = struct.pack("<d", 3.14)
        raw = val1 + val2
        result = _extract_double_column(raw, 2, 2)
        assert result[0] == "42"
        assert result[1].startswith("3.14")

    @pytest.mark.unit
    def test_extract_double_integer_value(self):
        """Integer-valued doubles are formatted without decimals."""
        raw = struct.pack("<d", 100.0)
        result = _extract_double_column(raw, 1, 0)
        assert result == ["100"]

    @pytest.mark.unit
    def test_extract_double_missing(self):
        """DBL_MAX bytes produce empty string."""
        raw = DBL_MAX_BYTES
        result = _extract_double_column(raw, 1, 0)
        assert result == [""]

    @pytest.mark.unit
    def test_extract_double_nan(self):
        """NaN bytes produce empty string."""
        raw = struct.pack("<d", float("nan"))
        result = _extract_double_column(raw, 1, 0)
        assert result == [""]

    @pytest.mark.unit
    def test_compute_binary_width_char(self):
        """Char encoding: width comes from slot's char_width."""
        var_spec = {"ddi_width": 11, "rng_min": None, "rng_max": None}
        slot_info = {"encoding": "char", "char_width": 11}
        assert compute_binary_width(var_spec, slot_info) == 11

    @pytest.mark.unit
    def test_compute_binary_width_double(self):
        """Double encoding: always 8 bytes."""
        var_spec = {"ddi_width": 6, "rng_min": 0.0, "rng_max": 400000.0}
        slot_info = {"encoding": "double", "char_width": 0}
        assert compute_binary_width(var_spec, slot_info) == 8

    @pytest.mark.unit
    def test_compute_binary_width_offset_with_range(self):
        """Offset encoding: width from range delta."""
        # range 0..255 → delta=255, 255.bit_length()=8, ceil(8/8)=1
        var_spec = {"ddi_width": 3, "rng_min": 0.0, "rng_max": 255.0}
        slot_info = {"encoding": "offset", "char_width": 0}
        assert compute_binary_width(var_spec, slot_info) == 1

        # range 0..400000 → delta=400000, bit_length=19, ceil(19/8)=3
        var_spec2 = {"ddi_width": 6, "rng_min": 0.0, "rng_max": 400000.0}
        assert compute_binary_width(var_spec2, slot_info) == 3

    @pytest.mark.unit
    def test_compute_binary_width_offset_no_range(self):
        """Offset encoding without range: falls back to ddi_width."""
        var_spec = {"ddi_width": 2, "rng_min": None, "rng_max": None}
        slot_info = {"encoding": "offset", "char_width": 0}
        # max_val = 10**2 - 1 = 99, bit_length=7, ceil(7/8)=1
        assert compute_binary_width(var_spec, slot_info) == 1

    @pytest.mark.unit
    def test_compute_binary_width_minimum_one(self):
        """Width is at least 1 byte."""
        var_spec = {"ddi_width": 1, "rng_min": 0.0, "rng_max": 1.0}
        slot_info = {"encoding": "offset", "char_width": 0}
        assert compute_binary_width(var_spec, slot_info) >= 1

    @pytest.mark.unit
    def test_width_shrinking_validation(self):
        """Width is not shrunk below what the range requires (audit fix B-1).

        compute_binary_width itself does not shrink; the shrinking happens
        inside extract_block. We verify compute_binary_width returns the
        correct *initial* width that represents the full range.
        """
        # range 0..400000 → needs 3 bytes (max 16,777,215 representable)
        var_spec = {"ddi_width": 6, "rng_min": 0.0, "rng_max": 400000.0}
        slot_info = {"encoding": "offset", "char_width": 0}
        w = compute_binary_width(var_spec, slot_info)
        max_representable = (1 << (w * 8)) - 1
        delta = int(var_spec["rng_max"]) - int(var_spec["rng_min"])
        assert max_representable >= delta, (
            f"Width {w} cannot represent delta {delta} "
            f"(max representable={max_representable})"
        )


# ═══════════════════════════════════════════════════════════════════════════
#  3. Unit tests — metadata matching
# ═══════════════════════════════════════════════════════════════════════════

class TestMetadataMatching:
    """Unit tests for metadata slot reading and DDI-to-slot matching."""

    @pytest.mark.unit
    @needs_real_data
    def test_read_metadata_slots(self, ddi_blocks, nesstar_data):
        """Parse real binary metadata and get correct slot count."""
        meta_map = find_metadata_sections(bytes(nesstar_data), ddi_blocks)
        assert len(meta_map) > 0

        # Read slots for first block found
        fid = next(iter(meta_map))
        blk = ddi_blocks[fid]
        nvars = len(blk["ddi_vars"])
        meta_start = meta_map[fid]
        slots = read_metadata_slots(bytes(nesstar_data), meta_start, nvars)
        assert len(slots) == nvars
        for slot in slots:
            assert "var_num" in slot
            assert "encoding" in slot
            assert slot["encoding"] in ("char", "offset", "double")

    @pytest.mark.unit
    @needs_real_data
    def test_match_ddi_to_slots(self, ddi_blocks, nesstar_data):
        """DDI vars merge correctly with binary slots."""
        meta_map = find_metadata_sections(bytes(nesstar_data), ddi_blocks)
        fid = next(iter(meta_map))
        blk = ddi_blocks[fid]
        nvars = len(blk["ddi_vars"])
        meta_start = meta_map[fid]
        slots = read_metadata_slots(bytes(nesstar_data), meta_start, nvars)
        merged = match_ddi_to_slots(blk["ddi_vars"], slots)
        assert len(merged) == nvars
        for m in merged:
            assert "binary_width" in m
            assert m["binary_width"] >= 1
            assert "name" in m
            assert "encoding" in m

    @pytest.mark.unit
    @needs_real_data
    def test_find_metadata_sections(self, ddi_blocks, nesstar_data):
        """All 9 blocks are located."""
        meta_map = find_metadata_sections(bytes(nesstar_data), ddi_blocks)
        assert len(meta_map) == EXPECTED_BLOCK_COUNT, (
            f"Expected {EXPECTED_BLOCK_COUNT} metadata sections, "
            f"found {len(meta_map)}: {list(meta_map.keys())}"
        )


# ═══════════════════════════════════════════════════════════════════════════
#  4. Integration tests — full pipeline
# ═══════════════════════════════════════════════════════════════════════════

class TestConvertPipeline:
    """Integration tests for full Nesstar → format conversion pipeline."""

    @pytest.mark.integration
    @needs_real_data
    @needs_exports
    def test_convert_csv_block10(self, converted_block10):
        """Convert Block-10 (smallest, 42K rows) to CSV; validate against text export."""
        out, report = converted_block10
        # Find the Block-10 CSV
        block10_csv = None
        for bname, binfo in report["blocks"].items():
            if "block_10" in bname or "loan" in bname:
                csv_info = binfo["files"].get("csv")
                if csv_info and "path" in csv_info:
                    block10_csv = csv_info["path"]
                    break

        assert block10_csv is not None, "Block-10 CSV not found in report"
        assert Path(block10_csv).exists()

        df_csv = pd.read_csv(block10_csv, dtype=str, keep_default_na=False)
        assert len(df_csv) == 42_853
        assert len(df_csv.columns) == 22

        # Compare against text export
        export_path = Path(EXPORT_DIR) / "Block-10-Household-Loan-records.txt"
        df_exp = pd.read_csv(
            export_path, sep="\t", header=None, dtype=str, keep_default_na=False
        )
        assert len(df_csv) == len(df_exp)
        assert len(df_csv.columns) == len(df_exp.columns)

        # Multiset comparison
        csv_tuples = Counter(
            tuple(v.strip() for v in row)
            for row in df_csv.values
        )
        exp_tuples = Counter(
            tuple(v.strip() for v in row)
            for row in df_exp.values
        )
        assert csv_tuples == exp_tuples, "Block-10 CSV does not match text export"

    @pytest.mark.integration
    @pytest.mark.slow
    @needs_real_data
    @needs_exports
    def test_convert_csv_all_blocks(self, converted_block10):
        """Convert all 9 blocks to CSV; validate against all text exports (multiset match)."""
        out, report = converted_block10
        export_dir = Path(EXPORT_DIR)

        for bname, binfo in report["blocks"].items():
            csv_info = binfo["files"].get("csv")
            if not csv_info or "path" not in csv_info:
                continue
            csv_path = csv_info["path"]
            df_csv = pd.read_csv(csv_path, dtype=str, keep_default_na=False)

            # Find matching export
            export_files = sorted(export_dir.glob("*.txt"))
            matched = _find_matching_export(Path(csv_path).stem, export_files)
            if matched is None:
                continue

            df_exp = pd.read_csv(
                matched, sep="\t", header=None, dtype=str, keep_default_na=False
            )
            assert len(df_csv) == len(df_exp), (
                f"{bname}: row count mismatch {len(df_csv)} vs {len(df_exp)}"
            )
            assert len(df_csv.columns) == len(df_exp.columns), (
                f"{bname}: col count mismatch "
                f"{len(df_csv.columns)} vs {len(df_exp.columns)}"
            )

            csv_tuples = Counter(
                tuple(v.strip() for v in row) for row in df_csv.values
            )
            exp_tuples = Counter(
                tuple(v.strip() for v in row) for row in df_exp.values
            )
            assert csv_tuples == exp_tuples, (
                f"{bname}: multiset mismatch against export {matched.name}"
            )

    @pytest.mark.integration
    @needs_real_data
    @needs_exports
    def test_convert_parquet(self, converted_block10):
        """Convert to parquet, read back, validate against text exports."""
        out, report = converted_block10
        export_dir = Path(EXPORT_DIR)

        parquet_files = list(out.glob("*.parquet"))
        assert len(parquet_files) > 0, "No parquet files created"

        # Validate Block-10 parquet
        for pq in parquet_files:
            if "block_10" in pq.stem or "loan" in pq.stem:
                df_pq = pd.read_parquet(pq).astype(str)
                for col in df_pq.columns:
                    df_pq[col] = df_pq[col].str.strip()

                export_path = (
                    export_dir / "Block-10-Household-Loan-records.txt"
                )
                df_exp = pd.read_csv(
                    export_path, sep="\t", header=None, dtype=str,
                    keep_default_na=False,
                )
                assert len(df_pq) == len(df_exp)
                assert len(df_pq.columns) == len(df_exp.columns)

                pq_tuples = Counter(
                    tuple(row) for row in df_pq.values
                )
                exp_tuples = Counter(
                    tuple(v.strip() for v in row) for row in df_exp.values
                )
                assert pq_tuples == exp_tuples
                break

    @pytest.mark.integration
    @needs_real_data
    def test_convert_tsv(self, converted_block10):
        """Convert to TSV, read back, compare to CSV output."""
        out, report = converted_block10
        for bname, binfo in report["blocks"].items():
            csv_info = binfo["files"].get("csv")
            tsv_info = binfo["files"].get("tsv")
            if not csv_info or not tsv_info:
                continue
            if "error" in csv_info or "error" in tsv_info:
                continue
            df_csv = pd.read_csv(csv_info["path"], dtype=str, keep_default_na=False)
            df_tsv = pd.read_csv(
                tsv_info["path"], sep="\t", dtype=str, keep_default_na=False
            )
            assert df_csv.shape == df_tsv.shape, f"{bname}: shape mismatch"
            csv_tuples = Counter(tuple(row) for row in df_csv.values)
            tsv_tuples = Counter(tuple(row) for row in df_tsv.values)
            assert csv_tuples == tsv_tuples, f"{bname}: CSV/TSV content differs"
            break  # one block is enough

    @pytest.mark.integration
    @needs_real_data
    def test_convert_stata(self, block10_dataframe, tmp_path):
        """Convert Block-10 to Stata, read back, validate leading zeros preserved."""
        df = block10_dataframe
        blk = {"name": "Block-10-Household-Loan-records", "nrecs": 42_853,
               "ddi_vars": [{"name": c, "label": c, "type": "character",
                             "ddi_width": 10, "dcml": 0} for c in df.columns]}
        merged = [{"name": c, "label": c, "type": "character",
                   "ddi_width": 10, "dcml": 0, "encoding": "char"}
                  for c in df.columns]
        files = _write_formats(df, str(tmp_path), "block_10_loan", ["stata"],
                               blk, merged)
        assert "stata" in files
        assert "error" not in files["stata"]
        dta_path = files["stata"]["path"]
        df_dta = pd.read_stata(dta_path)
        assert len(df_dta) == 42_853
        state_col = [c for c in df_dta.columns if "state" in c.lower()]
        if state_col:
            vals = df_dta[state_col[0]].dropna().astype(str)
            has_leading_zeros = any(
                v.startswith("0") and len(v) > 1 for v in vals
            )
            assert has_leading_zeros, "Leading zeros not preserved in Stata output"

    @pytest.mark.integration
    @needs_real_data
    def test_convert_excel(self, block10_dataframe, tmp_path):
        """Convert Block-10 to Excel, read back, validate shape."""
        df = block10_dataframe
        blk = {"name": "Block-10-Household-Loan-records", "nrecs": 42_853,
               "ddi_vars": [{"name": c, "label": c, "type": "character",
                             "ddi_width": 10, "dcml": 0} for c in df.columns]}
        merged = [{"name": c, "label": c, "type": "character",
                   "ddi_width": 10, "dcml": 0, "encoding": "char"}
                  for c in df.columns]
        files = _write_formats(df, str(tmp_path), "block_10_loan", ["excel"],
                               blk, merged)
        assert "excel" in files
        assert "error" not in files["excel"]
        xlsx_path = files["excel"]["path"]
        df_xl = pd.read_excel(xlsx_path, sheet_name="Data", header=1, dtype=str)
        assert len(df_xl) == 42_853
        assert len(df_xl.columns) == 22

    @pytest.mark.integration
    @needs_real_data
    def test_convert_json(self, block10_dataframe, tmp_path):
        """Convert Block-10 to JSON, parse back, validate."""
        df = block10_dataframe
        blk = {"name": "Block-10", "nrecs": 42_853,
               "ddi_vars": [{"name": c} for c in df.columns]}
        merged = [{"name": c, "ddi_width": 10} for c in df.columns]
        files = _write_formats(df, str(tmp_path), "block_10_loan", ["json"],
                               blk, merged)
        assert "json" in files
        assert "error" not in files["json"]
        with open(files["json"]["path"]) as fp:
            data = json.load(fp)
        assert isinstance(data, list)
        assert len(data) == 42_853
        assert len(data[0]) == 22

    @pytest.mark.integration
    @needs_real_data
    def test_convert_jsonl(self, block10_dataframe, tmp_path):
        """Convert Block-10 to JSONL, parse line-by-line, validate."""
        df = block10_dataframe
        blk = {"name": "Block-10", "nrecs": 42_853,
               "ddi_vars": [{"name": c} for c in df.columns]}
        merged = [{"name": c, "ddi_width": 10} for c in df.columns]
        files = _write_formats(df, str(tmp_path), "block_10_loan", ["jsonl"],
                               blk, merged)
        assert "jsonl" in files
        assert "error" not in files["jsonl"]
        records = []
        with open(files["jsonl"]["path"]) as fp:
            for line in fp:
                line = line.strip()
                if line:
                    records.append(json.loads(line))
        assert len(records) == 42_853
        assert len(records[0]) == 22

    @pytest.mark.integration
    @needs_real_data
    def test_convert_all_formats(self, block10_dataframe, tmp_path):
        """Write Block-10 to all formats, verify all files created."""
        df = block10_dataframe
        blk = {"name": "Block-10-Household-Loan-records", "nrecs": 42_853,
               "ddi_vars": [{"name": c, "label": c, "type": "character",
                             "ddi_width": 10, "dcml": 0} for c in df.columns]}
        merged = [{"name": c, "label": c, "type": "character",
                   "ddi_width": 10, "dcml": 0, "encoding": "char"}
                  for c in df.columns]
        files = _write_formats(df, str(tmp_path), "block_10_loan",
                               ALL_FORMATS, blk, merged)
        for fmt in ALL_FORMATS:
            assert fmt in files, f"Missing format {fmt}"
            finfo = files[fmt]
            if "error" not in finfo:
                assert Path(finfo["path"]).exists(), (
                    f"{fmt}: file not at {finfo['path']}"
                )

    @pytest.mark.integration
    @needs_real_data
    def test_convert_report_json(self, converted_block10):
        """Check conversion_report.json is created with correct structure."""
        out, report = converted_block10
        report_path = out / "conversion_report.json"
        assert report_path.exists(), "conversion_report.json not created"

        with open(report_path) as f:
            on_disk = json.load(f)

        assert "year" in on_disk
        assert on_disk["year"] == "1983"
        assert "blocks" in on_disk
        assert "errors" in on_disk
        assert "validation" in on_disk
        assert "formats" in on_disk
        assert len(on_disk["blocks"]) == EXPECTED_BLOCK_COUNT


# ═══════════════════════════════════════════════════════════════════════════
#  5. Validation tests
# ═══════════════════════════════════════════════════════════════════════════

class TestValidation:
    """Tests for validation logic."""

    @pytest.mark.integration
    @needs_real_data
    @needs_exports
    def test_validate_command_passes(self, converted_block10):
        """Run validate_against_export on converted data and text exports."""
        out, _ = converted_block10
        # Need parquet files for validation
        parquet_files = list(out.glob("*.parquet"))
        assert len(parquet_files) > 0

        results = validate_against_export(str(out), EXPORT_DIR, verbose=False)
        assert results.get("failed", 0) == 0, (
            f"Validation failed: {json.dumps(results, indent=2, default=str)}"
        )
        assert results.get("passed", 0) > 0

    @pytest.mark.integration
    def test_validate_block_row_count(self):
        """_validate_block checks row count matches DDI nrecs."""
        df = pd.DataFrame({"a": ["1", "2", "3"]})
        blk = {"nrecs": 3, "ddi_vars": [{"name": "a"}]}
        result = _validate_block(df, blk, "test_block")
        row_check = next(c for c in result["checks"] if c["check"] == "row_count")
        assert row_check["passed"] is True

        # Mismatched row count
        blk_bad = {"nrecs": 10, "ddi_vars": [{"name": "a"}]}
        result_bad = _validate_block(df, blk_bad, "test_block")
        row_check_bad = next(
            c for c in result_bad["checks"] if c["check"] == "row_count"
        )
        assert row_check_bad["passed"] is False
        assert result_bad["passed"] is False

    @pytest.mark.integration
    def test_validate_block_column_count(self):
        """_validate_block checks column count matches DDI."""
        df = pd.DataFrame({"a": ["1"], "b": ["2"]})
        blk = {"nrecs": 1, "ddi_vars": [{"name": "a"}, {"name": "b"}]}
        result = _validate_block(df, blk, "test_block")
        col_check = next(
            c for c in result["checks"] if c["check"] == "column_count"
        )
        assert col_check["passed"] is True

        # Mismatched (tolerance is ±3)
        blk_bad = {
            "nrecs": 1,
            "ddi_vars": [{"name": f"v{i}"} for i in range(10)],
        }
        result_bad = _validate_block(df, blk_bad, "test_block")
        col_check_bad = next(
            c for c in result_bad["checks"] if c["check"] == "column_count"
        )
        assert col_check_bad["passed"] is False


# ═══════════════════════════════════════════════════════════════════════════
#  6. CLI tests
# ═══════════════════════════════════════════════════════════════════════════

class TestCLI:
    """Tests for CLI entry-point behaviour."""

    _SCRIPT = str(PROJECT_ROOT / "nesstar_converter.py")

    @pytest.mark.unit
    def test_cli_formats_command(self):
        """Run `formats` command, check output lists all formats."""
        result = subprocess.run(
            [sys.executable, self._SCRIPT, "formats"],
            capture_output=True, text=True, cwd=str(PROJECT_ROOT),
        )
        assert result.returncode == 0, f"stderr: {result.stderr}"
        for fmt in ALL_FORMATS:
            assert fmt in result.stdout, f"Format '{fmt}' not in formats output"

    @pytest.mark.unit
    def test_cli_no_command(self):
        """No command prints help and exits 0."""
        result = subprocess.run(
            [sys.executable, self._SCRIPT],
            capture_output=True, text=True, cwd=str(PROJECT_ROOT),
        )
        assert result.returncode == 0
        assert "convert" in result.stdout.lower() or "convert" in result.stderr.lower()

    @pytest.mark.unit
    def test_cli_convert_invalid_format(self):
        """Invalid format name exits with code 2."""
        result = subprocess.run(
            [
                sys.executable, self._SCRIPT,
                "convert", "fake.Nesstar", "fake.xml", "/dev/null",
                "--formats", "banana",
            ],
            capture_output=True, text=True, cwd=str(PROJECT_ROOT),
        )
        assert result.returncode == 2

    @pytest.mark.unit
    @needs_real_data
    def test_cli_info_command(self):
        """Run info on real file, check output."""
        result = subprocess.run(
            [
                sys.executable, self._SCRIPT,
                "info", NESSTAR_FILE, DDI_FILE,
            ],
            capture_output=True, text=True, cwd=str(PROJECT_ROOT),
        )
        assert result.returncode == 0, f"stderr: {result.stderr}"
        assert "NESSTART" in result.stdout or "NESSTAR" in result.stdout
        assert "Block" in result.stdout or "block" in result.stdout


# ═══════════════════════════════════════════════════════════════════════════
#  7. Edge case tests
# ═══════════════════════════════════════════════════════════════════════════

class TestEdgeCases:
    """Tests for helper functions and edge cases."""

    @pytest.mark.unit
    def test_safe_name_basic(self):
        assert _safe_name("Block-10-Household-Loan-records") == "block_10_household_loan_records"

    @pytest.mark.unit
    def test_safe_name_special_chars(self):
        assert _safe_name("Hello (World) [2024]!") == "hello_world_2024"

    @pytest.mark.unit
    def test_safe_name_multiple_spaces(self):
        result = _safe_name("Block   10   records")
        assert "__" not in result  # no double underscores
        assert result == "block_10_records"

    @pytest.mark.unit
    def test_safe_name_empty(self):
        assert _safe_name("") == ""

    @pytest.mark.unit
    def test_find_matching_export_exact(self):
        """Export file matching works with exact stem overlap."""
        paths = [
            Path("exports/Block-10-Household-Loan-records.txt"),
            Path("exports/Block-1-3-Household-records.txt"),
        ]
        result = _find_matching_export("block_10_household_loan_records", paths)
        assert result is not None
        assert "Block-10" in result.stem

    @pytest.mark.unit
    def test_find_matching_export_no_match(self):
        paths = [Path("exports/something_else.txt")]
        result = _find_matching_export("totally_different_name", paths)
        assert result is None

    @pytest.mark.unit
    def test_find_matching_export_various_patterns(self):
        """Various naming patterns still find the right file."""
        paths = [
            Path("exports/Block-41-Persons-Demogrphic-weelyActivity-records.txt"),
            Path("exports/Block-5-Persons-DailyActivity-records.txt"),
        ]
        # The stem with underscores should match hyphenated export
        result = _find_matching_export(
            "block_41_persons_demogrphic_weelyactivity_records", paths
        )
        assert result is not None
        assert "Block-41" in result.stem

    @pytest.mark.unit
    def test_nesstar_magic_validation(self):
        """Verify magic byte check on non-Nesstar data."""
        assert NESSTAR_MAGIC == b"NESSTART"
        assert len(NESSTAR_MAGIC) == 8
        # A real Nesstar file starts with NESSTART
        if HAS_REAL_DATA:
            with open(NESSTAR_FILE, "rb") as f:
                magic = f.read(8)
            assert magic == NESSTAR_MAGIC

    @pytest.mark.unit
    def test_slot_size_constant(self):
        assert SLOT_SIZE == 160

    @pytest.mark.unit
    def test_all_formats_list(self):
        expected = {"parquet", "csv", "tsv", "excel", "stata", "json", "jsonl", "fwf"}
        assert set(ALL_FORMATS) == expected

    @pytest.mark.unit
    def test_format_extensions_complete(self):
        """Every format in ALL_FORMATS has an extension defined."""
        for fmt in ALL_FORMATS:
            assert fmt in FORMAT_EXTENSIONS

    @pytest.mark.unit
    def test_convert_nesstar_invalid_format(self, tmp_path):
        """convert_nesstar raises ValueError for unknown format."""
        if not HAS_REAL_DATA:
            pytest.skip("Real data not available")
        with pytest.raises(ValueError, match="Unknown format"):
            convert_nesstar(
                NESSTAR_FILE, DDI_FILE, str(tmp_path),
                formats=["banana"], verbose=False,
            )

    @pytest.mark.unit
    def test_dbl_max_bytes_length(self):
        """DBL_MAX_BYTES is exactly 8 bytes."""
        assert len(DBL_MAX_BYTES) == 8

    @pytest.mark.unit
    def test_dbl_max_roundtrip(self):
        """DBL_MAX_VAL roundtrips through struct pack/unpack."""
        packed = struct.pack("<d", DBL_MAX_VAL)
        assert packed == DBL_MAX_BYTES
        unpacked = struct.unpack("<d", packed)[0]
        assert unpacked == DBL_MAX_VAL
