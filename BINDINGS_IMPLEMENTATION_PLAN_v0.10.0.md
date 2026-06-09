# Bindings Implementation Plan for v0.10.0

This is the source of truth for aligning the Python and C++ bindings before the
v0.10.0 release. The older `BINDINGS_REVIEW_v0.9.0.md` and
`IMPLEMENTATION_PLAN_v0.9.0.md` files are superseded and retained only for
historical context.

## Scope

- Target full release parity for `cityjson-lib` Python and C++ bindings.
- Target full release parity for `cityjson-index` Python bindings.
- Keep WASM out of scope.
- Keep `cityjson-index` C++ bindings out of scope because that crate currently
  ships Python bindings only.
- Keep PROJ optional. Binding builds without the `proj` feature must return a
  clear unsupported-operation error for PROJ-only calls.

## Known v0.9 Document Staleness

- `cityjson-lib` Python runtime metadata was hard-coded as
  `cityjson_lib.__version__ == "0.6.1"` even though package metadata is
  release-aligned.
- Some APIs described in the v0.9 documents either already exist under
  different binding names or are represented by the current draft-authoring
  APIs.
- Current FFI patterns favor validated draft handles, copied output buffers, and
  opaque owned handles. New APIs should follow those patterns instead of
  exposing borrowed mutable slices.

## Required Work

### Version Alignment

- Keep `crates/cityjson-lib/ffi/cpp/CMakeLists.txt` aligned with the Rust
  workspace release line.
- Include the C++ CMake version in `cityjson-lib` cargo-release replacements.
- Derive `cityjson_lib.__version__` from installed package metadata at runtime.

### `cityjson-lib` FFI Parity

- Add a `proj` feature to `cityjson-lib-ffi-core` that enables
  `cityjson-lib/proj`.
- Expose C ABI functions for `Transformer` create/free/transform and model
  reprojection.
- Add Python wrappers for point transformation and `CityModel.reproject`.
- Add C++ RAII wrappers for transformer lifecycle and reprojection.
- Expose vertex and template-vertex mutation through set-by-index APIs while
  keeping existing copy APIs unchanged.
- Preserve current geometry authoring patterns: validated geometry drafts,
  semantic/material assignment through draft APIs, and geometry replacement by
  validated handle where needed.

### `cityjson-index` Python Parity

- Expose aggregate feature-bounds summary if the Rust API is present.
- Expose rowid pagination:
  `OpenedIndex.package_ref_page_after_record_id(...)` and
  `OpenedIndex.cityobject_ref_page_after_record_id(...)`.
- Expose bbox pagination for packages and CityObjects.
- Expose record-id lookup/read helpers:
  `lookup_package_ref_by_record_id(...)` and
  `read_package_by_record_id(...)`.
- Expose batch reads as `read_packages(refs)`, preserving input order.

## Verification

Run the focused checks first, then the full workspace gate:

- `just ffi test`
- `just test-python`
- `just fmt-check`
- `just lint`
- `just check`
- `just test`
- `just doc`
- `just ci`
