# CityJSON Bindings Review for v0.9.0 Release

> Superseded by [`BINDINGS_IMPLEMENTATION_PLAN_v0.10.0.md`](BINDINGS_IMPLEMENTATION_PLAN_v0.10.0.md).
> This review is retained as historical context. It is partly stale: it misses
> the hard-coded `cityjson_lib.__version__ == "0.6.1"` runtime mismatch,
> assumes some APIs exist under wrong names, and under-specifies the current FFI
> authoring patterns.

## Executive Summary

The **cityjson-lib** and **cityjson-index** Python and C++ bindings have **fallen behind** the Rust implementation and require updates before the v0.9.0 minor release. The workspace version is at `0.9.0`, but binding-specific version files are inconsistent and new Rust features are not fully exposed in the FFI layers.

---

## Current State

### Workspace Version
- **Rust workspace**: `0.9.0` (all crates in lockstep)
- **Root CHANGELOG**: Documents extensive changes since v0.8.0

### Binding Versions

| Binding | Current Version | Status | Location |
|---------|----------------|--------|----------|
| cityjson-lib Python | `0.9.0` | ✅ Aligned | `crates/cityjson-lib/ffi/python/pyproject.toml` |
| cityjson-index Python | `0.9.0` | ✅ Aligned | `crates/cityjson-index/ffi/python/pyproject.toml` |
| cityjson-lib C++ | `0.5.3` | ❌ **OUTDATED** | `crates/cityjson-lib/ffi/cpp/CMakeLists.txt` |

### Release Automation
- Python wheels: Automatically built and published via GitHub Actions workflow (`release.yml`)
- Version synchronization: Handled via `cargo-release` with `pre-release-replacements` in crate Cargo.toml files
- C++ bindings: **No automated release process** - manual version bump required

---

## Missing Features in Bindings

### cityjson-lib

#### Rust Features Not Exposed in FFI (C ABI / C++ / Python)

1. **PROJ Support (New in v0.9.0)**
   - Rust: `ops::transformer()` and `ops::reproject()` functions added
   - Rust: `proj` feature flag with `proj-sys` dependency
   - **Status**: ❌ NOT EXPOSED in any FFI binding
   - Impact: Python and C++ users cannot use coordinate reprojection
   - Files: `crates/cityjson-lib/src/proj.rs`, `crates/cityjson-lib/src/ops.rs`

2. **Geometry Editing APIs (New in v0.9.0)**
   - Rust: Validated geometry editing for cloning parts, replacing geometries, building material/semantic maps
   - **Status**: ❌ NOT EXPOSED in FFI
   - Impact: Python and C++ users cannot perform low-level geometry operations

3. **Mutable Vertex Pool Access (New in v0.9.0)**
   - Rust: Public mutable access to CityJSON real-world vertex pools
   - **Status**: ❌ NOT EXPOSED in FFI
   - Impact: Python and C++ users cannot perform in-place coordinate operations

4. **Transform Reconciliation (Changed in v0.9.0)**
   - Rust: `ops::append` and `ops::merge` now accept differing source transforms
   - **Status**: ⚠️ PARTIALLY EXPOSED - basic transform set/clear exposed, but not merge semantics
   - Impact: Python and C++ users cannot use the new transform reconciliation

#### C++ Specific Issues

1. **Version Mismatch**: CMakeLists.txt declares `VERSION 0.5.3` while Rust is at `0.9.0`
2. **Missing PROJ Headers**: No C++ wrappers for PROJ transformer creation or reprojection
3. **No Geometry Editing**: No C++ API for geometry manipulation

### cityjson-index

#### Rust Features Not Exposed in FFI

1. **Aggregate Feature-Bounds Summary API (New in v0.9.0)**
   - Rust: Whole-index bounds and feature counts without scanning feature pages
   - **Status**: ❌ NOT EXPOSED in Python FFI
   - Impact: Python users cannot get index-level summaries efficiently

2. **Batch Reconstruction from Persisted References (New in v0.9.0)**
   - Rust: Batch reconstruction API added
   - **Status**: ❌ NOT EXPOSED in Python FFI
   - Impact: Python users cannot reconstruct from persisted references

3. **Rowid-Ordered Decoded Scan APIs (New in v0.9.0)**
   - Rust: Rowid-ordered scan APIs and rowid-keyed lookup helpers
   - **Status**: ❌ NOT EXPOSED in Python FFI
   - Impact: Python users cannot perform ordered scans

4. **Package-Oriented APIs (Changed in v0.9.0)**
   - Rust: Reworked indexing around valid `CityJSONFeature` packages
   - **Status**: ✅ EXPOSED in Python FFI (via `cjx_index_*` functions)
   - The Python bindings do expose the new package-oriented APIs

---

## Version Synchronization Issues

### Automatic Version Updates
The workspace uses `cargo-release` with `pre-release-replacements`:

```toml
# In crates/cityjson-lib/Cargo.toml
[package.metadata.release]
pre-release-replacements = [
    { file = "ffi/python/pyproject.toml", search = "^version = \"[^\"]+\"$", replace = "version = \"{{version}}\"", exactly = 1, min = 1 },
]
```

```toml
# In crates/cityjson-index/Cargo.toml
[package.metadata.release]
pre-release-replacements = [
    { file = "ffi/python/pyproject.toml", search = "^version = \"[^\"]+\"$", replace = "version = \"{{version}}\"", exactly = 1, min = 1 },
    { file = "ffi/python/pyproject.toml", search = "cityjson-lib==[0-9]+\\.[0-9]+\\.[0-9]+", replace = "cityjson-lib=={{version}}", exactly = 1, min = 1 },
]
```

**Problem**: The C++ bindings (`CMakeLists.txt`) are **NOT** included in the automated version replacement, requiring manual updates.

---

## Required Updates Before v0.9.0 Release

### High Priority (Blocking)

1. **Update C++ binding version**
   - File: `crates/cityjson-lib/ffi/cpp/CMakeLists.txt`
   - Change: `project(cityjson_lib_cpp VERSION 0.5.3 ...)` → `project(cityjson_lib_cpp VERSION 0.9.0 ...)`
   - **Rationale**: Version must match workspace for consistency

2. **Add PROJ support to FFI core**
   - Add `cj_proj_transformer_create()` and `cj_proj_reproject()` to `cityjson-lib-ffi-core`
   - Expose in C++ header (`cityjson_lib.hpp`)
   - Expose in Python (`_ffi.py` and `__init__.py`)
   - **Rationale**: Major feature in v0.9.0, users expect it in bindings

3. **Add geometry editing APIs to FFI core**
   - Expose geometry cloning, replacement, and map building functions
   - **Rationale**: Core functionality for advanced use cases

### Medium Priority (Should Include)

4. **Add mutable vertex pool access to FFI**
   - Expose vertex pool mutation functions
   - **Rationale**: Enables in-place coordinate operations without serialization

5. **Add cityjson-index aggregate summary API to Python FFI**
   - Expose `index_summary()` or similar for whole-index bounds
   - **Rationale**: Performance-critical API for index inspection

6. **Add batch reconstruction API to Python FFI**
   - Expose batch reconstruction from persisted references
   - **Rationale**: Enables efficient batch operations

7. **Add rowid-ordered scan APIs to Python FFI**
   - Expose rowid-keyed lookup and ordered scan functions
   - **Rationale**: Completes the v0.9.0 index feature set

### Low Priority (Nice to Have)

8. **Update C++ README and documentation**
   - Document new features and version alignment
   - **Rationale**: User-facing documentation consistency

---

## Files Requiring Changes

### cityjson-lib

| File | Change Required | Priority |
|------|----------------|----------|
| `crates/cityjson-lib/ffi/cpp/CMakeLists.txt` | Bump version to 0.9.0 | High |
| `crates/cityjson-lib/ffi/core/src/exports.rs` | Add PROJ functions | High |
| `crates/cityjson-lib/ffi/core/src/abi.rs` | Add PROJ types | High |
| `crates/cityjson-lib/ffi/core/src/lib.rs` | Export PROJ functions | High |
| `crates/cityjson-lib/ffi/cpp/include/cityjson_lib/cityjson_lib.hpp` | Add PROJ wrappers | High |
| `crates/cityjson-lib/ffi/python/src/cityjson_lib/_ffi.py` | Add PROJ bindings | High |
| `crates/cityjson-lib/ffi/python/src/cityjson_lib/__init__.py` | Expose PROJ API | High |

### cityjson-index

| File | Change Required | Priority |
|------|----------------|----------|
| `crates/cityjson-index/ffi/core/src/lib.rs` | Add summary/reconstruction APIs | Medium |
| `crates/cityjson-index/ffi/python/src/cityjson_index/_native.py` | Expose new APIs | Medium |
| `crates/cityjson-index/ffi/python/src/cityjson_index/__init__.py` | Add Python wrappers | Medium |

---

## Recommendations

### For v0.9.0 Release

1. **Minimum viable**: Update C++ version to 0.9.0 and add PROJ support to all bindings
2. **Recommended**: Also add geometry editing APIs and cityjson-index summary APIs
3. **Full alignment**: Add all missing v0.9.0 features to bindings

### Process Improvements

1. **Add C++ to release automation**: Extend `pre-release-replacements` to include CMakeLists.txt
   ```toml
   # In crates/cityjson-lib/Cargo.toml
   pre-release-replacements = [
       { file = "ffi/python/pyproject.toml", search = "^version = \"[^\"]+\"$", replace = "version = \"{{version}}\"", exactly = 1, min = 1 },
       { file = "ffi/cpp/CMakeLists.txt", search = "VERSION [0-9]+\.[0-9]+\.[0-9]+", replace = "VERSION {{version}}", exactly = 1, min = 1 },
   ]
   ```

2. **Add C++ release workflow**: Create GitHub Actions workflow for C++ binding releases

3. **Document binding version policy**: Add section to CONTRIBUTING.md explaining binding version alignment

---

## Verification Checklist

Before releasing v0.9.0:

- [ ] C++ CMakeLists.txt version matches workspace (0.9.0)
- [ ] PROJ support exposed in cityjson-lib-ffi-core
- [ ] PROJ support exposed in C++ bindings
- [ ] PROJ support exposed in Python bindings
- [ ] Geometry editing APIs exposed in FFI
- [ ] cityjson-index summary APIs exposed in Python
- [ ] cityjson-index batch reconstruction exposed in Python
- [ ] cityjson-index rowid scan APIs exposed in Python
- [ ] All FFI tests pass
- [ ] Python wheel builds succeed
- [ ] C++ smoke tests pass

---

## Impact Assessment

### If Not Updated
- **Python users**: Will get v0.9.0 package but missing PROJ, geometry editing, and other new features
- **C++ users**: Version confusion (0.5.3 vs 0.9.0), missing all v0.9.0 features
- **Reputation**: Inconsistent release quality, user frustration

### If Updated
- **Effort**: ~2-3 days of focused development
- **Risk**: Medium - FFI changes require careful testing
- **Benefit**: Consistent, complete v0.9.0 release across all languages

---

## Conclusion

The bindings are **not ready** for v0.9.0 release. The **minimum requirement** is updating the C++ version and adding PROJ support to all bindings. For a complete release, all v0.9.0 features should be exposed in the FFI layers.

**Recommendation**: Delay v0.9.0 release until at least the high-priority items are addressed, or release as v0.9.0 with a follow-up v0.9.1 containing the binding updates.
