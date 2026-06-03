# Direct Staged Feature Reconstruction

## Status

Accepted

## Date

2026-06-03

## Context

`cityjson-index` read performance is dominated by reconstructing matched
packages into `CityJSONFeature` models after SQLite lookup has already found the
matching package references.

Regular `cityjson` package reads already used the direct staged assembly path in
`cityjson-json`: indexed CityObject fragments and localized vertices are passed
to `from_feature_assembly_with_base`, which delegates to the direct builder.
That path still has room for deeper parser work in the future, but it no longer
uses the older full feature parse/merge/reparse API.

The `cityjson-seq` and feature-files package path still performed avoidable work
inside `cityjson-index` for every read:

- parse the feature bytes into `serde_json::Value`
- inspect and sometimes insert a synthetic root CityObject when the feature id
  was not present in `CityObjects`
- serialize the mutated feature back to bytes
- call staged reconstruction, which parsed the feature again

That local `Value` round trip was not part of the index semantics. It existed to
preserve package-id behavior before handing the feature to the JSON staged
reader.

## Decision

Expose direct staged feature-slice APIs in `cityjson-json` and re-export them
through `cityjson-lib`:

- `cityjson_json::staged::from_feature_slice_with_base_direct`
- `cityjson_json::staged::from_feature_slice_with_indexed_id_and_base`
- `cityjson_lib::json::staged::from_feature_slice_with_base_direct`
- `cityjson_lib::json::staged::from_feature_slice_with_indexed_id_and_base`

The plain direct API parses the base root and feature root into the existing
prepared root representation, merges base context with the feature root, and
builds the owned `CityModel` directly. The indexed-id variant uses the indexed
package id as the returned `CityJSONFeature` id. When that package id is not a
CityObject key, the indexed-id staged reader adds the same synthetic wrapper
CityObject that `cityjson-index` previously inserted locally.

`cityjson-index` now delegates `cityjson-seq` and feature-files package
reconstruction to the indexed-id direct staged API. The public package read APIs,
CLI output, FFI behavior, Python behavior, SQLite schema, and duplicate-id
semantics remain unchanged.

The existing regular `cityjson` direct assembly path remains in place. Further
optimization of regular `cityjson` fragment import can be addressed separately
if benchmarks show the fragment-localization step is still dominant.

## Consequences

### Positive

- `cityjson-seq` and feature-files package reads avoid one full feature
  `serde_json::Value` materialization and one serialization per reconstructed
  package.
- The wrapper CityObject compatibility behavior now lives in the indexed-id
  staged JSON reconstruction path instead of being duplicated in `cityjson-index`.
- The staged API surface now makes direct feature-slice reconstruction explicit
  for library callers and downstream crates.
- The change does not require an index rebuild or schema migration.

### Negative

- The indexed-id staged path still needs to parse the `CityObjects` map as raw
  entries when it has to detect or add the wrapper root CityObject.
- The plain direct path preserves the normal CityJSON validation behavior: the
  returned feature id must resolve to a CityObject during model construction.
- Regular `cityjson` package reads are not materially changed by this ADR
  because they were already using direct staged assembly.

### Neutral tradeoff

The implementation keeps returning owned `CityModel` values. `BorrowedCityModel`
remains outside this change because historical benchmarks showed only small,
workload-specific deserialization gains, while the main bottleneck here was the
avoidable parse/mutate/serialize/reparse cycle.
