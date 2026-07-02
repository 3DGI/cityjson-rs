# Geometry Test Corpus Cleanup Plan

## Summary

Use `geometry_test_suite.md` as the structural source of truth and
`geometry_error_cases.md` as the coverage checklist. The cleaned corpus will be
organized by canonical fixtures and required test families, not by accumulated
historical test names. Every retained, merged, rewritten, or new test gets a
brief `/// Inputs / Assertions / Purpose` docstring.

No public API, type, or behavior changes.

## Integrated Test Strategy

- Keep the canonical fixture strategy from `geometry_test_suite.md`: `P1`,
  `L1`, `S1`, `D1`, `MS1`, `T1`, and `I1`.
- Cover every `geometry_error_cases.md` item inside the smallest relevant
  `geometry_test_suite.md` family.
- Prefer mutation tables over separate one-off malformed fixtures.
- Drop runtime rejection tests when the bad state is unrepresentable and replace
  them with construction/API-level tests.
- Keep WKB as a separate boundary serialization family because it is listed in
  `geometry_error_cases.md` but intentionally outside the core geometry mapping
  suite.
- Treat JSON read/write issues as out of scope for `cityjson-types`; they belong
  in `cityjson-json`.

## Test Families

1. `canonical_fixture_acceptance`
   - Uses all seven canonical fixtures.
   - Covers valid topology, dense maps, `CompositeSurface`/`MultiSurface`
     shared shape, `CompositeSolid`/`MultiSolid` shared shape, template
     geometry, and valid `GeometryInstance`.
   - Merges the current fixture acceptance and boundary-consistency smoke tests.

2. `flat_boundary_offset_consistency_cases`
   - Mutates `S1` or `D1` flat offsets: first offset not zero, decreasing
     offset, offset exceeding child length.
   - Covers inconsistent boundary offsets and trusted/stored insertion
     validation.
   - Replaces scattered offset tests with one table-driven family.

3. `boundary_shape_mismatches_and_empty_required_levels_are_rejected`
   - Mutates declared kind versus populated boundary layers: `MultiSurface` with
     shells/solids, `Solid` with solids, `MultiSolid` missing levels, surface
     without rings, shell without surfaces, solid without shells, ring/line
     without vertices.
   - Covers invalid topology, boundary shape mismatch, and empty child segments.
   - Merges the current geometry-validation shape tests and boundary
     shape-detection tests.

4. `boundary_roundtrip_preserves_flat_and_nested_topology`
   - Runs nested -> flat -> nested and flat -> nested -> flat over `P1`, `L1`,
     `S1`, `D1`, `MS1`, and `T1`.
   - Asserts exact flat arrays, exact nested topology, inner-ring attachment,
     inner-shell attachment, and multi-solid ordering.
   - Replaces the current per-fixture roundtrip matrix with two parameterized
     families.

5. `dense_semantic_and_material_maps_are_accepted`
   - Uses `P1`, `L1`, `S1`, `D1`, and `MS1`.
   - Asserts exactly one populated bucket, correct bucket for geometry kind,
     dense primitive count, preserved `None`, and no shell/solid regrouping in
     maps.
   - Merges current semantic/material bucket acceptance tests.

6. `wrong_or_non_dense_semantic_and_material_maps_are_rejected`
   - Mutates one map rule at a time: multiple buckets populated, wrong bucket,
     wrong assignment count, dropped `None`.
   - Covers resource map mismatch.
   - Replaces scattered wrong-bucket and shortened-map validation tests.

7. `resource_references_and_export_remapping_are_validated`
   - Uses `S1` and `D1`.
   - Positive assertions: semantic/material/texture/UV handles resolve; dense
     export remapping is stable.
   - Negative mutations: missing semantic, material, texture, and UV handles.
   - Covers missing references and global-pool-to-dense-output remapping.

8. `dense_texture_maps_are_accepted`
   - Uses `S1`.
   - Asserts texture ring arrays align with boundary rings, UV entries align
     with boundary vertex occurrences, untextured rings carry null placeholders,
     and reused geometric vertices can use different UVs per ring occurrence.
   - Merges current texture acceptance and reused-vertex UV tests.

9. `invalid_texture_topology_or_uv_payload_is_rejected`
   - Mutates `S1`: wrong ring count, wrong ring-texture count, ring start
     mismatch, wrong UV count, UV on untextured ring, null UV on textured ring,
     invalid texture/UV reference.
   - Covers texture map mismatch.

10. `cross_layer_ordering_follows_boundary_traversal`
    - Uses `S1` and `D1` creation paths.
    - Reorders surfaces/rings before insertion and asserts
      semantic/material/texture output follows boundary traversal.
    - Covers the suite doc's cross-layer ordering requirement and the error
      doc's occurrence-level UV edge case.

11. `template_geometry_and_instance_separation`
    - Positive: `T1` validates like regular surface geometry; `I1` stores no
      boundary/maps, references an existing template and regular root vertex,
      and has the expected transform.
    - Negative: missing template, missing reference point, missing
      transform/payload, instance carrying boundary/maps, template inserted into
      regular pool, regular geometry inserted into template pool.
    - Covers incomplete stored geometry and invalid `GeometryInstance`.

12. `draft_authoring_invariants`
    - Add tests in or near `src/v2_0/geometry_draft.rs`.
    - Reject empty required parts, duplicate material/texture themes on one
      surface/ring, missing regular/template vertex references, missing
      semantic/material/texture handles, and missing UV handles.
    - Assert coordinate and UV deduplication during draft insertion.
    - Covers duplicate authoring themes, draft missing references, and
      deduplication edge cases.

13. `wkb_boundary_serialization`
    - Keep WKB broad but table-driven.
    - Positive: supported boundary kinds emit expected little-endian ISO XYZ
      WKB; byte-stability; supported WKB inputs parse; open rings close on
      write; closed legacy rings are not double-closed; solids flatten to
      `MultiPolygonZ`; order is preserved.
    - Negative: empty boundary, inconsistent offsets, missing vertex reference,
      no reachable polygons, polygon with no rings, short rings, big-endian
      input, EWKB flags, unsupported/non-Z/singular types, wrong child types,
      empty multis, zero-ring polygons, unclosed rings, truncated payloads,
      trailing bytes, and count/index overflow where practically constructible.

## Existing Test Disposition

- Retain the earlier delete/keep/rewrite/merge decisions, but implement them
  under the family names above.
- `keep-as-is` means keep the current body/intent and add the required
  docstring.
- Delete low-signal private API tests from `boundary/tests.rs`: `empty`,
  `with_capacity`, `display_boundary_type`, `boundary_counter`, and
  `type_alias_consistency`.
- Rewrite `type_detection` as
  `boundary_type_detection_reports_highest_populated_level`.
- Split `instance_template_and_reference_must_resolve` into precise
  missing-template and missing-reference-point tests.
- Add the missing draft-authoring tests because `src/v2_0/geometry_draft.rs`
  currently has no tests.
- Do not include `tests/v2_0/semantic.rs::semantic_equality` in this cleanup.

## Test Plan

- Run `just fmt-check`.
- Run `just test -p cityjson-types`.
- Run `just test` if shared workspace helpers are touched.
- Run `just ci` before claiming done.

## Assumptions

- Scope is all geometry-related tests in `cityjson-types`.
- `geometry_test_suite.md` defines suite organization;
  `geometry_error_cases.md` defines required coverage inside that organization.
- JSON adapter cases from `geometry_error_cases.md` remain out of scope for this
  crate.
- Preserve existing user changes to
  `crates/cityjson-types/docs/dev/geometry_error_cases.md` and untracked
  `docs-site/` unless explicitly reviewed before editing.
