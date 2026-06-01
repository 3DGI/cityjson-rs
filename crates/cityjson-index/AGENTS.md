# AGENTS.md — cityjson-index

Orientation for AI coding agents working in the `cityjson-index` crate.
Humans: see the root [`AGENTS.md`](../../AGENTS.md) for workspace-wide
conventions — this file is derived from and defers to that document on
conflict. This file focuses on **crate-specific** guidance.

---

## What this crate is

`cityjson-index` is the **SQLite-backed indexing layer** for CityJSON
datasets. It provides a consistent, persistent abstraction over diverse storage
layouts so you can inspect, query, and retrieve CityJSON features without
manually handling layout differences.

- **CLI**: `cjindex` — dataset-first commands for inspection, indexing,
  validation, feature retrieval, and bounding-box queries
- **Library**: `cityjson_index` crate — programmatic access to index
  construction, feature lookup, spatial queries, and paginated scans
- **FFI**: C bindings (`ffi/core/`) and Python wheels (`ffi/python/`),
  published to PyPI as `cityjson-index`

The index is a **sidecar SQLite database** at `<DATASET_DIR>/.cityjson-index.sqlite`
by default, tracking source metadata, feature locations, bounding boxes, and
CityObject counts for fast spatial and identifier-based lookups.

## Crate Layout

```
.
├── Cargo.toml              # Crate manifest; inherits from workspace
├── justfile                # Crate-specific recipes (prefer over raw cargo)
├── README.md               # Public docs: install, usage, examples
├── CHANGELOG.md            # Keep a Changelog; promoted at release
├── src/
│   ├── lib.rs              # Main library API (CityIndex, resolve_dataset, ...)
│   ├── main.rs             # CLI entry point (cjindex binary)
│   ├── benchmark.rs        # Benchmark utilities
│   ├── profile.rs          # Profiling support (--profile flag)
│   └── bin/
│       └── bench-index.rs   # Benchmark harness
├── tests/
│   ├── common/             # Shared test utilities
│   ├── cityjson.rs         # Regular CityJSON layout tests
│   ├── cli.rs               # CLI command tests
│   ├── corpus.rs           # Corpus integration tests
│   ├── feature_files.rs    # Feature-files layout tests
│   ├── ndjson.rs           # NDJSON/CityJSONSeq layout tests
│   └── profile.rs          # Profiling tests
├── ffi/
│   ├── core/               # C FFI bindings (cityjson-index-ffi-core)
│   └── python/              # Python package + wheels (PyPI: cityjson-index)
├── docs/
│   ├── adr/                 # Architecture Decision Records
│   ├── glossary.md         # Crate-specific terminology
│   └── *.md                # Design docs, plans (full-scan iterators, etc.)
└── tools/
    └── ffi.sh              # FFI build/test helper script
```

## Tooling & Commands

**Use the crate's `justfile` — do not invent raw `cargo` invocations.**
The crate's justfile scopes commands to this crate only (avoiding full
workspace traversals).

| Recipe | What it does | Notes |
|--------|--------------|-------|
| `check` | `cargo check` for lib, bin, tests | Fast compile validation |
| `build [args]` | Build the `cjindex` binary | Pass extra args to cargo |
| `lint` | `cargo clippy` with workspace lints | Fix all clippy errors |
| `fmt` | `cargo fmt` for this crate + ffi-core | No rustfmt.toml |
| `fmt-check` | `cargo fmt --check` | CI uses this |
| `test` | Library + integration tests | All features enabled |
| `bench-index` | Human-readable benchmark output | Uses `target/benchmarks/` |
| `bench-index-json` | Benchmark output as JSON | For CI/automation |
| `ffi` | Build + test FFI core crate | C bindings |
| `ffi-build` | Build FFI core only | |
| `ffi-test` | Test FFI core only | |
| `test-python` | tox-based Python smoke tests | Requires Python 3.11-3.13 |
| `build-python` | Build Python wheel | Uses maturin |
| `clean` | Clean FFI artifacts | |
| `ci` | Full pipeline: fmt-check, check, lint, test, ffi, test-python, doc | **Must pass before PR** |
| `doc` | Docsrs build (all features, no deps) | |

**Environment variables:**
- `CITYJSON_INDEX_WORKERS` — override parallel worker count (default: CPU count)
- `CITYJSON_SHARED_CORPUS_ROOT` — path to cityjson-corpus checkout for tests

## Common Workflows

### As a CLI User

The CLI is **dataset-first**: point it at a dataset directory and it
auto-detects the storage layout.

```bash
# Inspect a dataset (layout, freshness, coverage, counts)
cjindex inspect /data/3dbag
cjindex inspect /data/3dbag --json  # Machine-readable

# Build or rebuild the index
cjindex index /data/3dbag           # Auto-detect layout
cjindex reindex /data/3dbag        # Explicit: drop and rebuild

# Validate index matches dataset (exits non-zero on issues)
cjindex validate /data/3dbag

# Get a single feature by identifier
cjindex get /data/3dbag --id NL.IMBAG.Pand.0503100000012869-0

# Query features intersecting a bounding box
cjindex query /data/3dbag \
  --min-x 4.4 --max-x 4.5 \
  --min-y 51.8 --max-y 51.9

# Get dataset metadata
cjindex metadata /data/3dbag

# Override index path or specify layout explicitly
cjindex get \
  --layout feature-files \
  --root /data/feature-files \
  --index /tmp/my-index.sqlite \
  --id some-feature-id

# Profile a command (writes JSON profile to file)
cjindex query /data/3dbag --min-x 4.4 --max-x 4.5 --profile /tmp/query-profile.json
```

All read commands (`get`, `query`, `metadata`) emit **line-oriented CityJSON**
streams: the first line is the metadata header, subsequent lines are
CityJSONFeature objects. Use `--output FILE` to write to a file instead
of stdout.

### As a Library User

```rust
use std::path::Path;
use cityjson_index::{CityIndex, BBox, resolve_dataset, StorageLayout};

// Dataset-first: auto-detect layout from directory
let resolved = resolve_dataset(Path::new("/data/3dbag"), None)?;
let index = CityIndex::open(resolved.storage_layout(), &resolved.index_path)?;

// Check index status
let status = index.lookup_feature_ref("some-id")?;

// Bounding box query
let bbox = BBox {
    min_x: 4.4, max_x: 4.5,
    min_y: 51.8, max_y: 51.9,
};
let features = index.query(&bbox)?;

// Iterate all features (paginated)
let mut page_iter = index.scan_feature_pages(100)?;
while let Some(page) = page_iter.next().transpose()? {
    // process page of features
}

// Filter features by type and LoD
use cityjson_index::{FeatureFilter, LodSelection};
let filter = FeatureFilter {
    cityobject_types: Some(vec!["Building".to_string()].into_iter().collect()),
    default_lod: LodSelection::Highest,
    ..Default::default()
};
let filtered = index.read_filtered_features(&feature_refs, &filter)?;
```

### Storage Layout-Specific Notes

The crate handles three storage layouts transparently:

| Layout | Description | Indexing Strategy |
|--------|-------------|-------------------|
| **NDJSON / CityJSONSeq** | Each `.city.jsonl` file: metadata line + one feature per line | Stores byte offsets for each feature line |
| **CityJSON** | Regular CityJSON files with shared vertices + `CityObjects` dict | Stores feature package ranges, reconstructs models on read |
| **Feature Files** | One feature per file, metadata in ancestor `.json` files | Caches metadata in SQLite, indexes individual feature files |

Auto-detection logic (in `resolve_dataset`):
1. Looks for `.city.jsonl` files → NDJSON
2. Looks for `.city.json` files → CityJSON
3. Looks for `metadata.json` + `.city.jsonl` files → Feature Files

If multiple layouts match, it returns an error; use explicit `--layout`
flags to disambiguate.

## Crate-Specific Conventions

### Code Style

- **Match existing style** in the file you're editing — the crate is
  internally consistent
- Use `cityjson_lib::Error` and `cityjson_lib::Result` for error handling
- SQLite operations should be **transactional** (see `Index::rebuild`)
- Prefer **dataset-first** APIs that auto-detect layout from a path
- Keep explicit layout modes available but secondary

### Error Handling

```rust
use cityjson_lib::{Error, Result};

fn my_func() -> Result<()> {
    // Use import_error for user-facing errors
    Err(Error::Import("meaningful message".to_string()))
}
```

The crate uses `import_error` helper for consistent error construction.

### Feature Reconstruction

- Preserve the **original structure** of CityJSON features
- Cache source metadata in SQLite to avoid re-reading on every lookup
- Group backend reads by source file when processing batches
- Use `cityjson_lib::ops` for selections and extractions

### Index Operations

- Always **validate freshness** (index mtime vs source mtimes)
- Check **coverage** (all detected sources are indexed)
- Report **issues** (missing files, changed files, count mismatches)
- Use **paginated iterators** for large scans (avoid loading entire index into memory)

## Testing Strategy

Tests live in `tests/` and are organized by storage layout:

| Test File | Focus |
|-----------|-------|
| `cityjson.rs` | Regular CityJSON file indexing and queries |
| `ndjson.rs` | CityJSONSeq / NDJSON layout |
| `feature_files.rs` | Feature-files layout |
| `cli.rs` | CLI command integration tests |
| `profile.rs` | Profiling flag behavior |
| `corpus.rs` | Integration with cityjson-corpus test data |

**Running tests:**
```bash
# Full test suite for this crate
just test

# With corpus tests (requires CITYJSON_SHARED_CORPUS_ROOT)
cargo test -p cityjson-index -- --include-ignored

# Specific test
cargo test -p cityjson-index --test feature_files
```

**Test organization:**
- Use `common/` module for shared test fixtures and helpers
- Each layout has its own test file with self-contained setup
- Tests that require corpus data use `#[ignore]` and check for
  `CITYJSON_SHARED_CORPUS_ROOT` env var
- Feature filtering tests exercise `FeatureFilter` with various type/LoD combinations

## FFI & Python

The crate ships **prebuilt Python wheels** for Linux x86_64, macOS
x86_64/arm64, and Windows AMD64, supporting Python 3.11–3.13.

### FFI Architecture

```
┌─────────────────────┐
│   Rust Library       │  cityjson-index
│   (src/lib.rs)       │
└────────────┬────────┘
             │
             ▼
┌─────────────────────┐
│   C FFI Core         │  cityjson-index-ffi-core
│   (ffi/core/)        │  - cbindgen-generated headers
│                       │  - Exports key types and functions
└────────────┬────────┘
             │
             ▼
┌─────────────────────┐
│   Python Package     │  cityjson-index (PyPI)
│   (ffi/python/)      │  - maturin-based build
│                       │  - PyO3 bindings
│                       │  - tox for testing
└─────────────────────┘
```

### FFI Commands

| Command | Purpose |
|---------|---------|
| `just ffi` | Build + test FFI core |
| `just ffi-build` | Build FFI core only |
| `just test-python` | Run Python tox tests |
| `just build-python` | Build Python wheel |

### FFI Development Notes

- **Do NOT hand-edit** `ffi/python/pyproject.toml` version — it's synced
  from the workspace `Cargo.toml` by `cargo-release`
- Python package uses **maturin** for building Rust extensions
- `ffi/python/tox.ini` and `ffi/python/.tox/` handle Python test environments
- `tools/ffi.sh` is the canonical wrapper for FFI operations
- Public Rust API changes may require corresponding FFI updates
- FFI exports must be `#[no_mangle]` and use `extern "C"`
- Python bindings use `PyO3` and are generated from Rust doc comments

### Python Usage

```python
from cityjson_index import CityIndex, resolve_dataset, BBox

# Dataset-first
resolved = resolve_dataset("/data/3dbag")
index = CityIndex.open(resolved.storage_layout, resolved.index_path)

# Query
bbox = BBox(4.4, 4.5, 51.8, 51.9)
features = index.query(bbox)

# Get by ID
feature = index.get("NL.IMBAG.Pand.0503100000012869-0")
```

## Debugging & Profiling

### Environment Variables

| Variable | Effect |
|----------|--------|
| `CITYJSON_INDEX_WORKERS=N` | Override parallel worker count |
| `RUST_LOG` | Enable rust logging (if configured) |

### Inspection Tools

```bash
# Inspect the SQLite index directly
sqlite3 /data/3dbag/.cityjson-index.sqlite \
  "SELECT * FROM sources;"

sqlite3 /data/3dbag/.cityjson-index.sqlite \
  "SELECT feature_id, min_x, max_x, min_y, max_y FROM feature_bbox LIMIT 10;"

# Check index status via CLI
cjindex inspect /data/3dbag --json | jq

# Profile a command
cjindex query /data/3dbag --min-x 4.4 --max-x 4.5 --profile /tmp/profile.json
cat /tmp/profile.json | jq
```

The `--profile` flag (available on all commands) writes a JSON file with
measurements for each operational phase: argument resolution, SQLite open,
lookup, reconstruction, serialization, etc.

### Benchmarking

```bash
# Human-readable benchmark output
just bench-index

# JSON output for automation
just bench-index-json

# Custom benchmark inputs
cargo run -p cityjson-index --bin bench-index -- \
  --dataset /custom/path \
  --workers 4 \
  --json
```

Benchmark harness prepares test data under `target/benchmarks/basisvoorziening-3d/`
by default. Use repeated runs for stable measurements.

## Constraints (What NOT to Do)

- **Don't hand-edit Python versions** — `ffi/python/pyproject.toml` and
  setup.cfg version fields are synced from `Cargo.toml` at release time
- **Don't add direct path dependencies** — use `[workspace.dependencies]`
- **Don't change SQLite schema without a migration path** — existing
  sidecars must remain readable or be auto-migrated on open
- **Don't break index compatibility** without a very good reason and
  documented migration strategy
- **Don't use raw SQLite queries in library code** — go through the
  `Index` abstraction to ensure schema version handling
- **Don't expose internal types** in the public API unless they're
  intended for downstream use
- **Don't hardcode layout assumptions** — always use `resolve_dataset` or
  explicit `StorageLayout` parameters

### Schema Migration Notes

The crate handles schema migrations automatically:
- Duplicate `feature_id` constraint was removed (old indexes are migrated on open)
- Missing columns are added with `ensure_*` methods in `Index::open`
- If you must add a new column or table, add a corresponding `ensure_*`
  method that runs on index open

## Key APIs at a Glance

### Index Construction

| Function | Purpose |
|----------|---------|
| `resolve_dataset(path, index_override)` | Auto-detect layout, return `ResolvedDataset` |
| `CityIndex::open(layout, index_path)` | Open existing or create new index |
| `CityIndex::reindex()` | Rebuild index from backend |

### Feature Lookup

| Function | Purpose |
|----------|---------|
| `lookup_feature_ref(id)` | Get lightweight reference by ID |
| `lookup_feature_refs(id)` | Get all references for duplicate IDs |
| `lookup_feature_ref_by_rowid(row_id)` | Get reference by SQLite row ID |
| `get(id)` | Get `CityModel` by ID |
| `get_with_metadata(id)` | Get `CityModel` + source metadata |
| `read_feature(ref)` | Reconstruct from `IndexedFeatureRef` |
| `read_features(refs)` | Batch reconstruct from refs |

### Spatial Queries

| Function | Purpose |
|----------|---------|
| `query(bbox)` | All features intersecting bbox (vec) |
| `query_iter(bbox)` | Iterator over features |
| `query_iter_with_metadata(bbox)` | Iterator with source metadata |
| `query_iter_with_ids(bbox)` | Iterator with feature IDs |

### Full-Index Scans

| Function | Purpose |
|----------|---------|
| `iter_all()` | Iterator over all features |
| `iter_all_with_ids()` | Iterator with feature IDs |
| `iter_all_with_metadata()` | Iterator with source metadata |
| `scan_features()` | Iterator of `IndexedFeature` (ref + model) |
| `scan_feature_pages(page_size)` | Paginated iterator |
| `feature_ref_page(offset, limit)` | Page of references |
| `feature_bounds_summary()` | Aggregate bounds + count |

### Inspection & Validation

| Function/Type | Purpose |
|---------------|---------|
| `index.inspect()` | `DatasetInspection` with layout, counts, status |
| `index.validate()` | `ValidationReport` with ok/fail status |
| `IndexStatus` | exists, freshness, coverage, counts, issues |
| `DatasetInspection` | dataset_root, layout, manifest, detected counts, index status |

### Feature Filtering

| Type | Purpose |
|------|---------|
| `FeatureFilter` | Type selection + LoD selection |
| `LodSelection` | All, Highest, Exact("2") |
| `FilteredFeature` | Model + `FeatureFilterDiagnostics` |
| `read_filtered_features(refs, filter)` | Batch filter with diagnostics |

## Integration Points

### Upstream Dependencies (this crate depends on)

| Crate | Purpose |
|-------|---------|
| `cityjson-types` | Core CityJSON 2.0 types |
| `cityjson-lib` | Higher-level CityJSON operations, model types |

**Note**: The crate uses `cityjson-lib` with the `json` feature enabled
for JSON serialization support.

### Downstream Dependencies (depend on this crate)

None — this is a **leaf crate** in the workspace dependency graph.
Changes here don't affect other crates in the monorepo.

### Shared with Root

- `rust-toolchain.toml` — Rust stable version pin
- `Cargo.lock` — Workspace lockfile
- `.github/workflows/` — CI configuration
- `.github/scripts/` — CI helper scripts

## Glossary (Crate-Specific Terms)

See `docs/glossary.md` for the full crate glossary. Key terms:

| Term | Meaning |
|------|---------|
| **Sidecar** | The `.cityjson-index.sqlite` database alongside dataset files |
| **Storage Layout** | How CityJSON data is organized on disk (NDJSON, CityJSON, Feature Files) |
| **Feature Package** | A single CityJSON file or NDJSON line containing one or more features |
| **Feature Reference** | Lightweight `IndexedFeatureRef` with location info (no payload) |
| **Row ID** | SQLite integer primary key for features (stable, ordered) |
| **Feature ID** | String identifier from CityJSON (may be duplicated) |
| **Source ID** | SQLite foreign key to the `sources` table |

## See Also

- **[Root `AGENTS.md`](../../AGENTS.md)** — Workspace-wide conventions,
  toolchain, commands, hard rules, CI, release, licensing
- **[`CONTRIBUTING.md`](../../CONTRIBUTING.md)** — PR guidelines, AI use
  policy, testing expectations
- **[`docs/development.md`](../../docs/development.md)** — Full development
  contract: toolchain versions, MSRV, clippy/rustfmt flags, Python packaging,
  justfile recipes, release flow
- **[`README.md`](./README.md)** — Public crate documentation
- **[`CHANGELOG.md`](./CHANGELOG.md)** — Release history
- **[`docs/glossary.md`](docs/glossary.md)** — Crate-specific terminology
- **[`ffi/README.md`](./ffi/README.md)** — FFI/Python-specific documentation
