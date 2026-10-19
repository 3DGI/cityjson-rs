/// Input: the C FFI source file is inspected as text after the plural package API migration.
/// Assertions: plural CityObject/package symbols are present and obsolete feature-level symbols are absent.
#[test]
fn ffi_cityobject_lookup_is_plural_only() {
    let source = include_str!("../src/lib.rs");

    for symbol in [
        "cjx_index_lookup_cityobject_refs",
        "cjx_index_package_refs_for_cityobject",
        "cjx_index_read_package_model_bytes",
        "cjx_index_package_source_paths",
        "cjx_index_read_filtered_packages",
        "cjx_filtered_packages_free",
    ] {
        assert!(
            source.contains(symbol),
            "missing plural package ABI symbol {symbol}"
        );
    }
    for removed in [
        "cjx_index_lookup_feature_refs",
        "cjx_index_get_bytes",
        "cjx_index_get_model_bytes",
        "cjx_index_read_feature_bytes",
        "cjx_index_read_feature_model_bytes",
        "cjx_index_read_filtered_features",
        "cjx_filtered_features_free",
    ] {
        assert!(
            !source.contains(removed),
            "obsolete feature ABI symbol remains: {removed}"
        );
    }
}

/// Input: the C FFI source file is inspected as text for public ref types and association APIs.
/// Assertions: `CityObject` refs, package refs, and package membership lookup are exposed through the ABI.
#[test]
fn ffi_cityobject_relationships_and_associations_match_rust() {
    let source = include_str!("../src/lib.rs");

    assert!(source.contains("cjx_cityobject_ref_t"));
    assert!(source.contains("cjx_package_ref_t"));
    assert!(source.contains("cjx_index_package_refs_for_cityobject"));
}

/// Input: the generated C header for `LoD` selection and diagnostics.
/// Assertions: both exact and missing values are string lists and shared ABI types are included.
#[test]
fn ffi_lod_header_carries_lists() {
    let header = include_str!("../include/cityjson_index/cityjson_index.h");
    assert!(header.contains("cjx_string_list_t exact_lods;"));
    assert!(header.contains("cjx_string_list_t requested_lods;"));
    assert!(header.contains("#include \"cityjson_lib/cityjson_lib.h\""));
}
