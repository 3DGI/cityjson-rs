#[test]
fn ffi_cityobject_lookup_is_plural_only() {
    let source = include_str!("../src/lib.rs");

    for symbol in [
        "cjx_index_lookup_cityobject_refs",
        "cjx_index_package_refs_for_cityobject",
        "cjx_index_read_package_model_bytes",
        "cjx_index_read_filtered_packages",
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
    ] {
        assert!(
            !source.contains(removed),
            "obsolete feature ABI symbol remains: {removed}"
        );
    }
}

#[test]
fn ffi_cityobject_relationships_and_associations_match_rust() {
    let source = include_str!("../src/lib.rs");

    assert!(source.contains("cjx_cityobject_ref_t"));
    assert!(source.contains("cjx_package_ref_t"));
    assert!(source.contains("cjx_index_package_refs_for_cityobject"));
}
