#![allow(
    clippy::doc_markdown,
    reason = "test docstrings use domain terminology plainly"
)]

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

type CityObjectBBoxPageFn =
    fn(&CityIndex, &BBox, Option<i64>, usize) -> cityjson_lib::Result<Vec<IndexedCityObjectRef>>;

/// Input: the expected future public Rust API types and method names are referenced as function pointers.
/// Assertions: plural CityObject lookup, package membership lookup, and package retrieval signatures compile exactly as planned.
#[test]
fn plural_package_api_signatures_are_stable() {
    let _: Option<Bounds3D> = None;
    let _: fn(&CityIndex, &str) -> cityjson_lib::Result<Vec<IndexedCityObjectRef>> =
        CityIndex::lookup_cityobject_refs;
    let _: fn(&CityIndex, Option<i64>, usize) -> cityjson_lib::Result<Vec<IndexedCityObjectRef>> =
        CityIndex::cityobject_ref_page_after_record_id;
    let _: CityObjectBBoxPageFn = CityIndex::query_cityobject_refs_page;
    let _: fn(&CityIndex, &IndexedCityObjectRef) -> cityjson_lib::Result<Vec<IndexedPackageRef>> =
        CityIndex::package_refs_for_cityobject;
    let _: fn(&CityIndex, &str) -> cityjson_lib::Result<Vec<cityjson_lib::CityModel>> =
        CityIndex::get_packages;
}

/// Input: a CityJSONSeq index with two physical CityObjects sharing the external id duplicate.
/// Assertions: lookup_cityobject_refs returns both occurrences ordered by CityObject record id.
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

/// Input: duplicate and missing external ids in one batch lookup.
/// Assertions: batch lookup deduplicates input ids, returns all physical matches, and keeps record-id order.
#[test]
fn lookup_cityobject_refs_for_ids_batches_duplicates_and_missing_ids() {
    let index = duplicate_cityjson_seq_index();
    let refs = index
        .lookup_cityobject_refs_for_ids(&["missing", "duplicate", "duplicate"])
        .expect("batch CityObject lookup should succeed");

    assert_eq!(refs.len(), 2);
    assert!(refs[0].record_id < refs[1].record_id);
    assert!(refs.iter().all(|item| item.external_id == "duplicate"));
}

/// Input: a CityJSON index with two root packages sharing one child CityObject.
/// Assertions: get_packages for the shared child returns both containing packages exactly once.
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

/// Input: one indexed shared-child CityObject occurrence with memberships in two packages.
/// Assertions: read_cityobject_packages reconstructs both valid packages containing that occurrence.
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

/// Input: a CityJSONSeq package ref found through a duplicate CityObject occurrence.
/// Assertions: read_package returns a valid feature whose original feature id is preserved.
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

/// Input: a feature-files package ref found through a child CityObject occurrence.
/// Assertions: read_package returns a valid feature whose standalone feature id is preserved.
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

/// Input: a synthetic CityJSON package reconstructed from one root in a shared-child hierarchy.
/// Assertions: the result is a CityJSONFeature with localized vertices and only in-package parent links retained.
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

/// Input: a bbox query intersecting multiple CityObjects inside one indexed package.
/// Assertions: query_package_refs returns the containing package once, not once per matching CityObject.
#[test]
fn package_query_returns_each_package_once() {
    let index = hierarchy_index();
    let hits = index
        .query_package_refs(&query_bounds())
        .expect("package query should succeed");

    assert_eq!(hits.len(), 1);
}

/// Input: a bbox query intersecting both parent and child CityObjects in a hierarchy.
/// Assertions: query_cityobject_refs returns granular refs for both matching CityObjects.
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

/// Input: a CityObject bbox query collected through one-row pages.
/// Assertions: paged CityObject bbox refs are identical to the eager query result.
#[test]
fn cityobject_bbox_pages_match_eager_query() {
    let index = hierarchy_index();
    let eager = index
        .query_cityobject_refs(&query_bounds())
        .expect("eager CityObject query should succeed");
    let mut paged = Vec::new();
    let mut after_record_id = None;
    loop {
        let page = index
            .query_cityobject_refs_page(&query_bounds(), after_record_id, 1)
            .expect("paged CityObject query should succeed");
        if page.is_empty() {
            break;
        }
        after_record_id = page.last().map(|cityobject| cityobject.record_id);
        paged.extend(page);
    }

    assert_eq!(paged, eager);
}

/// Input: a full CityObject scan collected through one-row pages.
/// Assertions: keyset paging returns every indexed CityObject exactly once in record-id order.
#[test]
fn cityobject_ref_pages_iterate_large_sets_by_record_id() {
    let index = hierarchy_index();
    let mut refs = Vec::new();
    let mut after_record_id = None;
    loop {
        let page = index
            .cityobject_ref_page_after_record_id(after_record_id, 1)
            .expect("CityObject ref page should succeed");
        if page.is_empty() {
            break;
        }
        after_record_id = page.last().map(|cityobject| cityobject.record_id);
        refs.extend(page);
    }

    assert_eq!(
        refs.len(),
        index.cityobject_count().expect("CityObject count")
    );
    assert!(
        refs.windows(2)
            .all(|pair| pair[0].record_id < pair[1].record_id)
    );
}

/// Input: a root CityObject ref in a deterministic multi-level hierarchy.
/// Assertions: descendant traversal is stable across calls, breadth-first, and deduplicated.
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

/// Input: the same package ref requested twice in one batch read.
/// Assertions: read_packages preserves request alignment while allowing the implementation to decode each distinct package once.
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

/// Input: two distinct regular CityJSON package refs from the same source.
/// Assertions: batch package reads preserve request order while reconstructing both packages.
#[test]
fn read_packages_batches_distinct_cityjson_packages_from_one_source() {
    let index = shared_child_index();
    let refs = index
        .lookup_cityobject_refs_for_ids(&["building-b", "building-a"])
        .expect("batch lookup")
        .into_iter()
        .flat_map(|cityobject| {
            index
                .package_refs_for_cityobject(&cityobject)
                .expect("package refs")
        })
        .collect::<Vec<_>>();
    let packages = index.read_packages(&refs).expect("batch package read");

    assert_eq!(packages.len(), 2);
    assert_eq!(packages[0].reference.record_id, refs[0].record_id);
    assert_eq!(packages[1].reference.record_id, refs[1].record_id);
    assert!(
        packages
            .iter()
            .all(|package| model_contains_id(&package.model, "shared-part"))
    );
}

/// Input: a BuildingPart package filtered for the Road CityObject type.
/// Assertions: read_filtered_packages returns an aligned no-model outcome and reports one ignored package.
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

/// Input: a BuildingPart package filtered for WaterBody.
/// Assertions: the aligned filter outcome has model None and still reports available input types.
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

/// Input: a package filtered with an exact LoD that no retained geometry provides.
/// Assertions: the batch report merge preserves a no-model outcome and records missing LoD diagnostics.
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
        default_lod: LodSelection::Exact(vec!["9.9".to_owned()]),
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

/// Input: package refs from all three storage layouts, including duplicates and a missing record.
/// Assertions: source paths match persisted backing files, preserve order/duplicates, and missing ids error.
#[test]
fn package_source_paths_cover_layouts_order_duplicates_and_missing_records() {
    let feature = cityjson_feature(
        "fixture",
        json!({"fixture": {"type": "Building"}}),
        json!([]),
    );
    let cases = [
        (
            write_cityjson_fixture(
                "source-path-cityjson",
                json!({"fixture": {"type": "Building"}}),
                json!([]),
            ),
            "fixture.city.json",
            open_cityjson_index as fn(&std::path::Path, &std::path::Path) -> CityIndex,
        ),
        (
            write_cityjson_seq_fixture("source-path-seq", std::slice::from_ref(&feature)),
            "fixture.city.jsonl",
            open_cityjson_seq_index,
        ),
        (
            write_feature_files_fixture("source-path-features", &feature),
            "features/fixture.city.jsonl",
            open_feature_files_index,
        ),
    ];

    for (root, relative_path, open) in cases {
        let index_path = temp_index_path("source-paths");
        let mut index = open(&root, &index_path);
        index.reindex().expect("fixture should reindex");
        let package = index
            .package_ref_page_after_record_id(None, 1)
            .expect("package page")
            .remove(0);
        let paths = index
            .package_source_paths(&[package.clone(), package.clone()])
            .expect("source paths");
        assert_eq!(
            paths,
            vec![root.join(relative_path), root.join(relative_path)]
        );
        assert!(
            index
                .package_source_paths(&[])
                .expect("empty lookup")
                .is_empty()
        );

        let mut missing = package;
        missing.record_id = i64::MAX;
        let error = index
            .package_source_paths(&[missing])
            .expect_err("missing record should fail");
        assert!(error.to_string().contains(&i64::MAX.to_string()));
    }
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

fn lod_model(lods: &[&str]) -> cityjson_lib::CityModel {
    let geometries: Vec<_> = lods
        .iter()
        .map(|lod| {
            json!({
                "type": "MultiSurface", "lod": lod, "boundaries": [[[0, 1, 2]]]
            })
        })
        .collect();
    cityjson_lib::json::from_slice(
        &serde_json::to_vec(&json!({
            "type": "CityJSON", "version": "2.0",
            "transform": {"scale": [1, 1, 1], "translate": [0, 0, 0]},
            "CityObjects": {"building": {"type": "Building", "geometry": geometries}},
            "vertices": [[0, 0, 0], [1, 0, 0], [0, 1, 0]]
        }))
        .expect("JSON bytes"),
    )
    .expect("LoD model")
}

/// Input: geometry at three LoDs with default and exact selections.
/// Assertions: exact selection retains any requested value and All/Highest retain their defaults.
#[test]
fn package_filter_selects_multiple_exact_lods() {
    let model = lod_model(&["1.0", "2.0", "2.2"]);
    for (selection, expected) in [
        (LodSelection::All, vec!["1.0", "2.0", "2.2"]),
        (LodSelection::Highest, vec!["2.2"]),
        (LodSelection::Exact(vec!["2.0".into()]), vec!["2.0"]),
        (
            LodSelection::Exact(vec!["1.0".into(), "2.2".into()]),
            vec!["1.0", "2.2"],
        ),
        (
            LodSelection::Exact(vec!["2.0".into(), "9.9".into()]),
            vec!["2.0"],
        ),
    ] {
        let filter = PackageFilter {
            default_lod: selection,
            ..Default::default()
        };
        let result = filter.apply(&model).expect("filter");
        let expected: BTreeSet<_> = expected.into_iter().map(str::to_owned).collect();
        assert_eq!(result.report.retained_lods["Building"], expected);
        assert_eq!(result.report.retained_geometry_count, expected.len());
        let output = result.model.expect("retained model");
        let output_lods: BTreeSet<_> = output
            .cityobjects()
            .iter()
            .flat_map(|(_, object)| {
                object.geometry().unwrap_or_default().iter().map(|handle| {
                    output
                        .get_geometry(*handle)
                        .expect("geometry")
                        .lod()
                        .expect("LoD")
                        .to_string()
                })
            })
            .collect();
        assert_eq!(output_lods, expected);
    }
    let filter = PackageFilter {
        default_lod: LodSelection::Exact(vec!["2.0".into(), "8.8".into(), "9.9".into()]),
        ..Default::default()
    };
    let report = filter.apply(&model).expect("partial default match").report;
    assert_eq!(
        report.missing_lods["Building"].requested_lods,
        BTreeSet::from(["8.8".into(), "9.9".into()])
    );
    assert!(report.ensure_requested_lods_available(&filter).is_err());
    assert_eq!(PackageFilter::default().default_lod, LodSelection::All);
}

/// Input: partial availability in two packages and two unavailable requested values.
/// Assertions: reports merge every missing value, clear values found in another package, and errors show available LoDs.
#[test]
fn package_filter_aggregates_missing_lods_across_packages() {
    let filter = PackageFilter {
        lods_by_type: [(
            "Building".into(),
            LodSelection::Exact(vec!["1.0".into(), "2.0".into(), "8.8".into(), "9.9".into()]),
        )]
        .into(),
        ..Default::default()
    };
    let first = filter.apply(&lod_model(&["1.0"])).expect("first");
    let second = filter.apply(&lod_model(&["2.0"])).expect("second");
    for reports in [
        [&first.report, &second.report],
        [&second.report, &first.report],
    ] {
        let mut report = PackageFilterReport::default();
        for item in reports {
            report.merge(item);
        }
        let missing = &report.missing_lods["Building"];
        assert_eq!(
            missing.requested_lods,
            BTreeSet::from(["8.8".into(), "9.9".into()])
        );
        assert_eq!(
            missing.available_lods,
            BTreeSet::from(["1.0".into(), "2.0".into()])
        );
        let error = report
            .ensure_requested_lods_available(&filter)
            .expect_err("unavailable")
            .to_string();
        for value in ["8.8", "9.9", "1.0", "2.0", "Building"] {
            assert!(error.contains(value), "{error}");
        }
        let available_filter = PackageFilter {
            default_lod: LodSelection::Exact(vec!["1.0".into(), "2.0".into()]),
            ..Default::default()
        };
        report
            .ensure_requested_lods_available(&available_filter)
            .expect("available across packages");
    }
}

/// Input: complementary package LoDs that jointly cover the requested selection.
/// Assertions: merged reports remove missing entries when all requested values are found.
#[test]
fn package_filter_merge_clears_lods_found_in_other_packages() {
    let filter = PackageFilter {
        default_lod: LodSelection::Exact(vec!["1.0".into(), "2.2".into()]),
        ..Default::default()
    };
    let mut report = filter.apply(&lod_model(&["1.0"])).expect("first").report;
    assert_eq!(
        report.missing_lods["Building"].requested_lods,
        BTreeSet::from(["2.2".into()])
    );
    report.merge(&filter.apply(&lod_model(&["2.2"])).expect("second").report);
    assert!(report.missing_lods.is_empty());
    report
        .ensure_requested_lods_available(&filter)
        .expect("all found");
}

/// Input: empty exact selections on the default or an unused type override.
/// Assertions: filtering and report validation reject both even when no matching type occurs.
#[test]
fn package_filter_rejects_empty_exact_selections() {
    let model = lod_model(&["1.0"]);
    for filter in [
        PackageFilter {
            default_lod: LodSelection::Exact(vec![]),
            ..Default::default()
        },
        PackageFilter {
            lods_by_type: [("Road".into(), LodSelection::Exact(vec![]))].into(),
            ..Default::default()
        },
    ] {
        assert!(
            filter
                .apply(&model)
                .err()
                .expect("empty selection")
                .to_string()
                .contains("must not be empty")
        );
        assert!(
            PackageFilterReport::default()
                .ensure_requested_lods_available(&filter)
                .is_err()
        );
    }
}

/// Input: shared-child packages with a structural Building and BuildingPart geometry at LoD 2.0.
/// Assertions: exact default validation passes and filtering preserves the hierarchy and geometry.
#[test]
fn default_exact_lod_preserves_structural_ancestors() {
    let index = shared_child_index();
    let child = index
        .lookup_cityobject_refs("shared-part")
        .expect("child")
        .remove(0);
    let refs = index.package_refs_for_cityobject(&child).expect("packages");
    for cityobject_types in [None, Some(BTreeSet::from(["Building".into()]))] {
        let filter = PackageFilter {
            cityobject_types,
            default_lod: LodSelection::Exact(vec!["2.0".into()]),
            ..Default::default()
        };
        let results = index
            .read_filtered_packages(&refs, &filter)
            .expect("filter");
        let mut merged = PackageFilterReport::default();
        for result in results {
            assert!(result.report.retained_types.contains("Building"));
            assert!(!result.report.available_lods.contains_key("Building"));
            assert!(result.report.missing_lods.is_empty());
            result
                .report
                .ensure_requested_lods_available(&filter)
                .expect("structural parent is valid");
            merged.merge(&result.report);
            let output = model_json(&result.model.expect("matching geometry"));
            let objects = output["CityObjects"].as_object().expect("objects");
            let (parent_id, parent) = objects
                .iter()
                .find(|(_, object)| object["type"] == "Building")
                .expect("parent");
            assert_eq!(parent["children"], json!(["shared-part"]));
            assert_eq!(objects["shared-part"]["parents"], json!([parent_id]));
            assert_eq!(objects["shared-part"]["geometry"][0]["lod"], "2.0");
        }
        merged
            .ensure_requested_lods_available(&filter)
            .expect("merged hierarchy");
    }
}

fn hierarchy_lod_model(lod: Option<&str>, parent_lod: Option<&str>) -> cityjson_lib::CityModel {
    let mut objects = hierarchy_cityobjects();
    if let Some(lod) = lod {
        objects["part"]["geometry"][0]["lod"] = json!(lod);
    } else {
        objects["part"]["geometry"][0]
            .as_object_mut()
            .expect("geometry")
            .remove("lod");
    }
    if let Some(lod) = parent_lod {
        let mut geometry = objects["part"]["geometry"][0].clone();
        geometry["lod"] = json!(lod);
        objects["building"]["geometry"] = json!([geometry]);
    } else {
        objects["building"]["geometry"] = json!([]);
    }
    cityjson_lib::json::from_slice(
        &serde_json::to_vec(&json!({
            "type": "CityJSON", "version": "2.0",
            "transform": {"scale": [1, 1, 1], "translate": [0, 0, 0]},
            "CityObjects": objects, "vertices": hierarchy_vertices()
        }))
        .expect("JSON bytes"),
    )
    .expect("hierarchical model")
}

/// Input: structural parents, geometry-owning parents, absent overrides, and geometry without LoD metadata.
/// Assertions: defaults target geometry owners, explicit overrides remain strict, and empty availability survives merging.
#[test]
fn default_exact_lod_validates_geometry_owners_and_explicit_overrides() {
    let filter = PackageFilter {
        default_lod: LodSelection::Exact(vec!["2.0".into(), "8.8".into(), "9.9".into()]),
        ..Default::default()
    };
    let report = filter
        .apply(&hierarchy_lod_model(Some("2.0"), None))
        .expect("partial match")
        .report;
    assert_eq!(report.missing_lods.len(), 1);
    assert_eq!(
        report.missing_lods["BuildingPart"].requested_lods,
        BTreeSet::from(["8.8".into(), "9.9".into()])
    );
    assert_eq!(
        report.missing_lods["BuildingPart"].available_lods,
        BTreeSet::from(["2.0".into()])
    );
    assert!(report.ensure_requested_lods_available(&filter).is_err());

    for requested_type in ["Building", "Road"] {
        let explicit = PackageFilter {
            lods_by_type: [(
                requested_type.into(),
                LodSelection::Exact(vec!["2.0".into()]),
            )]
            .into(),
            ..Default::default()
        };
        let result = explicit
            .apply(&hierarchy_lod_model(Some("2.0"), None))
            .expect("override");
        assert!(
            result
                .report
                .ensure_requested_lods_available(&explicit)
                .is_err()
        );
        if requested_type == "Building" {
            assert!(result.report.missing_lods.contains_key("Building"));
        }
    }
    let single = PackageFilter {
        default_lod: LodSelection::Exact(vec!["2.0".into()]),
        ..Default::default()
    };
    let parent_result = single
        .apply(&hierarchy_lod_model(Some("2.0"), Some("1.0")))
        .expect("parent geometry");
    let parent_output = model_json(&parent_result.model.expect("matching child"));
    assert!(
        parent_output["CityObjects"]["building"]
            .get("geometry")
            .is_none_or(|geometries| geometries.as_array().is_some_and(Vec::is_empty))
    );
    assert_eq!(
        parent_output["CityObjects"]["part"]["geometry"]
            .as_array()
            .expect("child geometry")
            .len(),
        1
    );
    let parent_report = parent_result.report;
    assert!(parent_report.missing_lods.contains_key("Building"));
    assert!(
        parent_report
            .ensure_requested_lods_available(&single)
            .is_err()
    );
    let no_lod = single
        .apply(&hierarchy_lod_model(None, None))
        .expect("missing metadata")
        .report;
    assert_eq!(
        no_lod.available_lods.get("BuildingPart"),
        Some(&BTreeSet::new())
    );
    assert!(!no_lod.available_lods.contains_key("Building"));
    let mut merged = PackageFilterReport::default();
    merged.merge(&no_lod);
    assert_eq!(
        merged.available_lods.get("BuildingPart"),
        Some(&BTreeSet::new())
    );
    assert!(merged.ensure_requested_lods_available(&single).is_err());
}

/// Input: hierarchical packages whose child geometries have complementary LoDs.
/// Assertions: both merge orders satisfy available values and retain only genuinely unavailable diagnostics.
#[test]
fn default_exact_lod_merges_hierarchy_availability() {
    for requested in [vec!["1.0", "2.0"], vec!["1.0", "2.0", "8.8", "9.9"]] {
        let filter = PackageFilter {
            default_lod: LodSelection::Exact(requested.iter().map(|lod| (*lod).into()).collect()),
            ..Default::default()
        };
        let reports: Vec<_> = ["1.0", "2.0"]
            .iter()
            .map(|lod| {
                filter
                    .apply(&hierarchy_lod_model(Some(lod), None))
                    .expect("package")
                    .report
            })
            .collect();
        for order in [[0, 1], [1, 0]] {
            let mut merged = PackageFilterReport::default();
            for index in order {
                merged.merge(&reports[index]);
            }
            assert!(!merged.missing_lods.contains_key("Building"));
            if requested.len() == 2 {
                assert!(merged.missing_lods.is_empty());
                merged
                    .ensure_requested_lods_available(&filter)
                    .expect("complementary values");
            } else {
                assert_eq!(
                    merged.missing_lods["BuildingPart"].requested_lods,
                    BTreeSet::from(["8.8".into(), "9.9".into()])
                );
                assert!(merged.ensure_requested_lods_available(&filter).is_err());
            }
        }
    }
}
