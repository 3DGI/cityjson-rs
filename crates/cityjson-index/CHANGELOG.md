# Changelog

## Unreleased

### Added

- Added normalized package indexing with source, package, CityObject,
  package-membership, relationship, and 3D bbox tables so every supported
  storage layout can expose stable package-level reads and spatial queries.
- Added package-oriented Rust APIs, including `lookup_cityobject_refs()`,
  `lookup_cityobject_refs_for_ids()`, `package_refs_for_cityobject()`,
  `get_packages()`, `read_package()`, `read_packages()`,
  `query_package_refs()`, `query_packages()`, `query_cityobject_refs()`,
  `query_cityobject_refs_page()`, descendant traversal, rowid lookup helpers,
  and keyset package and CityObject-reference pagination.
- Added package filter types and APIs for CityObject type and LoD selection,
  with mergeable diagnostics for missing LoDs and filtered package counts.
- Added matching C FFI and Python package bindings for plural CityObject lookup,
  package references, package reads, filtered package reads, and package filter
  diagnostics.
- Added normalized schema/API tests, CityJSONSeq terminology tests, FFI contract
  tests, and benchmark artifacts documenting baseline, implementation,
  previous-vs-current, scan-normalized, and CityObject workload performance
  comparisons.

### Changed

- Reworked indexing around valid `CityJSONFeature` packages instead of legacy
  feature rows. Regular `city-json` packages are synthetic root-plus-descendant
  closures; `city-json-seq` packages are original stream feature lines; and
  `feature-files` packages are standalone feature files.
- CityJSONSeq and feature-file indexing now records every CityObject key in a
  package while preserving the package's top-level `id` as the returned feature
  id for package reads.
- SQLite sidecars now allow duplicate external CityObject ids and preserve each
  physical occurrence separately. Single-id convenience lookups remain
  deterministic, while plural lookups expose every occurrence.
- CLI, README, C FFI, and Python docs were migrated from legacy feature APIs to
  package-oriented lookup, query, read, and filtering workflows.
- Benchmark preparation now supports all three storage layouts, avoids
  full-document clones when materializing CityJSONSeq and feature-file datasets,
  and the `bench-index` just recipes run the benchmark harness in release mode.
- Optimized `city-json-seq` and `feature-files` package reconstruction by using
  direct staged feature-slice reconstruction instead of a local
  parse/mutate/serialize/reparse cycle. Release-mode subset benchmarks show
  `read_package sample-256` around `0.26x` of the previous implementation for
  those two layouts.
- Optimized reindexing by deriving package and normalized CityObject metadata
  during backend scans and reusing prepared SQLite insertion statements during
  rebuild. Release-mode subset benchmarks show `index_reindex` averaging
  `0.603x` of the previous optimized implementation across `city-json`,
  `city-json-seq`, and `feature-files` layouts.
- Optimized package and CityObject reference workflows with batched CityObject
  id lookup, paged CityObject bbox queries, batched package membership lookup,
  and source-file reuse during `read_packages()`. Release-mode subset
  benchmarks show matched `bbox_query` rows averaging `0.875x` of the previous
  implementation, with new CityObject lookup and paging rows recorded as
  current baselines.
- Optimized `get_packages()`/CLI `get` by looking up containing package refs
  directly from CityObject external ids, and optimized scalar `read_package()`
  by reusing the provided package ref while fetching only source-location
  columns. Release-mode subset benchmarks against `8208c8e` show `get`
  averaging `0.928x` and `read_package` averaging `0.923x` of the previous
  implementation.
- Reworked sidecar rebuilds to stream scanned source headers and feature
  batches into a single SQLite writer instead of holding all scanned data and a
  cloned insertion vector in memory. Direct 25k-subset CLI checks show peak RSS
  dropping from `132124 KB` to `33004 KB` for `city-json-seq` and from
  `73072 KB` to `42904 KB` for `feature-files`.

### Removed

- Removed the legacy feature-row API surface in favor of package-oriented APIs,
  including the old `IndexedFeatureRef` reconstruction path and stale feature
  scan/read terminology.
- Removed obsolete planning documents after their design decisions were captured
  in ADRs and benchmark reports.

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
