# Nesstar Binary Format — Technical Reference

This document describes the proprietary Nesstar binary format (`.Nesstar` files)
as reverse-engineered for the `nesstar-converter` tool. There is no official
public specification; everything here was determined by binary analysis and
empirical validation against known survey datasets.

---

## 1. File Identification

Every valid Nesstar file begins with the 8-byte ASCII magic string:

```
Offset  0x00:  4E 45 53 53 54 41 52 54   →  "NESSTART"
```

If these bytes are absent, the file is not a Nesstar binary.

---

## 2. Overall Structure

A Nesstar file contains one or more **data blocks**. Each block represents a
rectangular dataset (rows × columns). The binary format is **column-major**
(columnar): all N values for column 0 are stored contiguously, then all N values
for column 1, and so on.

### Block layout (logical order in file)

```
┌──────────────────────────────────────┐
│  Data region                         │
│  bpr × nrecs bytes                   │
│  (all columns packed sequentially)   │
├──────────────────────────────────────┤
│  Metadata region                     │
│  nvars × 160-byte variable slots     │
└──────────────────────────────────────┘
```

The critical relationship:

```
data_start = meta_start - (bpr × nrecs)
```

where:

| Field        | Meaning                                                 |
|--------------|---------------------------------------------------------|
| `meta_start` | Byte offset where metadata begins                       |
| `bpr`        | Bytes per record (total width of one row across all columns) |
| `nrecs`      | Number of records (rows)                                |
| `nvars`      | Number of variables (columns)                           |

Data sits **immediately before** metadata. The converter locates metadata first,
then calculates backwards to find the data region.

---

## 3. Metadata Slot Structure (160 bytes per variable)

Each variable's metadata occupies a fixed 160-byte slot. The slots are stored
sequentially in the metadata region.

### Slot layout

| Byte offset | Size     | Field             | Description                                |
|-------------|----------|-------------------|--------------------------------------------|
| 0           | 4 bytes  | `var_num`         | Variable sequence number (uint32, LE)      |
| 4           | 1 byte   | encoding flag 1   | `== 1` → char (string) encoding           |
| 5           | 1 byte   | encoding flag 2   | `== 10` → double (float64) encoding       |
| 14          | 1 byte   | `char_width`      | Character width (meaningful for char type) |
| 63          | 80 bytes | `variable_name`   | Variable name, encoded as **UTF-16-LE**    |
| 143–159     | 17 bytes | (reserved/padding)| Unused in current implementations          |

### Encoding type decision

```
if byte[4] == 1:
    encoding = "char"
elif byte[5] == 10:
    encoding = "double"
else:
    encoding = "offset"
```

---

## 4. Data Encoding Types

### 4.1 char — Fixed-width ASCII strings

String values are stored as fixed-width ASCII byte sequences.

- **Width**: determined by `char_width` in the metadata slot.
- **Missing values**: the entire field is filled with `0x00` bytes.
- **Trailing padding**: right-padded with `0x00` or spaces.

Example (char_width = 10):

```
"MAHARASHT\x00"  →  "MAHARASHT"   (state name, trailing null stripped)
"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"  →  missing (NaN)
```

### 4.2 offset — Integer offset encoding

Numeric integer values are stored as `(value - range_min)` packed into the
minimum number of bytes needed.

- **Byte width**: `ceil((range_max - range_min).bit_length() / 8)`
  - If the entire range fits in 1 byte (max offset ≤ 255), width = 1.
  - If the range needs up to 16 bits, width = 2. And so on.
- **Byte order**: little-endian.
- **Missing values**: all bytes set to `0xFF`.

Example (range_min = 1, range_max = 99, width = 1 byte):

```
0x00  →  value = 0 + 1 = 1
0x09  →  value = 9 + 1 = 10
0x62  →  value = 98 + 1 = 99
0xFF  →  missing (NaN)
```

Example (range_min = 0, range_max = 50000, width = 2 bytes LE):

```
0x00 0x00  →  value = 0 + 0 = 0
0xE8 0x03  →  value = 1000 + 0 = 1000
0xFF 0xFF  →  missing (NaN)
```

### 4.3 double — IEEE 754 float64

Floating-point values are stored as standard 8-byte IEEE 754 doubles.

- **Byte width**: always 8 bytes.
- **Byte order**: little-endian (platform-native on x86/x64).
- **Missing values**: `DBL_MAX` (`1.7976931348623157e+308`) or `NaN`.

Example:

```
0x00 0x00 0x00 0x00 0x00 0x00 0x59 0x40  →  100.0
0xFF 0xFF 0xFF 0xFF 0xFF 0xFF 0xEF 0x7F  →  DBL_MAX → missing (NaN)
```

---

## 5. Column Ordering

The **binary metadata** defines its own column order via the `var_num` field in
each 160-byte slot. The **DDI XML** companion file may list variables in a
completely different order (typically the order they appear in the questionnaire
or codebook).

The converter uses the **binary `var_num` order** when reading data, then
reconciles with DDI labels and value-label mappings after extraction.

---

## 6. Metadata Discovery

The converter locates the metadata region by scanning the binary file for known
variable names. Variable names from the DDI companion file are encoded as
**UTF-16-LE** strings and searched for within the binary. Once a match is found,
the converter aligns to the nearest 160-byte slot boundary to locate the start
of the metadata region.

### Discovery algorithm (simplified)

1. Parse DDI XML to extract expected variable names.
2. Encode each name as UTF-16-LE.
3. Scan the Nesstar binary for any of these encoded strings.
4. Align the match position to a 160-byte boundary.
5. Read the full metadata region (nvars × 160 bytes).
6. Compute `data_start = meta_start - (bpr × nrecs)`.
7. Read data column-by-column using encoding rules from metadata.

---

## 7. Multi-Block Files

Some Nesstar exports contain multiple data blocks (e.g., person-level and
household-level records in the same file). Each block has its own independent
metadata region and data region. The converter handles these by discovering
each metadata region separately and extracting each block as an independent
dataset.

---

## 8. Companion DDI XML

The DDI (Data Documentation Initiative) XML file provides:

- Human-readable variable labels
- Value-label mappings (e.g., `1 = "Male"`, `2 = "Female"`)
- Universe descriptions and question text
- Survey metadata (title, producer, dates)

The converter merges DDI metadata with binary-extracted data to produce
fully labelled output files.
