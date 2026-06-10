---
title: Package layout
description: Imported cityjson-arrow specification page.
---

# Package file layout

This document specifies the binary format written by `PackageWriter` and read by
`PackageReader`.

The file extension is `.cityjson-parquet` by convention. Despite the name, the format
is a bespoke seekable container backed by Arrow IPC payloads, not a Parquet columnar file.

## Terminology

The key words MUST, MUST NOT, REQUIRED, SHOULD, and OPTIONAL in this document are to be
interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## Version

| Field | Value |
|---|---|
| Package magic | `CITYJSON_ARROW_PKG_V3\0` (22 bytes, including null terminator) |
| Footer magic | `CITYJSON_ARROW_PKG_V3IDX\0` (25 bytes, including null terminator) |
| Schema id | `cityjson-arrow.package.v3alpha3` |

## Overall layout

```text
PACKAGE_MAGIC             (22 bytes)
table_payload_0
table_payload_1
...
manifest_json             (UTF-8)
manifest_offset: u64 LE   (8 bytes)
manifest_length: u64 LE   (8 bytes)
PACKAGE_FOOTER_MAGIC      (25 bytes)
```

All multi-byte integers are little-endian.

## Table payloads

Each table payload is one Arrow IPC file written with `FileWriter`. Payloads MUST appear
in canonical tag order, immediately following `PACKAGE_MAGIC` and preceding `manifest_json`.

A producer MUST include all REQUIRED tables. A producer MUST NOT include any table more
than once. See [Package schema](package-schema.md#canonical-tables) for the full table list
and requirement levels.

## Footer

The footer is the fixed-size region at the end of the file, immediately following
`manifest_json`. It is exactly 41 bytes: 8 bytes for `manifest_offset`, 8 bytes for
`manifest_length`, and 25 bytes for `PACKAGE_FOOTER_MAGIC`.

| Field | Type | Description |
|-------|------|-------------|
| `manifest_offset` | uint64 LE | Byte offset of `manifest_json` from the start of the file |
| `manifest_length` | uint64 LE | Byte length of `manifest_json` |
| `PACKAGE_FOOTER_MAGIC` | bytes | Always `CITYJSON_ARROW_PKG_V3IDX\0` |

A producer writes the manifest last, so no seek-back pass is needed.

## Manifest

`manifest_json` is a UTF-8 JSON object at `manifest_offset` with byte length
`manifest_length`. See [Package schema](package-schema.md#persistent-manifest) for the full
field specification and a JSON example.

## Reader rules

- A reader MUST verify `PACKAGE_MAGIC` at byte 0 before reading anything else. A reader
  MUST reject any file where the first 22 bytes do not exactly match `CITYJSON_ARROW_PKG_V3\0`.
- A reader MUST read the footer at the end of the file to locate the manifest.
- A reader MUST verify `PACKAGE_FOOTER_MAGIC`. A reader MUST reject any file where the
  last 25 bytes do not exactly match `CITYJSON_ARROW_PKG_V3IDX\0`.
- The manifest byte range MUST lie within the file and before the footer. A reader MUST
  reject any file where `manifest_offset + manifest_length > footer_start`.
- The manifest byte range MUST start after `PACKAGE_MAGIC`. A reader MUST reject any file
  where `manifest_offset < 22`.
- Manifest `tables` entries MUST appear in canonical tag order. A reader MUST reject a
  manifest whose `tables` array is out of order.
- A reader MUST reject a manifest that lists the same table name more than once.
- A reader MUST reject a manifest that omits any REQUIRED table.
- For each table payload, the Arrow IPC schema MUST match the canonical schema for that
  table. A reader MUST reject any payload whose schema does not match.
- For each table payload, the row count decoded from the Arrow IPC file MUST match the
  `rows` value in the manifest entry. A reader MUST reject any payload where they differ.

!!! note
    The reader maps the file into memory and decodes only the byte slices referenced
    by the manifest, so it does not load the full file into memory.
