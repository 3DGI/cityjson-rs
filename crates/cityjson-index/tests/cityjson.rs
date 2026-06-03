#![allow(
    clippy::doc_markdown,
    reason = "test docstrings use domain terminology plainly"
)]

mod common;

use std::fs;

use cityjson_index::{CityIndex, StorageLayout, resolve_dataset};
use common::{
    bbox_for_model, cityjson_root, feature_files_root, find_first, model_contains_id,
    temp_fixture_root, temp_index_path,
};
use serde_json::Value;

#[test]
fn cityjson_cityindex_supports_end_to_end_queries() {
    let feature_root = feature_files_root();
    let feature_sample = find_first(&feature_root.join("features"), "city.jsonl", true);
    let metadata_bytes =
        fs::read(feature_root.join("metadata.json")).expect("root metadata file must be readable");
    let feature_bytes = fs::read(&feature_sample).expect("sample feature file must be readable");
    let mut document: Value =
        serde_json::from_slice(&metadata_bytes).expect("root metadata JSON should be valid");
    let feature: Value =
        serde_json::from_slice(&feature_bytes).expect("feature package JSON should be valid");
    let root = document
        .as_object_mut()
        .expect("metadata root must be a JSON object");
    root.insert("type".to_owned(), Value::String("CityJSON".to_owned()));
    root.insert(
        "CityObjects".to_owned(),
        feature
            .get("CityObjects")
            .cloned()
            .expect("feature package must contain CityObjects"),
    );
    root.insert(
        "vertices".to_owned(),
        feature
            .get("vertices")
            .cloned()
            .expect("feature package must contain vertices"),
    );
    let bytes = serde_json::to_vec(&document).expect("derived cityjson document should serialize");
    let root = temp_fixture_root("cityjson-data");
    let sample = root.join("sample.city.json");
    fs::write(&sample, &bytes).expect("derived cityjson tile should be writable");
    let value: Value = serde_json::from_slice(&bytes).expect("valid cityjson tile");
    let feature_id = first_cityobject_with_geometry(&value)
        .expect("sample cityjson tile must contain a cityobject with geometry");

    let index_path = temp_index_path("cityjson");
    let mut index = CityIndex::open(
        StorageLayout::CityJson {
            paths: vec![root.clone()],
        },
        &index_path,
    )
    .expect("cityjson index should open");

    index.reindex().expect("cityjson reindex should succeed");

    let model = index
        .get_packages(&feature_id)
        .expect("cityjson package lookup should succeed")
        .into_iter()
        .next()
        .expect("feature id should be indexed");
    assert!(model_contains_id(&model, &feature_id));

    let bbox = bbox_for_model(&model).expect("bbox should be computable from indexed model");
    let query_hits = index.query(&bbox).expect("cityjson query should succeed");
    assert!(
        query_hits
            .iter()
            .any(|candidate| model_contains_id(candidate, &feature_id)),
        "query should return the selected feature"
    );

    let iter_hits = index
        .query_iter(&bbox)
        .expect("cityjson query_iter should succeed")
        .collect::<cityjson_lib::Result<Vec<_>>>()
        .expect("cityjson query_iter items should succeed");
    assert!(
        iter_hits
            .iter()
            .any(|candidate| model_contains_id(candidate, &feature_id)),
        "query_iter should return the selected feature"
    );

    let package_hits = index
        .query_package_refs(&bbox)
        .expect("cityjson package query should succeed");
    assert!(
        package_hits
            .iter()
            .any(|candidate| candidate.model_id == feature_id),
        "package query should return the selected package id"
    );

    let metadata = index.metadata().expect("cityjson metadata should succeed");
    assert!(
        metadata
            .iter()
            .any(|entry| entry.get("transform").is_some()),
        "cityjson metadata should include at least one transform"
    );
}

fn first_cityobject_with_geometry(value: &Value) -> Option<String> {
    let cityobjects = value.get("CityObjects")?.as_object()?;
    cityobjects
        .iter()
        .find(|(_, object)| object.get("geometry").is_some())
        .map(|(id, _)| id.clone())
        .or_else(|| cityobjects.keys().next().cloned())
}

#[test]
fn cityjson_index_needs_reindex_after_adding_files() {
    let dir = temp_fixture_root("needs-reindex");
    let index_path = temp_index_path("needs-reindex");

    // Reindex against the empty directory — index is created with zero sources.
    let mut index = CityIndex::open(
        StorageLayout::CityJson {
            paths: vec![dir.clone()],
        },
        &index_path,
    )
    .expect("index should open on empty dir");
    index
        .reindex()
        .expect("reindex on empty dir should succeed");

    // Add a CityJSON file to the directory after indexing.
    let fixture = find_first(&cityjson_root(), "city.json", true);
    fs::copy(&fixture, dir.join("sample.city.json")).expect("fixture copy should succeed");

    // Inspect the dataset: the new file is unindexed so needs_reindex must be true.
    let resolved = resolve_dataset(&dir, Some(index_path))
        .expect("resolve_dataset should detect cityjson layout");
    let inspection = resolved.inspect().expect("inspect should succeed");

    assert!(
        !inspection.index.unindexed_source_paths.is_empty(),
        "unindexed_source_paths should contain the newly added file"
    );
    assert_eq!(
        inspection.index.fresh,
        Some(false),
        "index should not be fresh after a new file is added"
    );
    assert!(
        !inspection.index.issues.is_empty(),
        "inspect should report issues for the unindexed file"
    );
}
