#!/usr/bin/env python3
"""
Nesstar DDI → Multi-Format Converter

Converts proprietary Nesstar binary files (used by India's MOSPI/NSS surveys)
into researcher-friendly formats: CSV, Excel, Stata (.dta), JSON, Parquet,
tab-delimited text, and fixed-width text.

The Nesstar format is reverse-engineered from binary analysis with 100%
validation against official Nesstar Explorer text exports across multiple
NSS survey rounds (EUS 38th, 55th, 60th; HCES 38th–68th).

Quick start:
    python tools/nesstar_converter.py info  myfile.Nesstar ddi.xml
    python tools/nesstar_converter.py convert myfile.Nesstar ddi.xml ./output --formats csv,excel
    python tools/nesstar_converter.py convert myfile.Nesstar ddi.xml ./output --formats all
    python tools/nesstar_converter.py validate ./output ./text_exports/
    python tools/nesstar_converter.py batch --survey hces --formats parquet,csv

Supported output formats:
    parquet   Apache Parquet (columnar, compressed — best for analysis)
    csv       Comma-separated values (universal)
    tsv       Tab-separated values
    excel     Microsoft Excel (.xlsx)
    stata     Stata data file (.dta) — for Stata users
    json      JSON (records orientation)
    jsonl     JSON Lines (one JSON object per line — streaming-friendly)
    fwf       Fixed-width format text (preserves original column widths)

Requirements: pandas, pyarrow, numpy, tqdm, openpyxl (for Excel)
"""

import argparse
import json
import math
import mmap
import os
import re
import struct
import sys
import textwrap
import time
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

import numpy as np
import pandas as pd

try:
    from tqdm import tqdm
    HAS_TQDM = True
except ImportError:
    HAS_TQDM = False
    def tqdm(iterable, **kwargs):
        return iterable


# ═══════════════════════════════════════════════════════════════════════════
#  Constants
# ═══════════════════════════════════════════════════════════════════════════

SLOT_SIZE = 160
DBL_MAX_BYTES = struct.pack('<d', 1.7976931348623157e+308)
DBL_MAX_VAL   = 1.7976931348623157e+308
NESSTAR_MAGIC = b'NESSTART'

ALL_FORMATS = ['parquet', 'csv', 'tsv', 'excel', 'stata', 'json', 'jsonl', 'fwf']

FORMAT_EXTENSIONS = {
    'parquet': '.parquet',
    'csv':     '.csv',
    'tsv':     '.tsv',
    'excel':   '.xlsx',
    'stata':   '.dta',
    'json':    '.json',
    'jsonl':   '.jsonl',
    'fwf':     '.txt',
}

FORMAT_DESCRIPTIONS = {
    'parquet': 'Apache Parquet (columnar, compressed — best for large datasets)',
    'csv':     'Comma-separated values (opens in Excel, Google Sheets, R, Stata)',
    'tsv':     'Tab-separated values (traditional survey data format)',
    'excel':   'Microsoft Excel workbook (.xlsx)',
    'stata':   'Stata data file (.dta v117 — opens in Stata 13+)',
    'json':    'JSON array of records (for web/API use)',
    'jsonl':   'JSON Lines — one record per line (streaming-friendly)',
    'fwf':     'Fixed-width format text (preserves original column widths)',
}


# ═══════════════════════════════════════════════════════════════════════════
#  DDI XML Parsing
# ═══════════════════════════════════════════════════════════════════════════

def parse_ddi(ddi_path: str) -> dict:
    """Parse DDI XML and return block definitions with variable specs.

    Returns dict mapping file ID → block info with DDI variable metadata.
    """
    tree = ET.parse(ddi_path)
    root = tree.getroot()

    ns = ''
    if '}' in root.tag:
        ns = root.tag.split('}')[0].strip('{')

    def tag(name):
        return f'{{{ns}}}{name}' if ns else name

    blocks = {}
    for fd in root.iter(tag('fileDscr')):
        fid = fd.attrib.get('ID', '')
        uri = fd.attrib.get('URI', '')
        name = uri.split('Name=')[-1] if 'Name=' in uri else fid

        dims = fd.find(f'.//{tag("dimensns")}')
        nrecs = 0
        if dims is not None:
            cc = dims.find(tag('caseQnty'))
            if cc is not None and cc.text:
                nrecs = int(cc.text.strip())

        fid_num = int(re.sub(r'\D', '', fid)) if re.search(r'\d', fid) else 0

        blocks[fid] = {
            'fid': fid,
            'fid_num': fid_num,
            'name': name,
            'nrecs': nrecs,
            'ddi_vars': [],
        }

    for var in root.iter(tag('var')):
        vname = var.attrib.get('name', '')
        vfiles = var.attrib.get('files', '').split()
        loc = var.find(tag('location'))
        vf = var.find(tag('varFormat'))
        valrng = var.find(f'.//{tag("range")}')
        labl = var.find(tag('labl'))

        vtype = vf.attrib.get('type', 'character') if vf is not None else 'character'
        ddi_width = int(loc.attrib.get('width', '0')) if loc is not None else 0
        dcml = int(vf.attrib.get('dcml', '0')) if vf is not None else 0
        label = labl.text.strip() if labl is not None and labl.text else ''

        rng_min = rng_max = None
        if valrng is not None:
            try:
                rng_min = float(valrng.attrib.get('min', ''))
            except (ValueError, TypeError):
                pass
            try:
                rng_max = float(valrng.attrib.get('max', ''))
            except (ValueError, TypeError):
                pass

        for fid in vfiles:
            if fid in blocks:
                blocks[fid]['ddi_vars'].append({
                    'name': vname,
                    'label': label,
                    'type': vtype,
                    'ddi_width': ddi_width,
                    'dcml': dcml,
                    'rng_min': rng_min,
                    'rng_max': rng_max,
                })

    return blocks


# ═══════════════════════════════════════════════════════════════════════════
#  Metadata Slot Reader
# ═══════════════════════════════════════════════════════════════════════════

def read_metadata_slots(data: bytes, meta_start: int, nvars: int) -> list:
    """Read 160-byte metadata slots starting at meta_start.

    Each slot contains: var_num (uint32 @ 0), encoding flags, and
    variable name (UTF-16-LE @ byte 63).

    Returns list of dicts with: var_num, encoding, char_width, name.
    """
    slots = []
    for i in range(nvars):
        off = meta_start + i * SLOT_SIZE
        slot = data[off:off + SLOT_SIZE]
        if len(slot) < SLOT_SIZE:
            raise ValueError(f"Truncated metadata at slot {i}, offset {off}")

        var_num = struct.unpack_from('<I', slot, 0)[0]
        byte4 = slot[4]
        byte5 = slot[5]
        byte14 = slot[14]

        name_bytes = slot[63:63 + 80]
        try:
            name = name_bytes.decode('utf-16-le').split('\x00')[0]
        except UnicodeDecodeError:
            name = f"var_{var_num}"

        if byte4 == 1:
            encoding = 'char'
            char_width = byte14
        elif byte5 == 10:
            encoding = 'double'
            char_width = 0
        else:
            encoding = 'offset'
            char_width = 0

        slots.append({
            'var_num': var_num,
            'encoding': encoding,
            'char_width': char_width,
            'name': name,
            'slot_index': i,
        })

    return slots


def _count_actual_slots(data: bytes, meta_start: int, max_slots: int = 200) -> int:
    """Count consecutive valid metadata slots from meta_start."""
    count = 0
    for i in range(max_slots):
        off = meta_start + i * SLOT_SIZE
        if off + SLOT_SIZE > len(data):
            break
        vn = struct.unpack_from('<I', data, off)[0]
        if vn == 0 or vn > 50000:
            break
        try:
            nm = data[off + 63:off + 63 + 80].decode('utf-16-le').split('\x00')[0]
        except (UnicodeDecodeError, IndexError):
            break
        if not nm.strip():
            break
        count += 1
    return count


# ═══════════════════════════════════════════════════════════════════════════
#  Binary Width Computation
# ═══════════════════════════════════════════════════════════════════════════

def compute_binary_width(var_spec: dict, slot_info: dict) -> int:
    """Compute bytes per value for a variable in the binary data."""
    enc = slot_info['encoding']

    if enc == 'char':
        return slot_info['char_width']

    if enc == 'double':
        return 8

    rng_min = var_spec.get('rng_min')
    rng_max = var_spec.get('rng_max')

    if rng_min is not None and rng_max is not None:
        delta = int(rng_max) - int(rng_min)
        if delta <= 0:
            delta = 1
        return max(1, math.ceil(delta.bit_length() / 8))
    else:
        ddi_w = var_spec.get('ddi_width', 1)
        max_val = 10 ** ddi_w - 1
        return max(1, math.ceil(max_val.bit_length() / 8))


# ═══════════════════════════════════════════════════════════════════════════
#  Metadata Section Finder
# ═══════════════════════════════════════════════════════════════════════════

def find_metadata_sections(data: bytes, blocks: dict) -> dict:
    """Find metadata sections by searching for block-unique UTF-16-LE
    variable names, then walking back to first slot of each section.

    Returns dict mapping fid → metadata_start_offset.
    """
    all_var_names = {}
    for fid, blk in blocks.items():
        for v in blk['ddi_vars']:
            all_var_names.setdefault(v['name'], []).append(fid)

    metadata_map = {}
    used_pairs = set()

    for fid, blk in sorted(blocks.items(), key=lambda x: x[1]['fid_num']):
        nvars = len(blk['ddi_vars'])
        if nvars == 0:
            continue

        candidates = []
        for i, v in enumerate(blk['ddi_vars']):
            if len(all_var_names[v['name']]) == 1:
                candidates.append((v['name'], i))
        candidates.append((blk['ddi_vars'][0]['name'], 0))
        if nvars > 1:
            candidates.append((blk['ddi_vars'][-1]['name'], nvars - 1))

        found = False
        for cand_name, cand_idx in candidates:
            if found:
                break
            target = cand_name.encode('utf-16-le')

            search_start = 0
            while True:
                pos = data.find(target, search_start)
                if pos == -1:
                    break
                search_start = pos + 1

                slot_start = pos - 63
                if slot_start < 0:
                    continue

                meta_start = slot_start - cand_idx * SLOT_SIZE
                if meta_start < 0:
                    continue

                pair_key = (meta_start, nvars)
                if pair_key in used_pairs:
                    continue

                vn0 = struct.unpack_from('<I', data, meta_start)[0]
                if vn0 == 0 or vn0 > 50000:
                    continue

                try:
                    name0 = data[meta_start + 63:meta_start + 63 + 80].decode(
                        'utf-16-le').split('\x00')[0]
                except (UnicodeDecodeError, IndexError):
                    continue
                ddi_first = blk['ddi_vars'][0]['name']
                if (name0 != ddi_first and
                        not name0.startswith(ddi_first) and
                        not ddi_first.startswith(name0)):
                    continue

                actual_slots = _count_actual_slots(data, meta_start)
                last_ok = False
                ddi_last = blk['ddi_vars'][-1]['name']

                def _name_close(a, b):
                    return a == b or a.startswith(b) or b.startswith(a)

                if actual_slots >= nvars:
                    last_slot = meta_start + (nvars - 1) * SLOT_SIZE
                    try:
                        name_last = data[last_slot + 63:last_slot + 63 + 80].decode(
                            'utf-16-le').split('\x00')[0]
                        if _name_close(name_last, ddi_last):
                            last_ok = True
                    except (UnicodeDecodeError, IndexError):
                        pass

                if not last_ok and actual_slots > 0 and abs(actual_slots - nvars) <= 3:
                    last_slot_actual = meta_start + (actual_slots - 1) * SLOT_SIZE
                    try:
                        name_last_a = data[last_slot_actual + 63:last_slot_actual + 63 + 80].decode(
                            'utf-16-le').split('\x00')[0]
                        if _name_close(name_last_a, ddi_last):
                            last_ok = True
                    except (UnicodeDecodeError, IndexError):
                        pass

                if not last_ok and actual_slots == nvars:
                    last_ok = True

                if not last_ok:
                    continue

                metadata_map[fid] = meta_start
                used_pairs.add(pair_key)
                found = True
                break

    return metadata_map


# ═══════════════════════════════════════════════════════════════════════════
#  DDI ↔ Slot Matching
# ═══════════════════════════════════════════════════════════════════════════

def match_ddi_to_slots(ddi_vars: list, slots: list) -> list:
    """Match DDI variable specs to metadata slots.

    Returns merged list sorted by var_num (binary column order).
    """
    def _name_match(ddi_name, slot_name):
        if ddi_name == slot_name:
            return True
        return slot_name.startswith(ddi_name) or ddi_name.startswith(slot_name)

    if len(ddi_vars) == len(slots):
        merged = []
        for ddi_v, slot in zip(ddi_vars, slots):
            entry = {**ddi_v, **slot}
            entry['binary_width'] = compute_binary_width(ddi_v, slot)
            merged.append(entry)
    elif abs(len(ddi_vars) - len(slots)) <= 5:
        slot_by_name = {s['name']: s for s in slots}
        merged = []
        used_slots = set()
        for ddi_v in ddi_vars:
            matched_slot = None
            if ddi_v['name'] in slot_by_name and ddi_v['name'] not in used_slots:
                matched_slot = slot_by_name[ddi_v['name']]
                used_slots.add(ddi_v['name'])
            else:
                for s_name, s in slot_by_name.items():
                    if s_name not in used_slots and _name_match(ddi_v['name'], s_name):
                        matched_slot = s
                        used_slots.add(s_name)
                        break
            if matched_slot:
                entry = {**ddi_v, **matched_slot}
                entry['binary_width'] = compute_binary_width(ddi_v, matched_slot)
                merged.append(entry)
        if not merged:
            raise ValueError(
                f"DDI has {len(ddi_vars)} vars but found {len(slots)} metadata slots "
                f"and name matching found 0 overlaps"
            )
    else:
        raise ValueError(
            f"DDI has {len(ddi_vars)} vars but found {len(slots)} metadata slots"
        )

    merged.sort(key=lambda x: x['var_num'])
    return merged


# ═══════════════════════════════════════════════════════════════════════════
#  Data Extraction
# ═══════════════════════════════════════════════════════════════════════════

def extract_block(data: bytes, block_info: dict, merged_vars: list,
                  meta_start: int) -> pd.DataFrame:
    """Extract one block from the Nesstar binary as a DataFrame.

    Data is stored column-major: all values for column 0, then column 1, etc.
    Columns are read in var_num order then reordered to DDI listing order.
    """
    nrecs = block_info['nrecs']
    bpr = sum(v['binary_width'] for v in merged_vars)
    data_start = meta_start - (bpr * nrecs)

    # Audit fix B-1: Replace blind width-shrinking with cautious reduction
    # that validates reduced widths can still represent the variable range
    if data_start < 8:
        offset_vars = [(i, v) for i, v in enumerate(merged_vars)
                       if v['encoding'] == 'offset' and v['binary_width'] > 1]
        offset_vars.sort(key=lambda x: -x[1]['binary_width'])
        for idx, var in offset_vars:
            if data_start >= 8:
                break
            new_w = var['binary_width'] - 1
            # Validate: can new_w still represent (rng_max - rng_min)?
            rng_min = var.get('rng_min')
            rng_max = var.get('rng_max')
            if rng_min is not None and rng_max is not None:
                delta = int(rng_max) - int(rng_min)
                max_representable = (1 << (new_w * 8)) - 1
                if delta > max_representable:
                    continue  # Skip — shrinking would truncate values
            var['binary_width'] = new_w
            bpr -= 1
            data_start = meta_start - (bpr * nrecs)
        if data_start < 8:
            raise ValueError(
                f"Computed data_start={data_start} is invalid. "
                f"Binary width calculation may be wrong for this file."
            )

    result = {}
    col_offset = data_start

    for var in merged_vars:
        bw = var['binary_width']
        enc = var['encoding']
        name = var['name']
        total_bytes = bw * nrecs
        col_data = data[col_offset:col_offset + total_bytes]

        if len(col_data) < total_bytes:
            raise ValueError(
                f"Truncated data for {name}: got {len(col_data)}, need {total_bytes}"
            )

        if enc == 'char':
            values = _extract_char_column(col_data, bw, nrecs)
        elif enc == 'double':
            values = _extract_double_column(col_data, nrecs, var.get('dcml', 0))
        else:
            offset_min = int(var.get('rng_min', 0) or 0)
            values = _extract_offset_column(col_data, bw, nrecs, offset_min)

        result[name] = values
        col_offset += total_bytes

    df = pd.DataFrame(result)

    ddi_order = sorted(merged_vars, key=lambda x: x['slot_index'])
    ddi_col_order = [v['name'] for v in ddi_order]
    df = df[ddi_col_order]

    return df


def _extract_char_column(col_data: bytes, width: int, nrecs: int) -> list:
    """Extract character column: ASCII, fixed width, strip nulls/spaces."""
    values = []
    for i in range(nrecs):
        raw = col_data[i * width:(i + 1) * width]
        val = raw.replace(b'\x00', b' ').decode('ascii', errors='replace').strip()
        values.append(val if val else '')
    return values


def _extract_double_column(col_data: bytes, nrecs: int, dcml: int) -> list:
    """Extract double-precision column: 8-byte IEEE 754 floats."""
    arr = np.frombuffer(col_data, dtype='<f8', count=nrecs)
    values = []
    for d in arr:
        if np.isnan(d) or d >= DBL_MAX_VAL * 0.99:
            values.append('')
        else:
            if d == int(d):
                values.append(str(int(d)))
            else:
                formatted = f"{d:.{max(dcml, 6)}f}".rstrip('0').rstrip('.')
                values.append(formatted)
    return values


def _extract_offset_column(col_data: bytes, bw: int, nrecs: int,
                           offset_min: int) -> list:
    """Extract offset-encoded integer column."""
    miss_marker = b'\xff' * bw
    values = []
    for i in range(nrecs):
        raw = col_data[i * bw:(i + 1) * bw]
        if raw == miss_marker:
            values.append('')
        else:
            val = int.from_bytes(raw, 'little') + offset_min
            values.append(str(val))
    return values


# ═══════════════════════════════════════════════════════════════════════════
#  Core Converter
# ═══════════════════════════════════════════════════════════════════════════

def convert_nesstar(nesstar_path: str, ddi_path: str, output_dir: str,
                    formats: list[str] | None = None,
                    year: str = 'unknown',
                    verbose: bool = True) -> dict:
    """Convert a Nesstar binary file to one or more output formats.

    Args:
        nesstar_path: Path to .Nesstar binary file
        ddi_path: Path to companion ddi.xml
        output_dir: Directory for output files
        formats: List of output formats (default: ['parquet'])
        year: Year label for the report
        verbose: Print progress

    Returns:
        Extraction report dict with block info and validation results.
    """
    if formats is None:
        formats = ['parquet']

    # Validate formats
    for fmt in formats:
        if fmt not in ALL_FORMATS:
            raise ValueError(
                f"Unknown format '{fmt}'. Choose from: {', '.join(ALL_FORMATS)}"
            )

    if verbose:
        _print_header("NESSTAR CONVERTER")
        print(f"  Nesstar file : {nesstar_path}")
        print(f"  DDI metadata : {ddi_path}")
        print(f"  Output dir   : {output_dir}")
        print(f"  Formats      : {', '.join(formats)}")
        print()

    # Parse DDI
    if verbose:
        print("Step 1/4: Parsing DDI metadata...")
    blocks = parse_ddi(ddi_path)

    if verbose:
        total_records = sum(b['nrecs'] for b in blocks.values())
        print(f"  Found {len(blocks)} data blocks, {total_records:,} total records")
        for fid, blk in sorted(blocks.items(), key=lambda x: x[1]['fid_num']):
            print(f"    {fid:>4s}  {blk['name'][:55]:55s}  {blk['nrecs']:>10,} records  "
                  f"{len(blk['ddi_vars']):>3} vars")

    # Memory-map the binary (context managers ensure cleanup on error — audit fix B-3)
    fsize = os.path.getsize(nesstar_path)
    if fsize == 0:
        raise ValueError(f"Nesstar file is empty (0 bytes): {nesstar_path}")

    if verbose:
        print(f"\nStep 2/4: Reading Nesstar binary ({fsize / 1e6:.1f} MB)...")

    with open(nesstar_path, 'rb') as f_handle:
      with mmap.mmap(f_handle.fileno(), 0, access=mmap.ACCESS_READ) as data:

        if data[:8] != NESSTAR_MAGIC:
            raise ValueError(
                f"Not a valid Nesstar file. Expected magic 'NESSTART', "
                f"got '{data[:8].decode('ascii', errors='replace')}'"
            )

        # Find metadata sections
        if verbose:
            print("\nStep 3/4: Locating data blocks in binary...")
        meta_map = find_metadata_sections(data, blocks)

        if not meta_map:
            raise RuntimeError(
                "Could not find any data blocks in the Nesstar binary. "
                "The file may be corrupted or use an unsupported format variant."
            )

        if verbose:
            print(f"  Located {len(meta_map)}/{len(blocks)} blocks")

        os.makedirs(output_dir, exist_ok=True)
        report = {
            'year': year,
            'source_nesstar': nesstar_path,
            'source_ddi': ddi_path,
            'formats': formats,
            'blocks': {},
            'errors': [],
            'validation': {},
        }

        # Extract and convert blocks
        if verbose:
            print(f"\nStep 4/4: Extracting and converting blocks...")

        block_items = sorted(blocks.items(), key=lambda x: x[1]['fid_num'])
        progress = tqdm(block_items, desc="Blocks", disable=not verbose or not HAS_TQDM)

        for fid, blk in progress:
            block_name = _safe_name(blk['name'])

            if fid not in meta_map:
                msg = f"Skipped {fid} ({blk['name']}): metadata not found in binary"
                report['errors'].append(msg)
                if verbose and not HAS_TQDM:
                    print(f"  ⚠ {msg}")
                continue

            if HAS_TQDM:
                progress.set_postfix_str(block_name[:30])

            meta_start = meta_map[fid]
            nvars = len(blk['ddi_vars'])

            try:
                actual_nslots = _count_actual_slots(data, meta_start)
                read_count = nvars if actual_nslots >= nvars else actual_nslots
                slots = read_metadata_slots(data, meta_start, read_count)
                merged = match_ddi_to_slots(blk['ddi_vars'], slots)

                df = extract_block(data, blk, merged, meta_start)

                # Validate extracted data
                validation = _validate_block(df, blk, block_name)
                report['validation'][block_name] = validation

                # Write all requested formats
                files_written = _write_formats(df, output_dir, block_name, formats,
                                               blk, merged, verbose=(verbose and not HAS_TQDM))

                n_char = sum(1 for v in merged if v['encoding'] == 'char')
                n_offset = sum(1 for v in merged if v['encoding'] == 'offset')
                n_double = sum(1 for v in merged if v['encoding'] == 'double')

                report['blocks'][block_name] = {
                    'fid': fid,
                    'name': blk['name'],
                    'rows': len(df),
                    'columns': len(df.columns),
                    'column_names': list(df.columns),
                    'files': files_written,
                    'encoding_counts': {
                        'char': n_char, 'offset': n_offset, 'double': n_double
                    },
                    'validation': validation,
                }

            except Exception as e:
                msg = f"Error in {fid} ({blk['name']}): {e}"
                report['errors'].append(msg)
                if verbose:
                    print(f"\n  ❌ {msg}")

    # Save extraction report
    report_path = os.path.join(output_dir, 'conversion_report.json')
    with open(report_path, 'w') as f:
        json.dump(report, f, indent=2, default=str)

    # Print summary
    if verbose:
        _print_summary(report, blocks)

    return report


# ═══════════════════════════════════════════════════════════════════════════
#  Format Writers
# ═══════════════════════════════════════════════════════════════════════════

def _write_formats(df: pd.DataFrame, output_dir: str, block_name: str,
                   formats: list[str], blk: dict, merged_vars: list,
                   verbose: bool = False) -> dict:
    """Write a DataFrame to all requested formats. Returns dict of format→path."""
    files = {}

    for fmt in formats:
        ext = FORMAT_EXTENSIONS[fmt]
        out_path = os.path.join(output_dir, f"{block_name}{ext}")

        try:
            if fmt == 'parquet':
                df.to_parquet(out_path, index=False, compression='snappy')

            elif fmt == 'csv':
                df.to_csv(out_path, index=False, encoding='utf-8')

            elif fmt == 'tsv':
                df.to_csv(out_path, index=False, sep='\t', encoding='utf-8')

            elif fmt == 'excel':
                _write_excel(df, out_path, blk, merged_vars)

            elif fmt == 'stata':
                _write_stata(df, out_path, blk, merged_vars)

            elif fmt == 'json':
                df.to_json(out_path, orient='records', indent=2, force_ascii=False)

            elif fmt == 'jsonl':
                with open(out_path, 'w', encoding='utf-8') as jf:
                    cols = list(df.columns)
                    for vals in df.values:
                        jf.write(json.dumps(dict(zip(cols, (str(v) if not isinstance(v, str) else v for v in vals)), ), default=str) + '\n')

            elif fmt == 'fwf':
                _write_fwf(df, out_path, blk, merged_vars)

            size_mb = os.path.getsize(out_path) / (1024 * 1024)
            files[fmt] = {'path': out_path, 'size_mb': round(size_mb, 2)}

            if verbose:
                print(f"    ✓ {fmt:8s} → {os.path.basename(out_path)} ({size_mb:.1f} MB)")

        except Exception as e:
            files[fmt] = {'path': out_path, 'error': str(e)}
            if verbose:
                print(f"    ✗ {fmt:8s} → FAILED: {e}")

    return files


def _write_excel(df: pd.DataFrame, out_path: str, blk: dict,
                 merged_vars: list):
    """Write Excel with variable labels as a second header row."""
    # Build label map
    label_map = {}
    for v in blk.get('ddi_vars', []):
        if v.get('label'):
            label_map[v['name']] = v['label']

    with pd.ExcelWriter(out_path, engine='openpyxl') as writer:
        # Data sheet
        df.to_excel(writer, sheet_name='Data', index=False, startrow=1)
        ws = writer.sheets['Data']
        # Write labels in row 1
        for col_idx, col_name in enumerate(df.columns, 1):
            ws.cell(row=1, column=col_idx, value=label_map.get(col_name, col_name))

        # Metadata sheet
        meta_rows = []
        for v in merged_vars:
            meta_rows.append({
                'Variable': v['name'],
                'Label': v.get('label', ''),
                'Type': v.get('type', ''),
                'Width': v.get('ddi_width', ''),
                'Decimals': v.get('dcml', ''),
                'Encoding': v.get('encoding', ''),
                'Min': v.get('rng_min', ''),
                'Max': v.get('rng_max', ''),
            })
        pd.DataFrame(meta_rows).to_excel(writer, sheet_name='Variables', index=False)


def _write_stata(df: pd.DataFrame, out_path: str, blk: dict,
                 merged_vars: list):
    """Write Stata .dta with variable labels."""
    label_map = {}
    for v in blk.get('ddi_vars', []):
        if v.get('label'):
            label_map[v['name']] = v['label'][:80]  # Stata label limit

    # Keep all columns as strings to preserve leading zeros and exact values.
    # Survey codes like state="03" must not become 3 or 3.0.
    # Users can convert to numeric in Stata with: destring varname, replace
    df_stata = df.copy()
    for col in df_stata.columns:
        df_stata[col] = df_stata[col].astype(str).replace('nan', '')

    # Ensure column names are valid for Stata (max 32 chars, no special chars)
    col_renames = {}
    for col in df_stata.columns:
        clean = re.sub(r'[^a-zA-Z0-9_]', '_', col)[:32]
        if clean != col:
            col_renames[col] = clean
    if col_renames:
        df_stata = df_stata.rename(columns=col_renames)
        label_map = {col_renames.get(k, k): v for k, v in label_map.items()}

    df_stata.to_stata(out_path, write_index=False,
                      variable_labels=label_map,
                      version=117)


def _write_fwf(df: pd.DataFrame, out_path: str, blk: dict,
               merged_vars: list):
    """Write fixed-width format preserving DDI column widths."""
    # Get widths from DDI — add 1 to guarantee at least one space separator
    # between columns (prevents adjacent columns from merging when data fills
    # the DDI width exactly, e.g. an 11-char key in an 11-wide column).
    widths = {}
    for v in merged_vars:
        widths[v['name']] = max(v.get('ddi_width', 10), len(v['name']) + 1) + 1

    with open(out_path, 'w', encoding='utf-8') as f:
        # Header line
        header_parts = []
        for col in df.columns:
            w = widths.get(col, 12)
            header_parts.append(col.ljust(w))
        f.write(''.join(header_parts).rstrip() + '\n')

        # Data lines
        for _, row in df.iterrows():
            parts = []
            for col in df.columns:
                w = widths.get(col, 12)
                val = str(row[col]) if pd.notna(row[col]) else ''
                parts.append(val.ljust(w))
            f.write(''.join(parts).rstrip() + '\n')


# ═══════════════════════════════════════════════════════════════════════════
#  Validation
# ═══════════════════════════════════════════════════════════════════════════

def _validate_block(df: pd.DataFrame, blk: dict, block_name: str) -> dict:
    """Run internal validation checks on an extracted block."""
    checks = {
        'block_name': block_name,
        'passed': True,
        'checks': [],
    }

    # 1. Row count matches DDI
    expected = blk['nrecs']
    actual = len(df)
    ok = actual == expected
    checks['checks'].append({
        'check': 'row_count',
        'expected': expected,
        'actual': actual,
        'passed': ok,
    })
    if not ok:
        checks['passed'] = False

    # 2. Column count matches DDI variables
    expected_cols = len(blk['ddi_vars'])
    actual_cols = len(df.columns)
    ok = actual_cols == expected_cols or abs(actual_cols - expected_cols) <= 3
    checks['checks'].append({
        'check': 'column_count',
        'expected': expected_cols,
        'actual': actual_cols,
        'passed': ok,
    })
    if not ok:
        checks['passed'] = False

    # 3. No all-null columns (data was actually read)
    all_null_cols = [col for col in df.columns
                     if df[col].replace('', pd.NA).isna().all()]
    ok = len(all_null_cols) == 0
    checks['checks'].append({
        'check': 'no_all_null_columns',
        'all_null_columns': all_null_cols,
        'passed': ok,
    })

    # 4. No all-identical columns (suspicious)
    identical_cols = [col for col in df.columns if df[col].nunique() <= 1 and len(df) > 10]
    checks['checks'].append({
        'check': 'column_variance',
        'identical_columns': identical_cols,
        'note': 'informational — some columns may legitimately have one value',
    })

    # 5. Column names match DDI
    ddi_names = [v['name'] for v in blk['ddi_vars']]
    actual_names = list(df.columns)
    name_match = actual_names == ddi_names[:len(actual_names)]
    checks['checks'].append({
        'check': 'column_names_match_ddi',
        'passed': name_match,
    })
    if not name_match:
        checks['passed'] = False

    return checks


def validate_against_export(parquet_dir: str, export_dir: str,
                            verbose: bool = True) -> dict:
    """Validate converted files against Nesstar Explorer text exports.

    Compares each parquet file in parquet_dir against matching .txt
    files in export_dir using multiset (Counter) comparison.

    Returns validation results dict.
    """
    pq_dir = Path(parquet_dir)
    exp_dir = Path(export_dir)

    pq_files = sorted(pq_dir.glob('*.parquet'))
    exp_files = sorted(exp_dir.glob('*.txt'))

    if verbose:
        _print_header("VALIDATION")
        print(f"  Parquet dir : {parquet_dir}  ({len(pq_files)} files)")
        print(f"  Export dir  : {export_dir}  ({len(exp_files)} files)")
        print()

    if not pq_files:
        print("  ⚠ No parquet files found!")
        return {'error': 'No parquet files'}
    if not exp_files:
        print("  ⚠ No text export files found!")
        return {'error': 'No export files'}

    results = {'total': 0, 'passed': 0, 'failed': 0, 'skipped': 0, 'blocks': {}}

    for pq_path in pq_files:
        stem = pq_path.stem
        results['total'] += 1

        # Find matching export file
        matched_exp = _find_matching_export(stem, exp_files)

        if not matched_exp:
            results['skipped'] += 1
            if verbose:
                print(f"  ⚠ {stem}: no matching export found")
            continue

        if verbose:
            print(f"  {stem} ↔ {matched_exp.name}")

        # Load both
        df_pq = pd.read_parquet(pq_path)
        df_exp = pd.read_csv(matched_exp, sep='\t', dtype=str,
                             keep_default_na=False, header=None)

        # Normalize
        for col in df_pq.columns:
            df_pq[col] = df_pq[col].astype(str).str.strip()
        for col in df_exp.columns:
            df_exp[col] = df_exp[col].astype(str).str.strip()

        block_result = {
            'parquet': str(pq_path),
            'export': str(matched_exp),
            'checks': [],
        }

        # Check column count
        col_ok = len(df_pq.columns) == len(df_exp.columns)
        block_result['checks'].append({
            'check': 'column_count',
            'parquet': len(df_pq.columns),
            'export': len(df_exp.columns),
            'passed': col_ok,
        })

        # Check row count
        row_ok = len(df_pq) == len(df_exp)
        block_result['checks'].append({
            'check': 'row_count',
            'parquet': len(df_pq),
            'export': len(df_exp),
            'passed': row_ok,
        })

        # Value comparison (multiset)
        if col_ok and row_ok:
            pq_tuples = Counter(tuple(row) for row in df_pq.values)
            exp_tuples = Counter(tuple(row) for row in df_exp.values)
            value_ok = pq_tuples == exp_tuples

            if not value_ok:
                diff_count = sum((pq_tuples - exp_tuples).values())
                block_result['checks'].append({
                    'check': 'value_match',
                    'passed': False,
                    'differing_rows': diff_count,
                })
            else:
                block_result['checks'].append({
                    'check': 'value_match',
                    'passed': True,
                })
        else:
            block_result['checks'].append({
                'check': 'value_match',
                'passed': False,
                'skipped': True,
                'reason': 'column or row count mismatch',
            })

        all_passed = all(c.get('passed', False) for c in block_result['checks'])
        block_result['passed'] = all_passed

        if all_passed:
            results['passed'] += 1
            if verbose:
                print(f"    ✅ MATCH: {len(df_pq):,} rows × {len(df_pq.columns)} cols")
        else:
            results['failed'] += 1
            if verbose:
                for c in block_result['checks']:
                    if not c.get('passed', True):
                        print(f"    ❌ {c['check']}: {c}")

        results['blocks'][stem] = block_result

    if verbose:
        print(f"\n  Summary: {results['passed']}/{results['total']} blocks passed "
              f"({results['failed']} failed, {results['skipped']} skipped)")

    return results


def _find_matching_export(parquet_stem: str, exp_files: list) -> Path | None:
    """Find the best-matching export file for a parquet stem."""
    pq_clean = parquet_stem.lower().replace('_', '')
    best_match = None
    best_score = 0

    for exp_path in exp_files:
        exp_clean = exp_path.stem.lower().replace('-', '').replace('_', '').replace(' ', '')
        # Score based on overlap
        if pq_clean in exp_clean or exp_clean in pq_clean:
            score = min(len(pq_clean), len(exp_clean))
            if score > best_score:
                best_score = score
                best_match = exp_path

    return best_match


# ═══════════════════════════════════════════════════════════════════════════
#  Info Command (inspect without converting)
# ═══════════════════════════════════════════════════════════════════════════

def show_info(nesstar_path: str, ddi_path: str):
    """Show information about a Nesstar file without converting."""
    _print_header("NESSTAR FILE INFO")

    # File details
    nes_size = os.path.getsize(nesstar_path)
    print(f"  Nesstar file : {nesstar_path}")
    print(f"  File size    : {nes_size / 1e6:.1f} MB ({nes_size:,} bytes)")

    # Quick binary check
    with open(nesstar_path, 'rb') as f:
        magic = f.read(8)
    if magic != NESSTAR_MAGIC:
        print(f"  ⚠ WARNING: Not a valid Nesstar file (magic: {magic})")
        return

    print(f"  Magic        : NESSTART ✓")

    # DDI metadata
    print(f"\n  DDI file     : {ddi_path}")
    blocks = parse_ddi(ddi_path)
    total_records = sum(b['nrecs'] for b in blocks.values())
    total_vars = sum(len(b['ddi_vars']) for b in blocks.values())

    print(f"  Blocks       : {len(blocks)}")
    print(f"  Total records: {total_records:,}")
    print(f"  Total vars   : {total_vars}")
    print()

    print("  Block Details:")
    print(f"  {'ID':>4s}  {'Block Name':<55s}  {'Records':>10s}  {'Vars':>4s}  {'Types'}")
    print(f"  {'─'*4}  {'─'*55}  {'─'*10}  {'─'*4}  {'─'*30}")

    for fid, blk in sorted(blocks.items(), key=lambda x: x[1]['fid_num']):
        types = {}
        for v in blk['ddi_vars']:
            t = v.get('type', 'unknown')
            types[t] = types.get(t, 0) + 1
        type_str = ', '.join(f"{v} {k}" for k, v in sorted(types.items()))
        print(f"  {fid:>4s}  {blk['name'][:55]:<55s}  {blk['nrecs']:>10,}  "
              f"{len(blk['ddi_vars']):>4}  {type_str}")

    print()
    print("  Variable Sample (first block, first 10 vars):")
    first_blk = sorted(blocks.values(), key=lambda b: b['fid_num'])[0]
    for v in first_blk['ddi_vars'][:10]:
        rng = ''
        if v['rng_min'] is not None and v['rng_max'] is not None:
            rng = f"  [{v['rng_min']}, {v['rng_max']}]"
        print(f"    {v['name']:20s}  {v['type']:10s}  w={v['ddi_width']:>3}  "
              f"d={v['dcml']}  {v['label'][:40]}{rng}")


# ═══════════════════════════════════════════════════════════════════════════
#  Batch Converter
# ═══════════════════════════════════════════════════════════════════════════

def batch_convert(survey: str = 'hces', formats: list[str] | None = None):
    """Find and convert all Nesstar files for a survey."""
    if formats is None:
        formats = ['parquet']

    project_root = Path(__file__).resolve().parent.parent
    survey_dir = project_root / 'data' / survey

    if not survey_dir.exists():
        print(f"Survey directory not found: {survey_dir}")
        sys.exit(1)

    pairs = []
    for nesstar_path in sorted(survey_dir.rglob('*.Nesstar')):
        if nesstar_path.stat().st_size == 0:
            continue
        ddi_path = nesstar_path.parent / 'ddi.xml'
        if ddi_path.exists():
            rel = nesstar_path.relative_to(survey_dir)
            year = str(rel.parts[0])
            pairs.append((str(nesstar_path), str(ddi_path), year))

    _print_header(f"BATCH CONVERT: {survey.upper()}")
    print(f"  Found {len(pairs)} Nesstar files\n")

    for nes, ddi, year in pairs:
        size_mb = os.path.getsize(nes) / 1e6
        print(f"  {year:12s}  {size_mb:8.1f} MB  {Path(nes).name}")

    parquet_base = project_root / 'data' / 'parquet' / survey
    results = []

    for nes, ddi, year in pairs:
        out_dir = str(parquet_base / year)
        try:
            report = convert_nesstar(nes, ddi, out_dir, formats=formats, year=year)
            results.append((year, 'OK', report))
        except Exception as e:
            print(f"FAILED: {e}")
            import traceback
            traceback.print_exc()
            results.append((year, 'FAIL', str(e)))

    _print_header("BATCH SUMMARY")
    for year, status, info in results:
        if status == 'OK':
            nblocks = len(info['blocks'])
            total = sum(b['rows'] for b in info['blocks'].values())
            nerrs = len(info['errors'])
            err_str = f"  ({nerrs} errors)" if nerrs else ""
            print(f"  {year:12s}  ✅  {nblocks} blocks, {total:>10,} rows{err_str}")
        else:
            print(f"  {year:12s}  ❌  {info}")


# ═══════════════════════════════════════════════════════════════════════════
#  Helpers
# ═══════════════════════════════════════════════════════════════════════════

def _safe_name(name: str) -> str:
    """Convert block name to safe filesystem name."""
    safe = re.sub(r'[^a-zA-Z0-9]+', '_', name).strip('_').lower()
    return re.sub(r'_+', '_', safe)


def _auto_detect_ddi(nesstar_path: str) -> str:
    """Find ddi.xml in the same directory as the Nesstar file."""
    nes_dir = Path(nesstar_path).parent
    ddi = nes_dir / 'ddi.xml'
    if ddi.exists():
        return str(ddi)
    # Try case-insensitive
    for f in nes_dir.iterdir():
        if f.name.lower() == 'ddi.xml':
            return str(f)
    raise FileNotFoundError(
        f"Could not find ddi.xml in {nes_dir}.\n"
        f"Please specify the DDI file path explicitly."
    )


def _print_header(title: str):
    """Print a formatted section header."""
    width = 70
    print()
    print(f"{'═' * width}")
    print(f"  {title}")
    print(f"{'═' * width}")


def _print_summary(report: dict, blocks: dict):
    """Print conversion summary."""
    total_blocks = len(report['blocks'])
    total_rows = sum(b['rows'] for b in report['blocks'].values())
    nerrs = len(report['errors'])

    _print_header("CONVERSION COMPLETE")
    print(f"  Blocks extracted : {total_blocks}/{len(blocks)}")
    print(f"  Total rows       : {total_rows:,}")
    print(f"  Formats          : {', '.join(report['formats'])}")

    if report['errors']:
        print(f"\n  ⚠ Errors ({nerrs}):")
        for err in report['errors']:
            print(f"    - {err}")

    # Validation summary
    n_valid = sum(1 for v in report['validation'].values() if v.get('passed', False))
    n_total = len(report['validation'])
    if n_total > 0:
        status = "✅" if n_valid == n_total else "⚠"
        print(f"\n  Validation: {status} {n_valid}/{n_total} blocks passed all checks")

    print()


# ═══════════════════════════════════════════════════════════════════════════
#  CLI
# ═══════════════════════════════════════════════════════════════════════════

def main():
    parser = argparse.ArgumentParser(
        prog='nesstar_converter',
        description=textwrap.dedent("""\
            Convert Nesstar binary files to researcher-friendly formats.

            Nesstar is a proprietary format used by India's MOSPI (Ministry of
            Statistics) for NSS survey microdata. This tool reads Nesstar binaries
            using the companion DDI XML metadata and outputs data in your choice
            of format.

            Supported formats: parquet, csv, tsv, excel, stata, json, jsonl, fwf
        """),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=textwrap.dedent("""\
            Examples:
              # See what's in a Nesstar file (ddi.xml auto-detected)
              %(prog)s info myfile.Nesstar

              # Convert to CSV (easiest — opens in Excel/Google Sheets)
              %(prog)s convert myfile.Nesstar ./output --formats csv

              # Convert to multiple formats at once
              %(prog)s convert myfile.Nesstar ./output --formats csv,excel,stata

              # Convert to all supported formats
              %(prog)s convert myfile.Nesstar ./output --formats all

              # Validate against Nesstar Explorer text exports
              %(prog)s validate ./parquet_dir ./export_dir

              # Batch convert all HCES rounds
              %(prog)s batch --survey hces --formats parquet,csv

            The ddi.xml file is auto-detected from the same directory as the
            Nesstar file. You can specify it explicitly if needed.
        """),
    )

    sub = parser.add_subparsers(dest='command', help='Command to run')

    # --- info ---
    info_p = sub.add_parser(
        'info',
        help='Show file contents without converting',
        description='Inspect a Nesstar file: shows blocks, variables, and metadata.',
    )
    info_p.add_argument('nesstar_file', help='Path to .Nesstar binary file')
    info_p.add_argument('ddi_file', nargs='?', default=None,
                        help='Path to companion ddi.xml (auto-detected if omitted)')

    # --- convert ---
    conv_p = sub.add_parser(
        'convert',
        help='Convert a Nesstar file to other formats',
        description='Convert a single Nesstar binary file to one or more formats.',
    )
    conv_p.add_argument('nesstar_file', help='Path to .Nesstar binary file')
    conv_p.add_argument('ddi_file', nargs='?', default=None,
                        help='Path to companion ddi.xml (auto-detected if omitted)')
    conv_p.add_argument('output_dir', help='Directory for output files')
    conv_p.add_argument(
        '--formats', '-f',
        default='parquet',
        help=f'Comma-separated output formats or "all" (default: parquet). '
             f'Options: {", ".join(ALL_FORMATS)}',
    )
    conv_p.add_argument('--year', default='unknown', help='Year label for report')
    conv_p.add_argument('--quiet', '-q', action='store_true', help='Suppress progress output')

    # --- validate ---
    val_p = sub.add_parser(
        'validate',
        help='Validate against Nesstar Explorer text exports',
        description='Compare converted files against official text exports for accuracy.',
    )
    val_p.add_argument('parquet_dir', help='Directory with converted .parquet files')
    val_p.add_argument('export_dir', help='Directory with .txt export files')

    # --- batch ---
    batch_p = sub.add_parser(
        'batch',
        help='Convert all Nesstar files for a survey',
        description='Find and convert all Nesstar+DDI pairs in a survey directory.',
    )
    batch_p.add_argument('--survey', '-s', default='hces',
                         help='Survey name (directory under data/)')
    batch_p.add_argument(
        '--formats', '-f',
        default='parquet',
        help=f'Comma-separated output formats (default: parquet)',
    )

    # --- formats ---
    sub.add_parser(
        'formats',
        help='List all supported output formats',
    )

    args = parser.parse_args()

    if args.command == 'info':
        if not os.path.isfile(args.nesstar_file):
            print(f"Error: Nesstar file not found: {args.nesstar_file}")
            sys.exit(2)
        ddi = args.ddi_file or _auto_detect_ddi(args.nesstar_file)
        if not os.path.isfile(ddi):
            print(f"Error: DDI file not found: {ddi}")
            sys.exit(2)
        show_info(args.nesstar_file, ddi)

    elif args.command == 'convert':
        if not os.path.isfile(args.nesstar_file):
            print(f"Error: Nesstar file not found: {args.nesstar_file}")
            sys.exit(2)
        ddi = args.ddi_file or _auto_detect_ddi(args.nesstar_file)
        if not os.path.isfile(ddi):
            print(f"Error: DDI file not found: {ddi}")
            sys.exit(2)
        fmt_list = ALL_FORMATS if args.formats == 'all' else [
            f.strip().lower() for f in args.formats.split(',')
        ]
        # Validate format names
        invalid = [f for f in fmt_list if f not in ALL_FORMATS]
        if invalid:
            print(f"Error: unknown format(s): {', '.join(invalid)}")
            print(f"Supported: {', '.join(ALL_FORMATS)}")
            sys.exit(2)
        report = convert_nesstar(
            args.nesstar_file, ddi, args.output_dir,
            formats=fmt_list, year=args.year, verbose=not args.quiet,
        )
        if report.get('errors'):
            sys.exit(1)

    elif args.command == 'validate':
        results = validate_against_export(args.parquet_dir, args.export_dir)
        if results.get('failed', 0) > 0:
            sys.exit(1)

    elif args.command == 'batch':
        fmt_list = ALL_FORMATS if args.formats == 'all' else [
            f.strip().lower() for f in args.formats.split(',')
        ]
        batch_convert(args.survey, formats=fmt_list)

    elif args.command == 'formats':
        _print_header("SUPPORTED OUTPUT FORMATS")
        for fmt, desc in FORMAT_DESCRIPTIONS.items():
            print(f"  {fmt:10s}  {FORMAT_EXTENSIONS[fmt]:8s}  {desc}")
        print()

    else:
        parser.print_help()
        print("\nTip: Use 'info' to inspect a file, 'convert' to convert it.")
        sys.exit(0)


if __name__ == '__main__':
    main()
