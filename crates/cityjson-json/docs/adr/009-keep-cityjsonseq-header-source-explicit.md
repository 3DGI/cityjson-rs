# ADR 009: Keep CityJSONSeq Header Source Explicit

## Status

Accepted

## Context

ADR 008 made `cityjson-json` feature-first for `CityJSONSeq` writing:
`write_feature_stream` accepts owned `CityJSONFeature` models, validates that
they share compatible root state, derives the stream header from that shared
state, and writes a strict header-plus-feature stream.

PR #8, <https://github.com/3DGI/cityjson-rs/pull/8>, exposed a second valid
write mode from Tyler's mixed-source tile processing. In that workflow, the
caller already has an aggregate `CityJSON` document root for the output stream,
while the feature items come from sources with differing root metadata and
transforms. The strict derived-header writer rejected those streams with
`feature stream carries incompatible root state`, even though the aggregate root
was intentionally supplied by `cityjson-lib::json::write_cityjsonseq*`.

The branch also fixed an adjacent mixed-source merge bug in `cityjson-lib`:
clearing transforms during merge had round-tripped through JSON serialization,
which quantized real-world `f64` vertices through the CityJSON integer vertex
encoding path. The fix keeps the in-memory model as the authority and clears the
transform directly, preserving fractional coordinates.

The branch commits relevant to this decision are:

- `dff85c3` - fix precision issue when merging datasets with different
  transforms
- `a8d3e52` - honor base root when writing CityJSONSeq streams
- `46b125a` and `70e15d1` - lint follow-ups for the added tests

The PR reports downstream Tyler verification for the combined patch: mixed-source
tiles no longer show shifted geometry, debug `CityJSONSeq` exports for
mixed-source tiles are written successfully, and the two-source reproduction
completed with zero failed or pruned tiles.

## Decision

Keep two public CityJSONSeq writer entry points because they express different
contracts:

- `write_feature_stream` derives the stream header from the feature set. It is
  the strict default and validates that all feature models share compatible root
  state.
- `write_feature_stream_with_base` uses an explicit `CityJSON` document root as
  the stream header source. It validates the supplied root type and validates the
  feature items themselves, but it does not require feature root metadata to be
  mutually compatible.

Implement both entry points through one private stream-writing pipeline. The
shared implementation collects features, validates according to the selected
header source, computes the extent, chooses the transform, builds the header,
writes stream items, and returns the write report.

Represent the only axis of variation with a private enum:

```rust
enum FeatureStreamHeaderSource<'a> {
    DeriveFromFeatures,
    ExplicitBase(&'a OwnedCityModel),
}
```

The implementation remains generic over `Write` and `IntoIterator<Item =
OwnedCityModel>`, so the public API shape stays simple and the shared algorithm
is not tied to a concrete collection.

Header construction uses one helper, `build_feature_stream_header`, for both
cases. Its optional `header_model` is either the first feature in derived mode,
the explicit base root in explicit-base mode, or `None` for an empty stream. The
old separate `build_feature_stream_header_from_model` helper was redundant and
is removed.

Keep `serialize_feature_stream_header_model` because it centralizes the exact
header serialization policy: emit a `CityJSON` header with version and transform,
include metadata, extensions, appearance, geometry templates, and extra root
members, and exclude feature id, vertices, and city objects.

Update `cityjson-lib::json::write_cityjsonseq*` wrappers to call the explicit
base writer so their `base_root` parameter is honored.

## Alternatives Considered

Remove the explicit-base writer and require callers to normalize feature root
state before writing. This would make mixed-source streams depend on caller-side
mutation of feature metadata that is not semantically part of the feature item.
It would also lose the distinction between the aggregate stream root and the
source roots of individual features.

Put an optional base root into `CityJsonSeqWriteOptions`. This would hide a major
semantic difference inside an options object and make it easier to accidentally
weaken the strict derived-header path.

Keep separate implementations for `write_feature_stream` and
`write_feature_stream_with_base`. This worked initially but duplicated the core
write algorithm and created overlapping header helpers. The generic private
pipeline keeps the public distinction while reducing maintenance risk.

## Consequences

Positive:

- strict derived-header writing remains available and unchanged for callers that
  want root-state compatibility checking
- mixed-source callers can intentionally select the aggregate stream root
- `cityjson-lib` wrappers now honor their documented `base_root` input
- header construction has one implementation instead of two overlapping helpers
- the transform and extent reporting logic is shared between both writer modes

Negative:

- `cityjson-json` has one more public writer function
- the explicit-base mode deliberately accepts feature sets with incompatible
  root metadata, so callers choosing it must provide the authoritative aggregate
  root themselves

## Validation

The branch adds regression coverage for:

- preserving world coordinates when merging models with different fractional
  transforms
- rejecting incompatible root state in the strict derived-header writer
- writing mixed-source `CityJSONSeq` output through the explicit-base path
- honoring explicit and auto stream transforms through the `cityjson-lib` JSON
  wrappers

The post-review refactor keeps those behaviors and removes only duplicated
implementation structure.
