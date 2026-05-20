# ADR 001: Validated Geometry Editing APIs

Date: 2026-05-19

## Status

Accepted

## Context

CityJSON callers need to edit parsed geometries without reaching into internal storage or rebuilding city object references manually. A common case is adding appearance material themes after parsing, where the geometry handle must remain stable because `CityObject` values already reference it.

Before this change, callers could construct and insert geometries, but there was no clean inverse for the stored-parts constructor and no public replacement path that preserved the existing `GeometryHandle`. That pushed downstream tools toward local patch code that duplicated internal geometry layout and bypassed model-level invariant checks.

Material and semantic maps are also topology-sensitive: point, linestring, and surface assignments live at different flattened primitive levels. Builders need to reject mismatched geometry families and reject handles that do not exist in the owning model.

## Decision

Add public geometry editing APIs to `cityjson-types` for CityJSON 2.0:

- `Geometry::clone_stored_parts()` returns cloned stored geometry parts, including type, LoD, boundaries, semantics, material themes, texture themes, and geometry-instance payload.
- `CityModel::replace_geometry(handle, geometry)` validates the replacement with the same stored-geometry validation used by `CityModel::add_geometry`, preserves the existing handle, and returns the old geometry on success.
- Topology-aware map builders construct material and semantic maps from flattened primitive indices.
- Surface material and semantic builders accept surface-based geometries.
- Point and linestring semantic builders accept only their matching primitive families.
- Builders reject `GeometryInstance` through geometry-kind validation and reject missing material or semantic handles.

The map builders take `&CityModel` in addition to `&Geometry`. This differs from a geometry-only signature, but it is required to validate that assigned material and semantic handles exist in the model resource pools. Without model access, the builder could only construct shape-compatible maps and would defer missing-resource failures until later validation.

Textures are intentionally not part of this editing API. Texture assignment also depends on UV coordinate resources and ring-level mapping details, so it should be designed separately rather than hidden behind the simpler material/semantic builder shape.

## Consequences

Downstream tools can now implement appearance and semantic edits by cloning stored parts, replacing the relevant map/theme, reconstructing the geometry, and calling `replace_geometry` without invalidating existing city object references.

The public API keeps geometry internals encapsulated while still supporting round-trip-preserving edits for stored geometries. Replacement remains safe because the model validates vertex references, resource handles, geometry kind invariants, and instance/template constraints before mutating the geometry pool.

Builder signatures are slightly more explicit because callers must pass the owning model. This keeps missing-handle failures local to map construction and avoids creating maps that are known to be invalid for the target model.
