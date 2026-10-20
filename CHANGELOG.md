# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project follows
Semantic Versioning at the workspace level: every release bumps every
crate in the workspace to the same version.

## [Unreleased]

### Fixed

- Fixed exact default-LoD validation rejecting structural ancestors whose
  geometry lives on descendants. Rust reports and Python summaries now check
  defaults for geometry owners, preserve empty availability for geometry
  without LoD metadata, and continue checking every explicit type override.

### Changed

- `cityjson-index` exact LoD selection now accepts multiple values: Rust
  `LodSelection::Exact(Vec<String>)`, Python `LodSelection.Exact(Sequence[str])`,
  and C `cjx_lod_selection_t.exact_lods` string lists. Empty selections are
  rejected. Missing diagnostics use `requested_lods` sets/lists and aggregate
  every unavailable value with available LoDs across packages. Recompile C
  callers for the changed ABI layout.

## [0.11.0] - 2026-08-04

### Changed
- Changed the Arrow and Parquet canonical schema to `cityjson-arrow.package.v3alpha4`; geometry ownership columns are now nullable together so pooled geometries referenced by attributes, including CityJSON address locations, round-trip correctly.

### Fixed
- Fixed `cityjson-json` deserialization and serialization of
  `CityObject.address[*].location`: address locations are imported as
  pool-backed `AttributeValue::Geometry` references and serialized back to
  their CityJSON `MultiPoint` geometry representation.
- Fixed geometry-filtered selection with relatives so structurally required
  CityObjects are retained without leaking their geometries. Empty geometry
  intersections on the same CityObject now retain that object without geometry.

### Added
- Added `cityjson-index` ordered, batched package-provenance lookup across Rust, C, and Python APIs, exposing persisted source paths without exposing the private SQLite schema.
- Added `cityjson-types` WKB/EWKB boundary serialization APIs: `Boundary::to_wkb`
  and `Boundary::from_wkb` for little-endian ISO WKB, plus `Boundary::to_ewkb`,
  `Boundary::from_ewkb`, `EwkbType`, and `EwkbBoundary` for PostGIS EWKB.
  The implementation resolves coordinates from the caller-provided vertex pool,
  preserves boundary ordering, writes XYZ geometries, closes polygon rings on
  output, and maps surface-backed CityJSON boundaries to multi-polygon,
  polyhedral-surface, or TIN EWKB as requested.
- Added a Docker-backed `cityjson-types` GIS integration test setup for
  validating WKB/EWKB output against GEOS and PostGIS.

## [0.10.0] - 2026-06-17

### Added
- Added v0.10 binding parity coverage: `cityjson-lib` Python/C++ now expose
  PROJ transformer/reprojection workflows and vertex/template-vertex mutation,
  while `cityjson-index` Python exposes aggregate bounds summaries,
  record-id pagination, bbox pages, record-id package lookup/read, and batch
  package reads.
- Added validated CityJSON geometry editing APIs for cloning stored geometry parts, replacing geometries while preserving handles, and building topology-aware material and semantic maps.
- Added optional `cityjson-lib` PROJ support behind the `proj` feature, including
  cached `cityjson_lib::ops::Transformer` construction and `ops::reproject` for
  main vertex-pool reprojection.
- Added public mutable access to CityJSON real-world vertex pools so callers can
  perform in-place low-level coordinate operations without serialization
  round-trips.
- Added a `cityjson-index` aggregate feature-bounds summary API for callers that
  need whole-index bounds and feature counts without scanning feature pages.
- Added `cityjson-index` batch reconstruction from persisted feature references.
- Added `cityjson-index` rowid-ordered decoded scan APIs and rowid-keyed lookup
  and reconstruction helpers.

### Changed
- `cityjson-lib` Python now derives `__version__` from installed package
  metadata instead of a stale hard-coded runtime value.
- `cityjson-lib` C++ package version is included in release replacement
  automation so it stays aligned with the workspace release line.
- Expanded the devcontainer toolchain and root `just ffi` dispatcher so FFI,
  benchmark, and coverage recipes have the required local tools and route
  crate-specific commands correctly.
- Aligned `cityjson-lib` and `cityjson-index` SQLite bindings with the
  `proj-sys` dependency range so full-workspace all-feature builds resolve a
  single `libsqlite3-sys`.
- `cityjson-lib::ops::reproject` consumes a model, updates
  `metadata.referenceSystem`, clears any root transform, and leaves geometry
  template vertices and geometry-instance transforms unchanged.
- Renamed the core Rust crate from `cityjson` to `cityjson-types`, moved it to
  `crates/cityjson-types`, and updated workspace consumers to import
  `cityjson_types::...` directly. No compatibility alias remains in
  `cityjson-lib`.
- `cityjson-lib::ops::append` and `merge` now reconcile differing input
  transforms instead of rejecting them. Identical transforms are preserved,
  mixed transforms clear the merged model transform, and transform-free
  inputs stay transform-free.
- `cityjson-json::facade::append` now mirrors the same transform merge
  semantics as the Rust layer.
- FFI tooling now resolves the shared library from the workspace target
  directory, which keeps the C/C++ smoke tests aligned with Cargo's build
  output.
- Consolidated the seven `cityjson-*` Rust crates (`cityjson`,
  `cityjson-json`, `cityjson-arrow`, `cityjson-parquet`, `cityjson-lib`,
  `cityjson-fake`, `cityjson-index`) plus the two FFI core crates
  (`cityjson-lib-ffi-core`, `cityjson-index-ffi-core`) and the
  `cityjson-lib-wasm` shim into a single Cargo workspace at
  [`3DGI/cityjson`](https://github.com/3DGI/cityjson). All crates move
  to `shared-version = true` and bump together via
  `cargo release --workspace`.
- Unified the per-crate `CITYJSON_*_SHARED_CORPUS_ROOT` environment
  variables into a single `CITYJSON_SHARED_CORPUS_ROOT`.
- Bumped all workspace crates to `0.8.0` to mark the transition — this
  is a higher version than any pre-merge crate held.
- Crate names on crates.io and Python package names on PyPI are
  unchanged; only source repository URLs have moved.
- Optimized `cityjson-index` full-index page scans so later pages use a direct
  `features.id` range scan instead of a nullable paging predicate.
- Optimized `cityjson-index` batch reconstruction to group reads by source and
  reuse source metadata and open files while preserving caller order.

---

Per-crate history prior to the workspace merge is preserved in each
crate's former `CHANGELOG.md` inside `crates/<name>/CHANGELOG.md` and in
the full git history (line-level `git log` / `git blame` continues to
work across the move).
