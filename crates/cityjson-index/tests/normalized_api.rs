mod common;

use std::collections::BTreeSet;

use cityjson_index::{
    BBox, Bounds3D, CityIndex, IndexedCityObjectRef, IndexedPackageRef, LodSelection,
    PackageFilter, PackageFilterReport,
};
use common::{
    cityjson_feature, hierarchy_cityobjects, hierarchy_vertices, open_cityjson_index,
    open_cityjson_seq_index, open_feature_files_index, shared_child_cityobjects, temp_index_path,
    write_cityjson_fixture, write_cityjson_seq_fixture, write_feature_files_fixture,
};
use serde_json::{Value, json};

#[test]
fn plural_package_api_signatures_are_stable() {
    let _: Option<Bounds3D> = None;
    let _: fn(&CityIndex, &str) -> cityjson_lib::Result<Vec<IndexedCityObjectRef>> =
        CityIndex::lookup_cityobject_refs;
    let _: fn(&CityIndex, &IndexedCityObjectRef) -> cityjson_lib::Result<Vec<IndexedPackageRef>> =
        CityIndex::package_refs_for_cityobject;
    let _: fn(&CityIndex, &str) -> cityjson_lib::Result<Vec<cityjson_lib::CityModel>> =
        CityIndex::get_packages;
}

#[test]
fn lookup_cityobject_refs_returns_all_duplicate_occurrences() {
    let index = duplicate_cityjson_seq_index();
    let refs = index
        .lookup_cityobject_refs("duplicate")
        .expect("plural CityObject lookup should succeed");

    assert_eq!(refs.len(), 2);
    assert!(refs[0].record_id < refs[1].record_id);
    assert_eq!(refs[0].external_id, "duplicate");
    assert_eq!(refs[1].external_id, "duplicate");
}

#[test]
fn get_packages_returns_all_distinct_containing_packages() {
    let index = shared_child_index();
    let packages = index
        .get_packages("shared-part")
        .expect("shared child retrieval should succeed");

    assert_eq!(packages.len(), 2);
    assert!(
        packages
            .iter()
            .all(|model| model_contains_id(model, "shared-part"))
    );
}

#[test]
fn read_cityobject_packages_returns_all_shared_memberships() {
    let index = shared_child_index();
    let cityobject = index
        .lookup_cityobject_refs("shared-part")
        .expect("shared child lookup should succeed")
        .into_iter()
        .next()
        .expect("shared child should be indexed");
    let packages = index
        .read_cityobject_packages(&cityobject)
        .expect("shared child packages should reconstruct");

    assert_eq!(packages.len(), 2);
}

#[test]
fn cityjson_seq_read_package_preserves_original_feature_id() {
    let index = duplicate_cityjson_seq_index();
    let cityobject = index
        .lookup_cityobject_refs("duplicate")
        .expect("lookup")
        .remove(0);
    let package = index
        .package_refs_for_cityobject(&cityobject)
        .expect("package refs")
        .remove(0);
    let model = index
        .read_package(&package)
        .expect("package should reconstruct");

    assert_eq!(model_json(&model)["id"], "first");
}

#[test]
fn feature_files_read_package_preserves_original_feature_id() {
    let root = write_feature_files_fixture(
        "feature-files-preserve-id",
        &cityjson_feature(
            "feature-root",
            hierarchy_cityobjects(),
            hierarchy_vertices(),
        ),
    );
    let index_path = temp_index_path("feature-files-preserve-id");
    let mut index = open_feature_files_index(&root, &index_path);
    index.reindex().expect("feature files should reindex");
    let cityobject = index
        .lookup_cityobject_refs("part")
        .expect("lookup")
        .remove(0);
    let package = index
        .package_refs_for_cityobject(&cityobject)
        .expect("package refs")
        .remove(0);
    let model = index
        .read_package(&package)
        .expect("package should reconstruct");

    assert_eq!(model_json(&model)["id"], "feature-root");
}

#[test]
fn cityjson_read_package_localizes_vertices_and_prunes_external_links() {
    let index = shared_child_index();
    let cityobject = index
        .lookup_cityobject_refs("shared-part")
        .expect("lookup")
        .remove(0);
    let package = index
        .package_refs_for_cityobject(&cityobject)
        .expect("packages")
        .remove(0);
    let model = index
        .read_package(&package)
        .expect("synthetic package should reconstruct");
    let value = model_json(&model);

    assert_eq!(value["type"], "CityJSONFeature");
    assert!(
        value["vertices"]
            .as_array()
            .is_some_and(|vertices| vertices.len() == 3)
    );
    assert!(
        value["CityObjects"]["shared-part"]["parents"]
            .as_array()
            .is_some_and(|parents| parents.len() == 1)
    );
}

#[test]
fn package_query_returns_each_package_once() {
    let index = hierarchy_index();
    let hits = index
        .query_package_refs(&query_bounds())
        .expect("package query should succeed");

    assert_eq!(hits.len(), 1);
}

#[test]
fn cityobject_query_returns_granular_hits() {
    let index = hierarchy_index();
    let hits = index
        .query_cityobject_refs(&query_bounds())
        .expect("CityObject query should succeed");

    assert_eq!(
        hits.iter()
            .map(|hit| hit.external_id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["building", "part"])
    );
}

#[test]
fn descendant_cityobject_refs_are_cycle_safe_and_deterministic() {
    let index = hierarchy_index();
    let root = index
        .lookup_cityobject_refs("building")
        .expect("lookup")
        .remove(0);
    let first = index
        .descendant_cityobject_refs(&root)
        .expect("descendant traversal");
    let second = index
        .descendant_cityobject_refs(&root)
        .expect("repeat descendant traversal");

    assert_eq!(first, second);
    assert_eq!(
        first
            .iter()
            .map(|item| item.external_id.as_str())
            .collect::<Vec<_>>(),
        ["part"]
    );
}

#[test]
fn read_packages_decodes_duplicate_request_once_per_package() {
    let index = hierarchy_index();
    let cityobject = index
        .lookup_cityobject_refs("part")
        .expect("lookup")
        .remove(0);
    let package = index
        .package_refs_for_cityobject(&cityobject)
        .expect("packages")
        .remove(0);
    let packages = index
        .read_packages(&[package.clone(), package])
        .expect("batch package read");

    assert_eq!(
        packages.len(),
        2,
        "batch output remains aligned with requested refs"
    );
}

#[test]
fn package_type_prefilter_excludes_irrelevant_packages_before_decode() {
    let index = hierarchy_index();
    let cityobject = index
        .lookup_cityobject_refs("part")
        .expect("lookup")
        .remove(0);
    let package = index
        .package_refs_for_cityobject(&cityobject)
        .expect("packages")
        .remove(0);
    let filter = PackageFilter {
        cityobject_types: Some(BTreeSet::from(["Road".to_owned()])),
        ..PackageFilter::default()
    };
    let outcome = index
        .read_filtered_packages(&[package], &filter)
        .expect("filtered package read")
        .remove(0);

    assert!(outcome.model.is_none());
    assert_eq!(outcome.report.ignored_package_count, 1);
}

#[test]
fn package_filter_no_match_returns_none_model_with_report() {
    let index = hierarchy_index();
    let cityobject = index
        .lookup_cityobject_refs("part")
        .expect("lookup")
        .remove(0);
    let package = index
        .package_refs_for_cityobject(&cityobject)
        .expect("packages")
        .remove(0);
    let filter = PackageFilter {
        cityobject_types: Some(BTreeSet::from(["WaterBody".to_owned()])),
        ..PackageFilter::default()
    };
    let outcome = index
        .read_filtered_packages(&[package], &filter)
        .expect("filtered package read")
        .remove(0);

    assert!(outcome.model.is_none());
    assert!(outcome.report.available_types.contains("BuildingPart"));
}

#[test]
fn package_filter_reports_merge_for_batch_lod_validation() {
    let index = hierarchy_index();
    let cityobject = index
        .lookup_cityobject_refs("part")
        .expect("lookup")
        .remove(0);
    let package = index
        .package_refs_for_cityobject(&cityobject)
        .expect("packages")
        .remove(0);
    let filter = PackageFilter {
        default_lod: LodSelection::Exact("9.9".to_owned()),
        ..PackageFilter::default()
    };
    let outcome = index
        .read_filtered_packages(&[package], &filter)
        .expect("filtered package read")
        .remove(0);
    let mut report = PackageFilterReport::default();
    report.merge(&outcome.report);

    assert!(outcome.model.is_none());
    assert!(!report.missing_lods.is_empty());
}

fn hierarchy_index() -> CityIndex {
    let index_path = temp_index_path("normalized-api-hierarchy");
    let root = write_cityjson_fixture(
        "normalized-api-hierarchy",
        hierarchy_cityobjects(),
        hierarchy_vertices(),
    );
    let mut index = open_cityjson_index(&root, &index_path);
    index.reindex().expect("hierarchy should reindex");
    index
}

fn shared_child_index() -> CityIndex {
    let index_path = temp_index_path("normalized-api-shared-child");
    let root = write_cityjson_fixture(
        "normalized-api-shared-child",
        shared_child_cityobjects(),
        hierarchy_vertices(),
    );
    let mut index = open_cityjson_index(&root, &index_path);
    index.reindex().expect("shared child should reindex");
    index
}

fn duplicate_cityjson_seq_index() -> CityIndex {
    let index_path = temp_index_path("normalized-api-duplicates");
    let root = write_cityjson_seq_fixture(
        "normalized-api-duplicates",
        &[
            cityjson_feature(
                "first",
                json!({"duplicate": {"type": "Building"}}),
                json!([]),
            ),
            cityjson_feature(
                "second",
                json!({"duplicate": {"type": "BuildingPart"}}),
                json!([]),
            ),
        ],
    );
    let mut index = open_cityjson_seq_index(&root, &index_path);
    index.reindex().expect("duplicates should reindex");
    index
}

fn query_bounds() -> BBox {
    BBox {
        min_x: 0.0,
        max_x: 100.0,
        min_y: 0.0,
        max_y: 100.0,
    }
}

fn model_contains_id(model: &cityjson_lib::CityModel, id: &str) -> bool {
    model_json(model)["CityObjects"]
        .as_object()
        .is_some_and(|objects| objects.contains_key(id))
}

fn model_json(model: &cityjson_lib::CityModel) -> Value {
    serde_json::from_str(&cityjson_lib::json::to_string(model).expect("model should serialize"))
        .expect("serialized model should parse")
}
