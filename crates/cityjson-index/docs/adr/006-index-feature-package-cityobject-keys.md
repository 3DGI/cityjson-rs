# Index Feature Package CityObject Keys

## Status

Superseded by [ADR-008](008-normalized-package-cityobject-indexing-supersedes-006.md).

## Date

2026-04-27

## Context

`cityjson-index` supports three storage layouts:

- regular `CityJSON`, where root `CityObjects` are indexed from one shared
  document
- line-oriented `CityJSONFeature` streams
- directories of standalone `CityJSONFeature` files with separate metadata

Regular `CityJSON` already uses `CityObjects` keys as the indexed feature ids.
The feature-file and NDJSON layouts previously preferred the top-level
`CityJSONFeature.id`, falling back to a single `CityObjects` key only when the
top-level id was absent.

That made the same logical data index differently depending on physical
layout. It also failed for valid feature packages containing multiple
`CityObjects`: only one lookup id could represent a package that naturally has
multiple object identifiers.

The old schema also enforced uniqueness on `features.feature_id` and
`bbox_map.feature_id`. Real datasets can contain the same CityObject id in
multiple source files, source shards, or revisions. Rejecting those duplicates
made `reindex()` fail even though the index can otherwise store each row by its
stable integer feature row id.

## Decision

Feature-file and NDJSON indexing will derive indexed feature ids from every key
in the input package's `CityObjects` object.

For those layouts:

- the top-level `CityJSONFeature.id` is ignored during indexing
- every `CityObjects` key gets one indexed feature row
- all rows for the same package point at the same source byte range and use the
  same package-level bounds
- reconstruction by any alias returns the full original feature package
- package reconstruction uses the requested indexed alias as the staged feature
  id so `cityjson-lib` can validate the package against that CityObject key

Regular `CityJSON` keeps its existing behavior: one indexed feature row per
root CityObject, with child members grouped through `member_ranges`.

The SQLite schema will allow duplicate `feature_id` values. Query paths that
need a stable join or page boundary will use `features.id`, not `feature_id`.

Single-result APIs remain backward compatible by returning the earliest indexed
row for a duplicate id:

- `CityIndex::get()`
- `CityIndex::get_bytes()`
- `CityIndex::lookup_feature_ref()`
- CLI `get`
- Python `get()` and `get_json()`

Callers that need every duplicate can use the plural lookup API:

- Rust `CityIndex::lookup_feature_refs(id)`
- C FFI `cjx_index_lookup_feature_refs`
- Python `OpenedIndex.lookup_feature_refs(feature_id)`

## Implementation

The implementation lives in
[/home/balazs/Development/cityjson-rs/crates/cityjson-index/src/lib.rs](/home/balazs/Development/cityjson-rs/crates/cityjson-index/src/lib.rs).

Key points:

- replace single feature-id extraction for feature-file and NDJSON scans with a
  helper that validates `CityObjects` and returns all keys
- emit one `ScannedFeature` per CityObject key for feature-file and NDJSON
  package scans
- remove uniqueness from `features.feature_id` and `bbox_map.feature_id`
- migrate old sidecars by recreating the affected tables because SQLite cannot
  drop unique constraints in place
- order single-id lookups by `features.id LIMIT 1`
- page bbox results by `features.id` and join `bbox_map` back to `features`
  using `feature_rowid`
- expose plural lookup through Rust, C FFI, and Python

## Consequences

### Positive

- feature ids are derived consistently from CityObject identifiers across all
  layouts
- feature packages with multiple `CityObjects` become addressable by every
  contained CityObject key
- duplicate ids across files and sources no longer make reindexing fail
- callers that want deterministic legacy behavior still get one result from
  the existing APIs
- callers that need complete duplicate handling now have explicit plural
  lookup APIs

### Negative

- feature-file and NDJSON indexes can contain more rows than source feature
  packages because one package can produce multiple aliases
- duplicate aliases that point at the same package repeat bbox rows and count
  as separate indexed feature references
- old sidecar migration rewrites the affected SQLite tables on open

### Neutral tradeoff

The index treats feature-file and NDJSON aliases as lookup references to a
feature package, not as per-object extracts. Bounds and reconstructed payloads
therefore remain package-level for every alias from the same package.
