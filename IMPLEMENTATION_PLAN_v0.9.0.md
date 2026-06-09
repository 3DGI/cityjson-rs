# Implementation Plan for v0.9.0 Bindings Alignment

> Superseded by [`BINDINGS_IMPLEMENTATION_PLAN_v0.10.0.md`](BINDINGS_IMPLEMENTATION_PLAN_v0.10.0.md).
> This plan is retained as historical context. It is partly stale: it misses
> the hard-coded `cityjson_lib.__version__ == "0.6.1"` runtime mismatch,
> assumes some APIs exist under wrong names, and under-specifies the current FFI
> authoring patterns.

## Overview

This document provides a **complete implementation plan** for aligning the cityjson-lib and cityjson-index Python and C++ bindings with the Rust implementation for the v0.9.0 release, including PROJ support exposure through FFI (Option A: Rust as Facade).

---

## Scope

### In Scope
1. **PROJ Support in FFI** - Expose `reproject()` and `transformer()` through C ABI, C++, and Python bindings
2. **Geometry Editing APIs** - Expose geometry manipulation functions in FFI
3. **Mutable Vertex Pool Access** - Expose vertex pool mutation in FFI
4. **cityjson-index New APIs** - Expose aggregate summary, batch reconstruction, rowid scan APIs in Python FFI
5. **C++ Version Alignment** - Update C++ bindings from 0.5.3 to 0.9.0
6. **CI Testing** - Ensure PROJ functionality is tested on all OSes in all bindings

### Out of Scope
- WASM bindings (per user request)
- Full PROJ API exposure (only coordinate transformation)
- cityjson-index C++ bindings (only Python bindings exist)

---

## Phase 1: PROJ Support in FFI (Highest Priority)

### Objective
Expose the minimal PROJ coordinate transformation functionality through all FFI layers.

### Rust Implementation (cityjson-lib-ffi-core)

#### Files to Modify
- `crates/cityjson-lib/ffi/core/src/abi.rs` - Add PROJ types
- `crates/cityjson-lib/ffi/core/src/exports.rs` - Add PROJ functions
- `crates/cityjson-lib/ffi/core/src/lib.rs` - Export PROJ functions
- `crates/cityjson-lib/ffi/core/Cargo.toml` - Ensure proj feature is available

#### ABI Types (abi.rs)
```rust
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct cj_proj_transformer_t {
    _private: [u8; 0],
}
```

#### Exported Functions (exports.rs)

```rust
/// Create a PROJ transformer for coordinate transformation.
/// 
/// # Safety
/// - `source_crs` and `target_crs` must be valid UTF-8 strings
/// - `out_transformer` must be a valid writable pointer
#[no_mangle]
pub extern "C" fn cj_proj_transformer_create(
    source_crs: *const c_char,
    source_crs_len: usize,
    target_crs: *const c_char,
    target_crs_len: usize,
    out_transformer: *mut *mut cj_proj_transformer_t,
) -> cj_status_t {
    run_ffi(|| {
        let source_crs = required_string(source_crs, source_crs_len, "source_crs")?;
        let target_crs = required_string(target_crs, target_crs_len, "target_crs")?;
        
        // Enable proj feature at compile time
        #[cfg(feature = "proj")]
        {
            let transformer = cityjson_lib::ops::transformer(&source_crs, &target_crs)
                .map_err(|e| AbiError::from_cityjson_lib_error(e))?;
            let boxed = Box::new(transformer);
            let raw = Box::into_raw(boxed).cast::<cj_proj_transformer_t>();
            write_value(out_transformer, "out_transformer", raw);
        }
        #[cfg(not(feature = "proj"))]
        {
            return Err(AbiError::unsupported_operation(
                "PROJ support not enabled. Compile with --features proj"
            ));
        }
        Ok(())
    })
}

/// Free a PROJ transformer.
/// 
/// # Safety
/// - `transformer` must be a valid pointer from cj_proj_transformer_create or null
#[no_mangle]
pub extern "C" fn cj_proj_transformer_free(
    transformer: *mut cj_proj_transformer_t,
) -> cj_status_t {
    run_ffi(|| {
        if transformer.is_null() {
            return Ok(());
        }
        #[cfg(feature = "proj")]
        {
            let raw = unsafe { Box::from_raw(transformer.cast::<cityjson_lib::ops::Transformer>()) };
            drop(raw);
        }
        Ok(())
    })
}

/// Transform a single point using a PROJ transformer.
/// 
/// # Safety
/// - `transformer` must be a valid pointer from cj_proj_transformer_create
/// - `out_x`, `out_y`, `out_z` must be valid writable pointers
#[no_mangle]
pub extern "C" fn cj_proj_transformer_transform(
    transformer: *const cj_proj_transformer_t,
    x: f64,
    y: f64,
    z: f64,
    out_x: *mut f64,
    out_y: *mut f64,
    out_z: *mut f64,
) -> cj_status_t {
    run_ffi(|| {
        let transformer = required_handle(transformer)?;
        #[cfg(feature = "proj")]
        {
            let result = transformer.transform([x, y, z])
                .map_err(|e| AbiError::from_cityjson_lib_error(e))?;
            write_value(out_x, "out_x", result[0]);
            write_value(out_y, "out_y", result[1]);
            write_value(out_z, "out_z", result[2]);
        }
        #[cfg(not(feature = "proj"))]
        {
            return Err(AbiError::unsupported_operation(
                "PROJ support not enabled"
            ));
        }
        Ok(())
    })
}

/// Reproject a CityModel to a new CRS.
/// 
/// Consumes the input model and returns a new model with vertices transformed
/// to the target CRS. The model's referenceSystem is updated.
/// 
/// # Safety
/// - `model` must be a valid model handle
/// - `target_crs` must be a valid UTF-8 string
#[no_mangle]
pub extern "C" fn cj_model_reproject(
    model: *mut cj_model_t,
    target_crs: *const c_char,
    target_crs_len: usize,
) -> cj_status_t {
    run_ffi(|| {
        let model = model_take(model)?;
        let target_crs = required_string(target_crs, target_crs_len, "target_crs")?;
        
        #[cfg(feature = "proj")]
        {
            let result = cityjson_lib::ops::reproject(model, &target_crs)
                .map_err(|e| AbiError::from_cityjson_lib_error(e))?;
            write_value(model, "model", model_into_handle(result));
        }
        #[cfg(not(feature = "proj"))]
        {
            return Err(AbiError::unsupported_operation(
                "PROJ support not enabled. Compile with --features proj"
            ));
        }
        Ok(())
    })
}
```

#### Helper Functions (exports.rs)
```rust
#[cfg(feature = "proj")]
fn required_transformer(
    transformer: *const cj_proj_transformer_t,
) -> Result<&'static cityjson_lib::ops::Transformer, AbiError> {
    let ptr = NonNull::new(transformer.cast_mut())
        .ok_or_else(|| AbiError::invalid_argument("transformer must not be null"))?;
    // SAFETY: The pointer originates from cj_proj_transformer_create
    Ok(unsafe { &*ptr.as_ptr().cast::<cityjson_lib::ops::Transformer>() })
}
```

### C++ Bindings (cityjson-lib-ffi-cpp)

#### Files to Modify
- `crates/cityjson-lib/ffi/cpp/include/cityjson_lib/cityjson_lib.hpp` - Add PROJ wrappers
- `crates/cityjson-lib/ffi/cpp/CMakeLists.txt` - Update version to 0.9.0

#### Header Additions (cityjson_lib.hpp)
```cpp
#pragma once

#include <string>
#include <array>

// Forward declaration
struct cj_proj_transformer_t;

namespace cityjson_lib {

class ProjTransformer;

/// RAII wrapper for a PROJ coordinate transformer.
class ProjTransformer {
public:
    /// Create a transformer between two CRS.
    /// 
    /// @param source_crs Source coordinate reference system (e.g., "EPSG:4326")
    /// @param target_crs Target coordinate reference system (e.g., "EPSG:3857")
    /// @throws std::runtime_error if transformer creation fails
    static ProjTransformer create(const std::string& source_crs, const std::string& target_crs);
    
    ~ProjTransformer();
    
    // Disable copying
    ProjTransformer(const ProjTransformer&) = delete;
    ProjTransformer& operator=(const ProjTransformer&) = delete;
    
    // Enable moving
    ProjTransformer(ProjTransformer&& other) noexcept;
    ProjTransformer& operator=(ProjTransformer&& other) noexcept;
    
    /// Transform a single point.
    /// 
    /// @param point Input point as [x, y, z]
    /// @return Transformed point as [x, y, z]
    /// @throws std::runtime_error if transformation fails
    std::array<double, 3> transform(const std::array<double, 3>& point) const;
    
private:
    explicit ProjTransformer(cj_proj_transformer_t* handle);
    
    cj_proj_transformer_t* handle_;
};

/// Reproject a CityModel to a new CRS.
/// 
/// The model is modified in place. Vertices are transformed to the target CRS,
/// and the model's referenceSystem is updated.
/// 
/// @param model The model to reproject
/// @param target_crs Target coordinate reference system (e.g., "EPSG:3857")
/// @throws std::runtime_error if reprojection fails
void reproject(CityModel& model, const std::string& target_crs);

} // namespace cityjson_lib
```

#### Implementation (cityjson_lib.hpp - inline implementations)
```cpp
inline ProjTransformer::ProjTransformer(cj_proj_transformer_t* handle) 
    : handle_(handle) {}

inline ProjTransformer::~ProjTransformer() {
    if (handle_) {
        cj_proj_transformer_free(handle_);
    }
}

inline ProjTransformer ProjTransformer::create(const std::string& source_crs, const std::string& target_crs) {
    cj_proj_transformer_t* handle = nullptr;
    check_status(cj_proj_transformer_create(
        source_crs.data(), source_crs.size(),
        target_crs.data(), target_crs.size(),
        &handle
    ));
    return ProjTransformer(handle);
}

inline ProjTransformer::ProjTransformer(ProjTransformer&& other) noexcept 
    : handle_(other.handle_) {
    other.handle_ = nullptr;
}

inline ProjTransformer& ProjTransformer::operator=(ProjTransformer&& other) noexcept {
    if (this != &other) {
        if (handle_) {
            cj_proj_transformer_free(handle_);
        }
        handle_ = other.handle_;
        other.handle_ = nullptr;
    }
    return *this;
}

inline std::array<double, 3> ProjTransformer::transform(const std::array<double, 3>& point) const {
    double out_x, out_y, out_z;
    check_status(cj_proj_transformer_transform(
        handle_,
        point[0], point[1], point[2],
        &out_x, &out_y, &out_z
    ));
    return {out_x, out_y, out_z};
}

inline void reproject(CityModel& model, const std::string& target_crs) {
    check_status(cj_model_reproject(
        model.handle_,
        target_crs.data(), target_crs.size()
    ));
}
```

#### CMakeLists.txt Update
```cmake
# Change from 0.5.3 to 0.9.0
project(cityjson_lib_cpp VERSION 0.9.0 LANGUAGES CXX)
```

### Python Bindings (cityjson-lib-ffi-python)

#### Files to Modify
- `crates/cityjson-lib/ffi/python/src/cityjson_lib/_ffi.py` - Add PROJ bindings
- `crates/cityjson-lib/ffi/python/src/cityjson_lib/__init__.py` - Expose PROJ API

#### FFI Layer (_ffi.py)
```python
class ProjTransformerStruct(Structure):
    _fields_ = []

# Add to _lib loading
self._lib.cj_proj_transformer_create.argtypes = [
    c_char_p, c_size_t, c_char_p, c_size_t, POINTER(c_void_p)
]
self._lib.cj_proj_transformer_create.restype = c_int

self._lib.cj_proj_transformer_free.argtypes = [c_void_p]
self._lib.cj_proj_transformer_free.restype = c_int

self._lib.cj_proj_transformer_transform.argtypes = [
    c_void_p, c_double, c_double, c_double, POINTER(c_double), POINTER(c_double), POINTER(c_double)
]
self._lib.cj_proj_transformer_transform.restype = c_int

self._lib.cj_model_reproject.argtypes = [c_void_p, c_char_p, c_size_t]
self._lib.cj_model_reproject.restype = c_int

# Add to _FFILibrary class
def _proj_transformer_create(self, source_crs: str, target_crs: str) -> int:
    handle = c_void_p()
    self._raise_if_error(
        self._lib.cj_proj_transformer_create(
            source_crs.encode('utf-8'), len(source_crs),
            target_crs.encode('utf-8'), len(target_crs),
            byref(handle)
        )
    )
    return handle.value

def _proj_transformer_free(self, handle: int) -> None:
    self._raise_if_error(
        self._lib.cj_proj_transformer_free(c_void_p(handle))
    )

def _proj_transformer_transform(self, handle: int, x: float, y: float, z: float) -> tuple[float, float, float]:
    out_x = c_double()
    out_y = c_double()
    out_z = c_double()
    self._raise_if_error(
        self._lib.cj_proj_transformer_transform(
            c_void_p(handle), x, y, z,
            byref(out_x), byref(out_y), byref(out_z)
        )
    )
    return (out_x.value, out_y.value, out_z.value)

def _model_reproject(self, handle: int, target_crs: str) -> None:
    self._raise_if_error(
        self._lib.cj_model_reproject(
            c_void_p(handle),
            target_crs.encode('utf-8'), len(target_crs)
        )
    )
```

#### Public API (__init__.py)
```python
class ProjTransformer:
    """A coordinate transformer for reprojecting points between CRS.
    
    Example:
        transformer = ProjTransformer.create("EPSG:4326", "EPSG:3857")
        point = transformer.transform([10.0, 20.0, 30.0])
    """
    
    def __init__(self, handle: int, _ffi: '_FFILibrary'):
        self._handle = handle
        self._ffi = _ffi
    
    @staticmethod
    def create(source_crs: str, target_crs: str) -> 'ProjTransformer':
        """Create a transformer between two coordinate reference systems.
        
        Args:
            source_crs: Source CRS (e.g., "EPSG:4326")
            target_crs: Target CRS (e.g., "EPSG:3857")
            
        Returns:
            A new ProjTransformer instance.
            
        Raises:
            RuntimeError: If transformer creation fails.
        """
        handle = _ffi._proj_transformer_create(source_crs, target_crs)
        return ProjTransformer(handle, _ffi)
    
    def __del__(self) -> None:
        if hasattr(self, '_handle') and self._handle is not None:
            self._ffi._proj_transformer_free(self._handle)
            self._handle = None
    
    def transform(self, point: list[float] | tuple[float, float, float]) -> tuple[float, float, float]:
        """Transform a single point from source to target CRS.
        
        Args:
            point: Input point as [x, y, z] or (x, y, z)
            
        Returns:
            Transformed point as (x, y, z)
            
        Raises:
            RuntimeError: If transformation fails.
        """
        if len(point) != 3:
            raise ValueError("Point must have exactly 3 coordinates [x, y, z]")
        return self._ffi._proj_transformer_transform(self._handle, point[0], point[1], point[2])

# Add to CityModel class
def reproject(self, target_crs: str) -> None:
    """Reproject this model's vertices to a new coordinate reference system.
    
    The model's vertices are transformed to the target CRS, and the
    model's referenceSystem metadata is updated. Template vertices and
    geometry-instance transforms are preserved.
    
    Args:
        target_crs: Target CRS (e.g., "EPSG:3857")
        
    Raises:
        RuntimeError: If reprojection fails (e.g., missing source CRS, invalid CRS).
    """
    self._ffi._model_reproject(self._handle, target_crs)
```

---

## Phase 2: Geometry Editing APIs

### Objective
Expose geometry cloning, replacement, and map building functions through FFI.

### Rust Implementation

#### Files to Modify
- `crates/cityjson-lib/ffi/core/src/abi.rs` - Add geometry editing types
- `crates/cityjson-lib/ffi/core/src/exports.rs` - Add geometry editing functions
- `crates/cityjson-lib/ffi/core/src/lib.rs` - Export functions

#### Key Functions to Expose
Based on the CHANGELOG, expose:
1. Geometry part cloning
2. Geometry replacement with handle preservation
3. Material map building
4. Semantic map building

```rust
// In exports.rs
#[no_mangle]
pub extern "C" fn cj_geometry_clone_part(
    model: *const cj_model_t,
    geometry_handle: cj_geometry_id_t,
    part_index: usize,
    out_geometry_handle: *mut cj_geometry_id_t,
) -> cj_status_t {
    // Implementation using cityjson_types APIs
}

#[no_mangle]
pub extern "C" fn cj_geometry_replace(
    model: *mut cj_model_t,
    geometry_handle: cj_geometry_id_t,
    new_geometry: *const cj_geometry_draft_t,
) -> cj_status_t {
    // Implementation
}
```

### C++ and Python Bindings
Follow the same pattern as PROJ - add wrappers in C++ header and Python classes.

---

## Phase 3: Mutable Vertex Pool Access

### Objective
Expose mutable access to CityJSON real-world vertex pools.

### Rust Implementation

```rust
// In exports.rs
#[no_mangle]
pub extern "C" fn cj_model_vertices_mut(
    model: *mut cj_model_t,
    out_vertices: *mut *mut cj_vertex_t,
    out_count: *mut usize,
) -> cj_status_t {
    // Return mutable pointer to vertex pool
    // Note: This is unsafe - caller must not modify while model is in use
}

#[no_mangle]
pub extern "C" fn cj_model_set_vertices(
    model: *mut cj_model_t,
    vertices: *const cj_vertex_t,
    count: usize,
) -> cj_status_t {
    // Replace vertex pool
}
```

---

## Phase 4: cityjson-index New APIs

### Objective
Expose aggregate summary, batch reconstruction, and rowid scan APIs in Python FFI.

### Rust Implementation (cityjson-index-ffi-core)

#### Files to Modify
- `crates/cityjson-index/ffi/core/src/lib.rs` - Add new functions

#### Functions to Add

```rust
// Aggregate feature-bounds summary
#[no_mangle]
pub extern "C" fn cjx_index_summary(
    handle: *const cjx_index_t,
    out_bounds: *mut cjx_bounds3d_t,
    out_feature_count: *mut usize,
) -> cj_status_t {
    // Implementation using CityIndex::summary() or similar
}

// Batch reconstruction from persisted references
#[no_mangle]
pub extern "C" fn cjx_index_reconstruct_batch(
    handle: *const cjx_index_t,
    refs: *const cjx_cityobject_ref_t,
    ref_count: usize,
    out_models: *mut *mut cj_bytes_t,
    out_count: *mut usize,
) -> cj_status_t {
    // Implementation
}

// Rowid-ordered scan
#[no_mangle]
pub extern "C" fn cjx_index_scan_rowid_ordered(
    handle: *const cjx_index_t,
    start_rowid: i64,
    limit: usize,
    out_refs: *mut *mut cjx_cityobject_ref_t,
    out_count: *mut usize,
) -> cj_status_t {
    // Implementation
}
```

### Python Bindings
Add corresponding methods to `CityIndex` class in `__init__.py`:

```python
class CityIndex:
    # Existing methods...
    
    def summary(self) -> tuple[Bounds3D, int]:
        """Get aggregate bounds and feature count for the entire index."""
        bounds = Bounds3D()
        count = c_size_t()
        self._ffi._index_summary(self._handle, byref(bounds), byref(count))
        return (bounds, count.value)
    
    def reconstruct_batch(self, refs: list[CityObjectRef]) -> list[bytes]:
        """Reconstruct multiple CityObjects from persisted references."""
        # Implementation
    
    def scan_rowid_ordered(self, start_rowid: int, limit: int) -> list[CityObjectRef]:
        """Scan CityObjects in rowid order."""
        # Implementation
```

---

## Phase 5: CI Testing for PROJ

### Objective
Ensure PROJ functionality is tested on all OSes (Linux, macOS, Windows) in all bindings.

### GitHub Actions Workflow Updates

#### File: `.github/workflows/ci.yml`

Add PROJ-enabled test jobs:

```yaml
jobs:
  # Existing jobs...
  
  test-ffi-proj:
    name: FFI PROJ Tests
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-14, windows-latest]
    
    steps:
      - uses: actions/checkout@v6
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: 1.93.0
      
      - name: Install PROJ (Linux)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libproj-dev proj-bin
      
      - name: Install PROJ (macOS)
        if: matrix.os == 'macos-14'
        run: |
          brew install proj
      
      - name: Install PROJ (Windows)
        if: matrix.os == 'windows-latest'
        run: |
          vcpkg install proj
          # Or use pre-built binaries
      
      - name: Build with PROJ feature
        run: |
          cargo build --features proj --workspace
          cargo build --features proj,python --package cityjson-lib-ffi-core
          cargo build --features proj --package cityjson-lib-ffi-cpp
      
      - name: Run PROJ tests (Rust)
        run: |
          cargo test --features proj --workspace -- --test-threads=1
      
      - name: Run PROJ tests (Python)
        run: |
          cd crates/cityjson-lib/ffi/python
          pip install -e .
          python -c "
            from cityjson_lib import CityModel, ProjTransformer
            # Test transformer creation
            t = ProjTransformer.create('EPSG:4326', 'EPSG:3857')
            point = t.transform([10.0, 20.0, 30.0])
            print(f'Transformed point: {point}')
            # Test model reprojection
            model = CityModel.parse_document_bytes(b'{\"type\":\"CityJSON\",\"version\":\"2.0\",\"metadata\":{\"referenceSystem\":{\"vertical\":\"EPSG:4326\"}},\"CityObjects\":{},\"vertices\":[[0,0,0]]}')
            model.reproject('EPSG:3857')
            print('Reprojection successful')
          "
      
      - name: Run PROJ tests (C++)
        run: |
          cd crates/cityjson-lib/ffi/cpp
          cmake -B build -DCMAKE_BUILD_TYPE=Release
          cmake --build build
          # Run C++ smoke test with PROJ
          ./build/cityjson_lib_cpp_smoke
          # Add PROJ-specific test
          ./build/cityjson_lib_proj_test
```

### New Test Files

#### Rust Tests (cityjson-lib-ffi-core)
```rust
// tests/proj.rs
#[cfg(feature = "proj")]
#[test]
fn test_proj_transformer_create() {
    use std::ffi::CString;
    
    let source = CString::new("EPSG:4326").unwrap();
    let target = CString::new("EPSG:3857").unwrap();
    
    let mut transformer = ptr::null_mut();
    unsafe {
        assert_eq!(
            cj_proj_transformer_create(
                source.as_ptr(),
                source.as_bytes().len(),
                target.as_ptr(),
                target.as_bytes().len(),
                &mut transformer
            ),
            cj_status_t::CJ_STATUS_SUCCESS
        );
        assert!(!transformer.is_null());
        
        // Test transformation
        let mut out_x = 0.0;
        let mut out_y = 0.0;
        let mut out_z = 0.0;
        assert_eq!(
            cj_proj_transformer_transform(
                transformer,
                10.0, 20.0, 30.0,
                &mut out_x,
                &mut out_y,
                &mut out_z
            ),
            cj_status_t::CJ_STATUS_SUCCESS
        );
        
        // Clean up
        assert_eq!(
            cj_proj_transformer_free(transformer),
            cj_status_t::CJ_STATUS_SUCCESS
        );
    }
}

#[cfg(feature = "proj")]
#[test]
fn test_model_reproject() {
    // Create a simple model with EPSG:4326
    let json = r#"{
        "type": "CityJSON",
        "version": "2.0",
        "metadata": {
            "referenceSystem": {"vertical": "EPSG:4326"}
        },
        "CityObjects": {},
        "vertices": [[10.0, 20.0, 30.0]]
    }"#;
    
    let mut model = ptr::null_mut();
    unsafe {
        // Parse model
        let bytes = cj_bytes_t {
            data: json.as_ptr() as *const c_char,
            len: json.len(),
        };
        assert_eq!(
            cj_model_parse_json(bytes, &mut model),
            cj_status_t::CJ_STATUS_SUCCESS
        );
        
        // Reproject
        let target = CString::new("EPSG:3857").unwrap();
        assert_eq!(
            cj_model_reproject(model, target.as_ptr(), target.as_bytes().len()),
            cj_status_t::CJ_STATUS_SUCCESS
        );
        
        // Clean up
        assert_eq!(
            cj_model_free(model),
            cj_status_t::CJ_STATUS_SUCCESS
        );
    }
}
```

#### C++ Tests
```cpp
// tests/test_proj.cpp
#include <cityjson_lib/cityjson_lib.hpp>
#include <cassert>
#include <iostream>

int main() {
    // Test ProjTransformer
    try {
        auto transformer = cityjson_lib::ProjTransformer::create("EPSG:4326", "EPSG:3857");
        auto result = transformer.transform({10.0, 20.0, 30.0});
        std::cout << "Transformed: " << result[0] << ", " << result[1] << ", " << result[2] << std::endl;
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }
    
    // Test model reprojection
    try {
        auto model = cityjson_lib::CityModel::parse_json(R"({
            "type": "CityJSON",
            "version": "2.0",
            "metadata": {"referenceSystem": {"vertical": "EPSG:4326"}},
            "CityObjects": {},
            "vertices": [[10.0, 20.0, 30.0]]
        })"");
        model.reproject("EPSG:3857");
        std::cout << "Model reprojection successful" << std::endl;
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }
    
    return 0;
}
```

#### Python Tests
```python
# tests/test_proj.py
import pytest
from cityjson_lib import CityModel, ProjTransformer

class TestProjTransformer:
    def test_create_transformer(self):
        """Test creating a PROJ transformer."""
        t = ProjTransformer.create("EPSG:4326", "EPSG:3857")
        assert t is not None
    
    def test_transform_point(self):
        """Test transforming a point."""
        t = ProjTransformer.create("EPSG:4326", "EPSG:3857")
        point = t.transform([10.0, 20.0, 30.0])
        assert len(point) == 3
        assert isinstance(point[0], float)
        # Verify transformation is non-identity
        assert point != (10.0, 20.0, 30.0)
    
    def test_invalid_crs(self):
        """Test that invalid CRS raises an error."""
        with pytest.raises(RuntimeError):
            ProjTransformer.create("EPSG:0", "EPSG:3857")


class TestModelReproject:
    def test_reproject_model(self):
        """Test reprojecting a model."""
        json = b'''{
            "type": "CityJSON",
            "version": "2.0",
            "metadata": {"referenceSystem": {"vertical": "EPSG:4326"}},
            "CityObjects": {},
            "vertices": [[10.0, 20.0, 30.0]]
        }'''
        model = CityModel.parse_document_bytes(json)
        model.reproject("EPSG:3857")
        # Model should be valid after reprojection
        summary = model.summary()
        assert summary.vertex_count == 1
    
    def test_reproject_missing_reference_system(self):
        """Test that missing referenceSystem raises an error."""
        json = b'''{
            "type": "CityJSON",
            "version": "2.0",
            "metadata": {},
            "CityObjects": {},
            "vertices": [[10.0, 20.0, 30.0]]
        }'''
        model = CityModel.parse_document_bytes(json)
        with pytest.raises(RuntimeError):
            model.reproject("EPSG:3857")
```

---

## Phase 6: Release Automation Updates

### Update cargo-release Configuration

#### File: `crates/cityjson-lib/Cargo.toml`
```toml
[package.metadata.release]
pre-release-replacements = [
    { file = "ffi/python/pyproject.toml", search = "^version = \"[^\"]+\"$", replace = "version = \"{{version}}\"", exactly = 1, min = 1 },
    { file = "ffi/cpp/CMakeLists.txt", search = "VERSION [0-9]+\.[0-9]+\.[0-9]+", replace = "VERSION {{version}}", exactly = 1, min = 1 },
]
```

---

## Implementation Timeline

| Phase | Task | Estimated Duration | Priority |
|-------|------|-------------------|----------|
| 1 | PROJ FFI Core (Rust) | 1 day | High |
| 1 | PROJ C++ Bindings | 0.5 day | High |
| 1 | PROJ Python Bindings | 0.5 day | High |
| 1 | PROJ CI Tests | 0.5 day | High |
| 2 | Geometry Editing FFI | 1 day | Medium |
| 3 | Mutable Vertex Pool FFI | 0.5 day | Medium |
| 4 | cityjson-index New APIs | 1 day | Medium |
| 5 | C++ Version Update | 0.25 day | High |
| 6 | Integration Testing | 1 day | High |
| **Total** | | **~6-7 days** | |

---

## Verification Checklist

### Before Merging
- [ ] All PROJ FFI functions compile on all platforms
- [ ] C++ bindings compile with PROJ support
- [ ] Python bindings compile with PROJ support
- [ ] PROJ tests pass on Linux
- [ ] PROJ tests pass on macOS
- [ ] PROJ tests pass on Windows
- [ ] Geometry editing APIs exposed and tested
- [ ] Mutable vertex pool access exposed and tested
- [ ] cityjson-index new APIs exposed and tested
- [ ] C++ version updated to 0.9.0
- [ ] Release automation updated

### Before Release
- [ ] All CI workflows pass
- [ ] Python wheels build successfully with PROJ feature
- [ ] C++ smoke tests pass on all platforms
- [ ] Documentation updated for new features
- [ ] CHANGELOG updated with binding changes

---

## Risk Assessment

### Technical Risks
1. **PROJ Dependency**: Users must have PROJ installed. Mitigation: Clear documentation, CI testing.
2. **FFI Complexity**: Exposing PROJ through FFI. Mitigation: Minimal API surface, thorough testing.
3. **Cross-Platform**: PROJ installation varies by OS. Mitigation: CI testing on all platforms.

### Schedule Risks
1. **PROJ Build Issues**: May take time to resolve on all platforms. Mitigation: Start with Phase 1.
2. **Testing Complexity**: PROJ tests may reveal issues. Mitigation: Incremental testing.

### Resource Risks
1. **Developer Availability**: Requires Rust + C/C++ + Python expertise. Mitigation: Clear implementation plan.

---

## Success Criteria

1. **Functional**: All v0.9.0 Rust features are exposed in Python and C++ bindings
2. **Reliable**: All tests pass on all supported platforms (Linux, macOS, Windows)
3. **Usable**: Users can install and use PROJ functionality without errors
4. **Maintainable**: Code follows existing patterns and conventions

---

## Appendix: File Changes Summary

### cityjson-lib
- `crates/cityjson-lib/ffi/core/src/abi.rs` - Add PROJ types
- `crates/cityjson-lib/ffi/core/src/exports.rs` - Add PROJ functions + geometry editing + vertex pool
- `crates/cityjson-lib/ffi/core/src/lib.rs` - Export new functions
- `crates/cityjson-lib/ffi/core/Cargo.toml` - Verify proj feature
- `crates/cityjson-lib/ffi/cpp/CMakeLists.txt` - Update version to 0.9.0
- `crates/cityjson-lib/ffi/cpp/include/cityjson_lib/cityjson_lib.hpp` - Add PROJ wrappers
- `crates/cityjson-lib/ffi/python/src/cityjson_lib/_ffi.py` - Add PROJ bindings
- `crates/cityjson-lib/ffi/python/src/cityjson_lib/__init__.py` - Expose PROJ API
- `crates/cityjson-lib/ffi/core/tests/proj.rs` - Add PROJ tests

### cityjson-index
- `crates/cityjson-index/ffi/core/src/lib.rs` - Add summary/reconstruction/scan APIs
- `crates/cityjson-index/ffi/python/src/cityjson_index/_native.py` - Expose new APIs
- `crates/cityjson-index/ffi/python/src/cityjson_index/__init__.py` - Add Python wrappers

### CI/Release
- `.github/workflows/ci.yml` - Add PROJ test jobs
- `crates/cityjson-lib/Cargo.toml` - Add CMakeLists.txt to pre-release-replacements

---

## Next Steps

1. **Start with Phase 1 (PROJ)**: This is the highest priority and has dependencies
2. **Implement incrementally**: Commit each function as it's completed
3. **Test on all platforms**: Use GitHub Actions for cross-platform verification
4. **Document progress**: Update this plan with actual vs. estimated durations
