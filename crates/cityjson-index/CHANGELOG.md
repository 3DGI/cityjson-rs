# Changelog

## Unreleased

### Added

- Added the SQLite row id to `IndexedFeatureRef` and made the type
  serializable so downstream tools can persist row-ordered scan references.
- Added `CityIndex::read_features()` for batch reconstruction from persisted
  `IndexedFeatureRef` values.
- Added decoded rowid-ordered scan APIs, including `scan_features()` and
  `scan_feature_pages()`, that return feature references together with decoded
  `CityModel` payloads.
- Added rowid-keyed lookup and reconstruction helpers for callers that persist
  SQLite feature row ids instead of feature-id strings.
- Added `CityIndex::feature_bounds_summary()` to return whole-index 3D bounds
  and feature count in one aggregate query, with `None` for empty indexes.
- Added `CityIndex::lookup_feature_refs()` plus matching C FFI and Python
  bindings for callers that need every indexed row for a duplicate feature id.
- Added `FeatureFilter` and `CityIndex::read_filtered_features()` so callers can
  apply explicit CityObject type and LoD selections with shared diagnostics
  before computing extents, grid assignments, or exported tile content.

### Changed

- Stopped routing core model types through `cityjson_lib::cityjson`; Rust code
  now uses the renamed `cityjson_types` crate directly after the
  `cityjson-types` package rename.
- Feature-file and NDJSON indexing now derives feature ids from every key in a
  feature package's `CityObjects` object and ignores the package's top-level
  `id` during indexing.
- SQLite sidecars now allow duplicate `feature_id` values. Existing sidecars
  with the old unique constraints are migrated on open by recreating the
  affected index tables.
- Single-id lookup APIs continue to return one result for duplicate ids, using
  the earliest indexed feature row deterministically.
- Optimized ordered full-index page scans by using separate first-page and
  later-page SQL paths. Later pages now page with `WHERE f.id > ?`, preserving
  result order and page semantics while allowing SQLite to use the integer
  primary-key range scan.
- Optimized batch feature reconstruction so source metadata is loaded once per
  source and backend reads are grouped by source file while preserving input
  order.
- Filtered feature reads now use `cityjson_lib::ops` selections for object-type,
  exact LoD, and explicit highest-LoD policies, while leaving unfiltered reads
  policy-neutral.
- Optimized reindexing by deriving package and normalized CityObject metadata
  during backend scans and reusing prepared SQLite insertion statements during
  rebuild. Release-mode subset benchmarks show `index_reindex` averaging
  `0.603x` of the previous optimized implementation across `city-json`,
  `city-json-seq`, and `feature-files` layouts.

## 0.4.2

### Added

- Added a JSON-emitting benchmark harness for Basisvoorziening 3D datasets, including full-tile, deterministic subset, and optional multi-tile preparation flows.
- Added Linux-only `--profile` support for `cjindex` commands with stage timings, RSS snapshots, and machine-readable JSON output.
- Added process-local worker-count control for indexing via `CITYJSON_INDEX_WORKERS`, with parallel backend scanning during `reindex()` and benchmark runs that exercise the configured worker count.
- Added benchmark preparation for a deterministic multi-source Basisvoorziening 3D case so the default benchmark run now includes a real parallelism signal.

### Changed

- Consolidated benchmark and profiling documentation under repo-local plans for parallel indexing, benchmarking, and test cleanup.
- Feature-file indexing now shards work by feature file after metadata discovery while preserving one SQLite source row per metadata file.
- Benchmark runs now allocate a fresh SQLite sidecar per worker count, and RSS fields distinguish current process RSS from process-lifetime peak RSS.

### Fixed

- Built the `cjindex` binary in the `just test` path so the CLI integration tests can resolve the executable during `just ci` and release validation.
- Added a filesystem fallback for the `cjindex` test helper so release validation can find the binary even when `CARGO_BIN_EXE_cjindex` is not exported.
- Kept fast correctness tests on tracked fixtures while moving corpus-backed coverage behind `CITYJSON_CORPUS`.
- Removed misleading fake profiling stages from `cjindex reindex` and report the real scan-and-rebuild operation instead.

## 0.4.0

- Removed benchmark binaries, Criterion harnesses, and benchmark-only test corpus preparation from CI and the test harness.
- Replaced generated benchmark data with small tracked correctness fixtures for CityJSON, CityJSONSeq/NDJSON, and feature-file layouts.
- Upgraded `cityjson-lib` to 0.6.0 while keeping only the JSON feature enabled for `cityjson-index` and its FFI core.
- Scoped CI formatting and validation to correctness targets and removed Arrow/Parquet/Criterion from the `cityjson-index` dependency graph.
- Fixed the Python binding validation path to build a temporary JSON-only `cityjson-lib` wheel for tests.
- Replaced the GitHub Actions Rust toolchain action with direct `rustup` installation to avoid action archive download failures.

## 0.4.1

- Bumped the package version to `0.4.1`.
- Aligned `cityjson-lib` with `0.6.1` for the release train.

## 0.3.1

- Maintenance release for the initial public package metadata and release workflow.

## 0.3.0

- First public release of the `cityjson-index` crate.
- Ships the `cjindex` CLI for dataset inspection, indexing, and queries.
- Packages the public docs and release metadata for a first public GitHub/crates.io release.
