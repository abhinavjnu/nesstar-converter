#!/usr/bin/env python3
"""Create the redistributable fixture contract used by the Rust migration.

The expected tables are intentionally decoded through the Python converter,
which remains the migration oracle.  Run with ``--check`` in CI to verify that
the committed fixture bytes and manifest are reproducible.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import shutil
import struct
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from nesstar_converter import (  # noqa: E402
    DATASET_COUNT_FIELD,
    DESCRIPTOR_RECORD_SIZE_FIELD,
    DESCRIPTOR_TABLE_RECORD_ID_FIELD,
    NESSTAR_MAGIC,
    RESOURCE_INDEX_OFFSET_FIELD,
    SLOT_SIZE,
    _parse_resource_layouts,
    extract_block,
    extract_block_resource_indexed,
    find_metadata_sections,
    match_ddi_to_slots,
    parse_ddi,
    read_metadata_slots,
)


def _slot(var_num: int, name: str, *, kind: str, width: int = 0) -> bytes:
    slot = bytearray(SLOT_SIZE)
    struct.pack_into("<I", slot, 0, var_num)
    if kind == "char":
        slot[4] = 1
        slot[14] = width
    elif kind == "double":
        slot[5] = 10
    slot[63 : 63 + min(80, len(name.encode("utf-16-le")))] = name.encode("utf-16-le")[:80]
    return bytes(slot)


def _ddi(block_id: str, name: str, rows: int, variables: list[dict], *, namespaced: bool) -> bytes:
    ns = ' xmlns="http://www.icpsr.umich.edu/DDI"' if namespaced else ""
    variables_xml = "\n".join(
        "  <var name=\"{name}\" files=\"{block}\"><location width=\"{width}\"/>"
        "<varFormat type=\"{type}\" dcml=\"{dcml}\"/>{range}<labl>{label}</labl></var>".format(
            block=block_id,
            name=v["name"],
            width=v.get("width", 0),
            type=v["type"],
            dcml=v.get("dcml", 0),
            range=(
                f'<valrng><range min="{v["min"]}" max="{v["max"]}"/></valrng>'
                if "min" in v
                else ""
            ),
            label=v.get("label", v["name"]),
        )
        for v in variables
    )
    return (
        f"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<codeBook{ns}>\n"
        f"  <fileDscr ID=\"{block_id}\" URI=\"Name={name}\"><dimensns><caseQnty>{rows}</caseQnty>"
        f"</dimensns></fileDscr>\n{variables_xml}\n</codeBook>\n"
    ).encode()


def _metadata_fixture() -> tuple[bytes, bytes]:
    variables = [
        {"name": "ASCII", "type": "character", "width": 4, "label": "Fixed ASCII"},
        {"name": "OFFSET", "type": "numeric", "width": 3, "min": -2, "max": 300, "label": "Offset integer"},
        {"name": "FLOAT", "type": "numeric", "width": 8, "dcml": 3, "label": "Little-endian double"},
    ]
    ddi = _ddi("F1", "metadata-scan", 4, variables, namespaced=True)
    columns = b"001 \x00ABC \x00\x00\x00\x00Z9  " + b"\x00\x00\x07\x00\xff\xff\x04\x01" + struct.pack(
        "<4d", -1.0, 0.0, math.pi, math.nan
    )
    meta_start = 128
    data = bytearray(meta_start + SLOT_SIZE * 3)
    data[:8] = NESSTAR_MAGIC
    data[meta_start - len(columns) : meta_start] = columns
    for i, (name, kind, width) in enumerate((("ASCII", "char", 4), ("OFFSET", "offset", 0), ("FLOAT", "double", 0))):
        start = meta_start + i * SLOT_SIZE
        data[start : start + SLOT_SIZE] = _slot((i + 1) * 10, name, kind=kind, width=width)
    return ddi, bytes(data)


def _resource_entry(index: int, name: str, record_id: int, width: int, mode: int, fmt: int, offset: int) -> bytes:
    entry = bytearray(SLOT_SIZE)
    struct.pack_into("<I", entry, 0, index)
    entry[5] = fmt
    entry[6:14] = int(offset).to_bytes(8, "little", signed=True)
    struct.pack_into("<I", entry, 15, record_id)
    entry[63 : 63 + min(64, len(name.encode("utf-16-le")))] = name.encode("utf-16-le")[:64]
    entry[149] = width
    entry[159] = mode
    return bytes(entry)


def _resource_fixture() -> tuple[bytes, bytes]:
    variables = [
        {"name": "ASCII", "type": "character", "width": 4},
        {"name": "UTF8", "type": "character", "width": 8},
        {"name": "NIBBLE", "type": "numeric", "width": 1, "min": -1, "max": 14},
        {"name": "U8", "type": "numeric", "width": 3},
        {"name": "U16", "type": "numeric", "width": 5},
        {"name": "U24", "type": "numeric", "width": 7},
        {"name": "U32", "type": "numeric", "width": 10},
        {"name": "U40", "type": "numeric", "width": 12},
        {"name": "CDOUBLE", "type": "numeric", "width": 8, "dcml": 3},
        {"name": "RAWBYTE", "type": "numeric", "width": 3},
    ]
    ddi = _ddi("F2", "resource-index", 5, variables, namespaced=False)
    payloads = [
        ("ASCII", b"A   B   \x00\x00\x00\x00C D E   ", 4, 0, 0, 0),
        ("UTF8", "café".encode() + b"\0\0\0" + b"two\0\0\0\0\0" + "東京".encode() + b"\0\0" + b"\0" * 8 + b"last\0\0\0\0", 8, 1, 0, 0),
        ("NIBBLE", bytes([0x12, 0x3F, 0x40]), 0, 5, 2, -1),
        ("U8", bytes([0, 7, 255, 10, 42]), 0, 5, 3, 100),
        ("U16", b"".join(x.to_bytes(2, "little") for x in (1, 512, 65535, 3, 9)), 0, 5, 4, 0),
        ("U24", b"".join(x.to_bytes(3, "little") for x in (1, 70000, 0xFFFFFF, 3, 9)), 0, 5, 5, 0),
        ("U32", b"".join(x.to_bytes(4, "little") for x in (1, 70000, 0xFFFFFFFF, 3, 9)), 0, 5, 6, 0),
        ("U40", b"".join(x.to_bytes(5, "little") for x in (1, 70000, 0xFFFFFFFFFF, 3, 9)), 0, 5, 7, 0),
        ("CDOUBLE", struct.pack("<5d", -1.0, 0.0, math.pi, math.nan, 1.7976931348623157e308), 0, 5, 10, 0),
        ("RAWBYTE", bytes([1, 2, 255, 4, 5]), 1, 0, 0, 0),
    ]
    descriptor_id, directory_id, first_value_id = 100, 101, 200
    descriptor_offset, directory_offset, payload_offset, index_offset = 1024, 1152, 3200, 4096
    data = bytearray(4096 + 4 + (2 + len(payloads)) * 15)
    data[:8] = NESSTAR_MAGIC
    struct.pack_into("<I", data, RESOURCE_INDEX_OFFSET_FIELD, index_offset)
    data[DATASET_COUNT_FIELD] = 1
    struct.pack_into("<H", data, DESCRIPTOR_RECORD_SIZE_FIELD, 32)
    struct.pack_into("<I", data, DESCRIPTOR_TABLE_RECORD_ID_FIELD, descriptor_id)
    struct.pack_into("<III", data, descriptor_offset, 1, len(payloads), 5)
    struct.pack_into("<H", data, descriptor_offset + 20, SLOT_SIZE)
    struct.pack_into("<I", data, descriptor_offset + 22, directory_id)
    records = [(descriptor_id, descriptor_offset, 32), (directory_id, directory_offset, len(payloads) * SLOT_SIZE)]
    cursor = payload_offset
    for i, (name, payload, width, mode, fmt, offset) in enumerate(payloads):
        record_id = first_value_id + i
        entry = _resource_entry(i + 1, name, record_id, width, mode, fmt, offset)
        data[directory_offset + i * SLOT_SIZE : directory_offset + (i + 1) * SLOT_SIZE] = entry
        data[cursor : cursor + len(payload)] = payload
        records.append((record_id, cursor, len(payload)))
        cursor += len(payload) + 8
    struct.pack_into("<I", data, index_offset, len(records))
    for i, (record_id, start, length) in enumerate(records):
        position = index_offset + 4 + i * 15
        struct.pack_into("<I", data, position, record_id)
        data[position + 4 : position + 10] = start.to_bytes(6, "little")
        struct.pack_into("<I", data, position + 10, length)
    return ddi, bytes(data)


def _rows_to_files(rows, columns: list[str]) -> tuple[bytes, bytes]:
    encoded_json = (json.dumps(rows, ensure_ascii=False, indent=2) + "\n").encode()
    output = []
    with tempfile.TemporaryFile(mode="w+", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=columns, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
        stream.seek(0)
        output.append(stream.read().encode())
    return encoded_json, output[0]


def _decoded_outputs(root: Path, metadata_ddi: bytes, metadata_data: bytes, resource_ddi: bytes, resource_data: bytes) -> dict[Path, bytes]:
    synthetic = root / "synthetic"
    synthetic.mkdir(parents=True, exist_ok=True)
    (synthetic / "metadata-scan.ddi.xml").write_bytes(metadata_ddi)
    (synthetic / "metadata-scan.Nesstar").write_bytes(metadata_data)
    (synthetic / "resource-index.ddi.xml").write_bytes(resource_ddi)
    (synthetic / "resource-index.Nesstar").write_bytes(resource_data)
    outputs = {}
    metadata_blocks = parse_ddi(str(synthetic / "metadata-scan.ddi.xml"))
    metadata_block = metadata_blocks["F1"]
    meta_start = find_metadata_sections(metadata_data, metadata_blocks)["F1"]
    merged = match_ddi_to_slots(metadata_block["ddi_vars"], read_metadata_slots(metadata_data, meta_start, 3))
    metadata_frame = extract_block(metadata_data, metadata_block, merged, meta_start)
    resource_blocks = parse_ddi(str(synthetic / "resource-index.ddi.xml"))
    resource_block = resource_blocks["F2"]
    resource_layout = _parse_resource_layouts(resource_data, resource_blocks)["F2"]
    resource_frame = extract_block_resource_indexed(resource_data, resource_block, resource_layout)
    for stem, frame in (("metadata-scan", metadata_frame), ("resource-index", resource_frame)):
        rows = [{column: str(row[column]) for column in frame.columns} for _, row in frame.iterrows()]
        as_json, as_tsv = _rows_to_files(rows, list(frame.columns))
        outputs[Path("expected") / f"{stem}.json"] = as_json
        outputs[Path("expected") / f"{stem}.tsv"] = as_tsv
    return outputs


def _fixture_files(root: Path) -> tuple[dict[Path, bytes], dict[str, dict]]:
    metadata_ddi, metadata_data = _metadata_fixture()
    resource_ddi, resource_data = _resource_fixture()
    outputs = _decoded_outputs(root, metadata_ddi, metadata_data, resource_ddi, resource_data)
    truncated_resource = bytearray(4098)
    truncated_resource[:8] = NESSTAR_MAGIC
    struct.pack_into("<I", truncated_resource, RESOURCE_INDEX_OFFSET_FIELD, 4096)
    malformed = {
        Path("malformed/bad-magic.Nesstar"): b"NOTSTAR!\0\0\0",
        Path("malformed/truncated-metadata.Nesstar"): metadata_data[: 128 + SLOT_SIZE - 1],
        Path("malformed/truncated-resource.Nesstar"): bytes(truncated_resource),
    }
    files = {
        Path("synthetic/metadata-scan.ddi.xml"): metadata_ddi,
        Path("synthetic/metadata-scan.Nesstar"): metadata_data,
        Path("synthetic/resource-index.ddi.xml"): resource_ddi,
        Path("synthetic/resource-index.Nesstar"): resource_data,
        **outputs,
        **malformed,
    }
    dimensions = {
        "metadata-scan": {"rows": 4, "columns": 3, "method": "metadata_scan", "encodings": ["fixed_ascii", "offset_le", "double_le"]},
        "resource-index": {"rows": 5, "columns": 10, "method": "resource_index", "encodings": ["fixed_ascii", "nul_utf8", "nibble", "uint8", "uint16", "uint24", "uint32", "uint40", "compact_double", "raw_byte_numeric"]},
    }
    return files, dimensions


def _write(root: Path) -> None:
    files, dimensions = _fixture_files(root)
    for relative, contents in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(contents)
    manifest = {
        "version": 1,
        "license": "CC0-1.0",
        "source": "Synthetic data created for converter testing; no survey records are included.",
        "fixtures": dimensions,
        "malformed": {
            "bad-magic.Nesstar": "Invalid first eight bytes.",
            "truncated-metadata.Nesstar": "Metadata slot ends one byte early.",
            "truncated-resource.Nesstar": "Resource index is truncated.",
        },
        "files": {str(path): hashlib.sha256(contents).hexdigest() for path, contents in sorted(files.items())},
    }
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="verify committed files regenerate byte-for-byte")
    parser.add_argument("--root", type=Path, default=ROOT / "fixtures")
    args = parser.parse_args()
    root = args.root.resolve()
    if not args.check:
        _write(root)
        return 0
    with tempfile.TemporaryDirectory() as temporary:
        generated = Path(temporary) / "fixtures"
        _write(generated)
        expected = {p.relative_to(generated): p.read_bytes() for p in generated.rglob("*") if p.is_file()}
    actual = {p.relative_to(root): p.read_bytes() for p in root.rglob("*") if p.is_file()} if root.exists() else {}
    if expected != actual:
        missing = sorted(str(p) for p in expected.keys() - actual.keys())
        extra = sorted(str(p) for p in actual.keys() - expected.keys())
        changed = sorted(str(p) for p in expected.keys() & actual.keys() if expected[p] != actual[p])
        print(json.dumps({"missing": missing, "extra": extra, "changed": changed}, indent=2), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
