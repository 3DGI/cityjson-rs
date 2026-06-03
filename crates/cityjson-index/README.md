# cityjson-index

Index CityJSON datasets with a persistent SQLite sidecar. `cityjson-index` is the Rust crate; `cjindex` is the CLI.

## Problem

CityJSON datasets are often large, awkward to scan repeatedly, and split across storage layouts that make ad hoc testing expensive.

This crate gives you a consistent indexing layer so you can:

- inspect dataset layout and freshness
- reindex changed data
- fetch features by identifier
- query features by bounding box
- read regular CityJSON, CityJSONSeq, and feature-files datasets through the same package API

## What It Does

`cityjson-index` aims to be a small, predictable indexing layer for CityJSON data:

- builds or refreshes a `.cityjson-index.sqlite` sidecar
- tracks indexed sources, packages, CityObjects, relationships, and 3D bounds
- reconstructs valid CityJSONFeature package payloads on read
- exposes a CLI for dataset inspection, querying, and retrieval

## Install

### Library

```toml
[dependencies]
cityjson-index = "0.4.0"
```

### CLI

```bash
cargo install cityjson-index --bin cjindex
```

Or run it from a checkout:

```bash
cargo run --bin cjindex -- --help
```

## Usage

### As a Library

The main entry points are:

- `cityjson_index::CityIndex`
- `cityjson_index::resolve_dataset`
- `cityjson_index::StorageLayout`

Example:

```rust
use std::path::Path;

use cityjson_index::{CityIndex, resolve_dataset};

let resolved = resolve_dataset(Path::new("/data/3dbag"), None)?;
let index = CityIndex::open(resolved.storage_layout(), &resolved.index_path)?;
let status = index.status()?;
assert!(status.exists);
# Ok::<(), cityjson_lib::Error>(())
```

### As a CLI

The CLI is dataset-oriented:

```bash
cjindex inspect /data/3dbag
cjindex index /data/3dbag
cjindex reindex /data/3dbag
cjindex validate /data/3dbag
cjindex get /data/3dbag --id NL.IMBAG.Pand.0503100000012869-0
cjindex query /data/3dbag --min-x 4.4 --max-x 4.5 --min-y 51.8 --max-y 51.9
cjindex metadata /data/3dbag
```

Useful patterns:

- `inspect` reports detected layout, freshness, and coverage
- `validate` exits non-zero when the index is missing, stale, or out of sync
- `get` and `query` emit a line-oriented CityJSON stream

Explicit low-level mode is still available when you want to specify the layout directly:

```bash
cjindex get \
  --layout feature-files \
  --root /data/feature-files \
  --index /tmp/cjindex.sqlite \
  --id NL.IMBAG.Pand.0503100000012869-0
```

## Storage Layouts

### CityJSONSeq

Each `.city.jsonl` file begins with metadata, followed by one CityJSONFeature per line. The index stores one package record per feature line and one CityObject record per CityObject occurrence.

### CityJSON

Regular CityJSON files share a vertices array and a `CityObjects` dictionary. The index stores one package record per root CityObject package and reconstructs a valid CityJSONFeature on read.

### Feature Files

Each CityJSONFeature package lives in its own file. Metadata is discovered through ancestor `.json` files and cached in the SQLite index.

## Benchmarks

The `bench-index` harness prepares Basisvoorziening 3D benchmark inputs under `target/benchmarks/basisvoorziening-3d` by default:

```bash
just bench-index
just bench-index-json
```

Default cases include the original single-tile full/subset datasets and a generated multi-source dataset derived from the pinned artifact. CityJSONSeq and regular CityJSON indexing parallelism currently depends on multiple source files, so the generated multi-source case is the default signal for worker-count comparisons. Feature-file indexing parallelizes across individual feature files while keeping one source row per metadata file.

Each worker-count measurement uses a fresh SQLite index path. Treat one pass as a smoke measurement and use repeated runs for timing comparisons. Memory fields report process RSS snapshots: `current_rss_bytes` is current RSS, `process_peak_rss_bytes` is process-lifetime peak RSS, and `peak_rss_bytes` is a deprecated compatibility alias for the same process-lifetime peak.

## Development

The repository ships the `cjindex` CLI, the Rust library, and release-facing FFI/Python packaging under `ffi/`.

Useful local commands:

```bash
just check
just lint
just test
just ffi
just ci
```

## Contributing

This crate follows the workspace contract. See
[`CONTRIBUTING.md`](../../CONTRIBUTING.md) for PR guidelines and
[`docs/development.md`](../../docs/development.md) for tooling, lints,
and release flow.

## License

Dual-licensed under MIT or Apache-2.0, at your option. See
[`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE).
