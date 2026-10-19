# cityjson-index FFI Core

The C ABI exposes CityObject and package APIs. `cjx_index_package_source_paths` returns an owned UTF-8 byte array in package-ref order; release it with `cjx_bytes_array_free`. `cjx_index_read_filtered_packages`
reconstructs package refs and applies a typed `cjx_package_filter_t`. The function
returns an owned `cjx_filtered_package_t` array. Call
`cjx_filtered_packages_free(packages, count)` exactly once for a returned array.

The generated C header is `include/cityjson_index/cityjson_index.h`; include
both this directory and the cityjson-lib FFI include directory when compiling.
Regenerate it from the workspace with:

```sh
cbindgen crates/cityjson-index/ffi/core --only-target-dependencies \
  --config crates/cityjson-index/ffi/core/cbindgen.toml \
  --output crates/cityjson-index/ffi/core/include/cityjson_index/cityjson_index.h
```

For `CJX_LOD_SELECTION_EXACT`, `cjx_lod_selection_t.exact_lods` is a nonempty
`cjx_string_list_t` of UTF-8 LoDs. Input buffers are borrowed for the call.
Returned `cjx_missing_lod_selection_t.requested_lods` and `available_lods` are
owned string lists; the enclosing `cjx_filtered_packages_free` releases them.
This replaces the previous single-string fields and changes the ABI layout;
recompile callers against the new header.
