# cityjson-index FFI Core

The C ABI exposes CityObject and package APIs. `cjx_index_read_filtered_packages`
reconstructs package refs and applies a typed `cjx_package_filter_t`. The function
returns an owned `cjx_filtered_package_t` array. Call
`cjx_filtered_packages_free(packages, count)` exactly once for a returned array.
