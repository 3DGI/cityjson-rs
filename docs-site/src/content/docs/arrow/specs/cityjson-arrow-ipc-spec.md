---
title: Arrow IPC layout
description: Imported cityjson-arrow specification page.
---

# Arrow IPC stream layout

This document specifies the binary format written by `write_stream` and read by `read_stream`.

## Terminology

The key words MUST, MUST NOT, REQUIRED, SHOULD, and OPTIONAL in this document are to be
interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## Version

| Field | Value |
|---|---|
| Stream magic | `CITYJSON_ARROW_STREAM_V3\0` (25 bytes, including null terminator) |
| Schema id | `cityjson-arrow.package.v3alpha3` |

## Overall layout

```text
STREAM_MAGIC          (25 bytes)
prelude_len: u64 LE   (8 bytes)
prelude_json          (prelude_len bytes, UTF-8)
frame_0
frame_1
...
end_marker: 0xFF      (1 byte)
```

All multi-byte integers are little-endian.

## Prelude

`prelude_json` is a UTF-8 JSON object. It MUST contain:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `header` | object | REQUIRED | A `CityArrowHeader` object; see [Package schema](package-schema.md#header) |
| `projection` | object | REQUIRED | A `ProjectionLayout` object; see [Package schema](package-schema.md#projection-layout) |

The prelude is small by design. It carries only the information needed to validate and
decode the frames that follow.

## Frames

Each frame carries one canonical table. Frames MUST appear in canonical tag order.

```text
table_tag:             u8      (1 byte)
rows:                  u64 LE  (8 bytes)
arrow_ipc_stream_payload       (self-delimiting)
```

| Field | Type | Description |
|-------|------|-------------|
| `table_tag` | uint8 | Integer tag identifying the canonical table; see [Package schema](package-schema.md#canonical-tables) |
| `rows` | uint64 LE | Declared row count; the reader MUST verify this against the decoded batch |
| `arrow_ipc_stream_payload` | bytes | One Arrow IPC stream written with `StreamWriter`; self-delimiting |

The Arrow IPC stream payload is self-delimiting. A producer does not need to know its
length before writing.

## End marker

The stream ends with a single byte `0xFF`. A reader MUST treat any byte other than `0xFF`
after the last frame as a format error.

## Reader rules

- A reader MUST verify `STREAM_MAGIC` before reading anything else. A reader MUST reject
  any stream where the first 25 bytes do not exactly match `CITYJSON_ARROW_STREAM_V3\0`.
- A reader MUST decode the JSON prelude before reading any frames.
- Frames MUST appear in canonical tag order. A reader MUST reject a stream where frames
  are out of order.
- A reader MUST reject a stream that contains duplicate table tags.
- A reader MUST reject a stream that omits any REQUIRED table
  (`metadata`, `vertices`, `geometry_boundaries`, `geometries`, `cityobjects`).
- For each frame, the Arrow IPC payload schema MUST match the canonical schema for that
  table. A reader MUST reject any frame whose payload schema does not match.
- For each frame, the decoded row count MUST match the declared `rows` value. A reader
  MUST reject any frame where they differ.
