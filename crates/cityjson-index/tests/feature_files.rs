#![allow(
    clippy::doc_markdown,
    reason = "test docstrings use domain terminology plainly"
)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use cityjson_index::{CityIndex, StorageLayout};
use common::{
    bbox_for_model, feature_files_root, find_first, materialize_subset, model_contains_id,
    temp_index_path,
};
use serde_json::{Map, Value};

#[test]
fn feature_files_cityindex_supports_end_to_end_queries() {
    let source_root = feature_files_root();
    let sample = find_first(&source_root.join("features"), "city.jsonl", true);
    let root = materialize_subset(
        "feature-files-data",
        &source_root,
        &[source_root.join("metadata.json"), sample.clone()],
    );
    let bytes = fs::read(&sample).expect("sample feature file must be readable");
    let value: Value = serde_json::from_slice(&bytes).expect("valid JSON feature");
    let feature_id = first_cityobject_id(&value);

    let index_path = temp_index_path("feature-files");
    let mut index = CityIndex::open(
        StorageLayout::FeatureFiles {
            root: root.clone(),
            metadata_glob: "**/metadata.json".to_owned(),
            feature_glob: "**/*.city.jsonl".to_owned(),
        },
        &index_path,
    )
    .expect("feature-files index should open");

    index
        .reindex()
        .expect("feature-files reindex should succeed");

    let model = index
        .get_packages(&feature_id)
        .expect("feature-files package lookup should succeed")
        .into_iter()
        .next()
        .expect("feature id should be indexed");
    assert!(model_contains_id(&model, &feature_id));

    let bbox = bbox_for_model(&model).expect("bbox should be computable from indexed model");
    let query_hits = index
        .query_package_models(&bbox)
        .expect("feature-files query should succeed");
    assert!(
        query_hits
            .iter()
            .any(|candidate| model_contains_id(candidate, &feature_id)),
        "query should return the selected feature"
    );

    let metadata = index
        .metadata()
        .expect("feature-files metadata should succeed");
    assert!(
        metadata
            .iter()
            .any(|entry| entry.get("transform").is_some()),
        "feature-files metadata should include at least one transform"
    );
}

#[test]
fn feature_files_index_every_cityobject_key_and_ignore_top_level_id() {
    let root = temp_parallel_fixture_root("feature-files-cityobject-keys");
    materialize_feature_files_metadata(&root);
    let feature_path = root.join("features/a/multi.city.jsonl");
    write_feature_file(
        &feature_path,
        "ignored-feature-id",
        &[("feature-key-a", 0), ("feature-key-b", 10)],
    );

    let index_path = temp_index_path("feature-files-cityobject-keys");
    let mut index =
        build_feature_files_index(&root, &index_path).expect("feature-files reindex should work");

    assert!(
        index
            .get_packages("ignored-feature-id")
            .expect("top-level id lookup should succeed")
            .is_empty()
    );

    for feature_id in ["feature-key-a", "feature-key-b"] {
        let model = index
            .get_packages(feature_id)
            .expect("cityobject key lookup should succeed")
            .into_iter()
            .next()
            .expect("cityobject key should be indexed");
        assert!(model_contains_id(&model, "feature-key-a"));
        assert!(model_contains_id(&model, "feature-key-b"));
    }

    index
        .reindex()
        .expect("reindexing aliases should remain stable");
}

/// Input: two feature-files packages that reuse the same CityObject key in different files.
/// Assertions: plural lookup returns both duplicate occurrences in row order with their original source paths.
#[test]
fn feature_files_allow_duplicate_cityobject_keys() {
    let root = temp_parallel_fixture_root("feature-files-duplicate-keys");
    materialize_feature_files_metadata(&root);
    let first_path = root.join("features/a/first.city.jsonl");
    let second_path = root.join("features/a/second.city.jsonl");
    write_feature_file(&first_path, "ignored-first", &[("duplicate-key", 0)]);
    write_feature_file(&second_path, "ignored-second", &[("duplicate-key", 20)]);

    let index_path = temp_index_path("feature-files-duplicate-keys");
    let index =
        build_feature_files_index(&root, &index_path).expect("duplicate ids should reindex");

    let refs = index
        .lookup_cityobject_refs("duplicate-key")
        .expect("plural lookup should succeed");
    assert_eq!(
        refs.len(),
        2,
        "both duplicate CityObjects should be indexed"
    );
    assert!(refs[0].record_id != refs[1].record_id);
    let mut package_model_ids = refs
        .iter()
        .map(|reference| {
            index
                .package_refs_for_cityobject(reference)
                .expect("duplicate package lookup should succeed")[0]
                .model_id
                .clone()
        })
        .collect::<Vec<_>>();
    package_model_ids.sort();
    assert_eq!(package_model_ids, ["ignored-first", "ignored-second"]);
    assert!(first_path.exists());
    assert!(second_path.exists());
}

#[test]
fn feature_files_reindex_is_stable_across_worker_counts() {
    let root = materialize_parallel_feature_files_dataset("feature-files-parity", 4);
    let single_index_path = temp_index_path("feature-files-parity-single");
    let multi_index_path = temp_index_path("feature-files-parity-multi");

    let single = with_worker_count_env(1, || build_feature_files_index(&root, &single_index_path))
        .expect("single-worker reindex should succeed");
    let multi = with_worker_count_env(4, || build_feature_files_index(&root, &multi_index_path))
        .expect("multi-worker reindex should succeed");

    assert_eq!(
        single.source_count().expect("single source count"),
        4,
        "single-worker source count should match the materialized dataset"
    );
    assert_eq!(
        single.package_count().expect("single package count"),
        8,
        "single-worker package count should match the materialized dataset"
    );
    assert_eq!(
        single.cityobject_count().expect("single CityObject count"),
        8,
        "single-worker CityObject count should match the materialized dataset"
    );

    assert_eq!(
        single.source_count().expect("single source count"),
        multi.source_count().expect("multi source count"),
        "source counts should be stable across worker counts"
    );
    assert_eq!(
        single.package_count().expect("single package count"),
        multi.package_count().expect("multi package count"),
        "package counts should be stable across worker counts"
    );
    assert_eq!(
        single.cityobject_count().expect("single CityObject count"),
        multi.cityobject_count().expect("multi CityObject count"),
        "CityObject counts should be stable across worker counts"
    );

    let single_ids = collect_package_ids(&single).expect("single feature ids");
    let multi_ids = collect_package_ids(&multi).expect("multi feature ids");
    assert_eq!(
        single_ids, multi_ids,
        "feature identifier sets should be stable across worker counts"
    );

    let sample_id = single_ids
        .iter()
        .next()
        .cloned()
        .expect("dataset should contain at least one feature");
    let single_model = single
        .get_packages(&sample_id)
        .expect("single package lookup should succeed")
        .into_iter()
        .next()
        .expect("sample package should be indexed");
    let multi_model = multi
        .get_packages(&sample_id)
        .expect("multi package lookup should succeed")
        .into_iter()
        .next()
        .expect("sample package should be indexed");
    assert_eq!(
        model_value(&single_model),
        model_value(&multi_model),
        "feature reconstruction should be stable across worker counts"
    );
    assert!(
        model_contains_id(&single_model, &sample_id),
        "sample feature should be present in the reconstructed model"
    );

    let bbox = bbox_for_model(&single_model).expect("sample model bbox should be computable");
    let single_hits = single
        .query_package_models(&bbox)
        .expect("single query should succeed");
    let multi_hits = multi
        .query_package_models(&bbox)
        .expect("multi query should succeed");
    assert_eq!(
        single_hits.len(),
        1,
        "the sample bbox should isolate the single chosen feature"
    );
    assert_eq!(
        single_hits.len(),
        multi_hits.len(),
        "bbox hit counts should be stable across worker counts"
    );
    assert!(
        multi_hits
            .iter()
            .any(|candidate| model_contains_id(candidate, &sample_id)),
        "the sample feature should be present in the multi-worker bbox results"
    );
}

#[test]
fn feature_files_single_metadata_parallelizes_by_feature_file() {
    let root = materialize_single_metadata_feature_files_dataset("feature-files-one-source", 16);
    let single_index_path = temp_index_path("feature-files-one-source-single");
    let multi_index_path = temp_index_path("feature-files-one-source-multi");

    let single = with_worker_count_env(1, || build_feature_files_index(&root, &single_index_path))
        .expect("single-worker reindex should succeed");
    let multi = with_worker_count_env(4, || build_feature_files_index(&root, &multi_index_path))
        .expect("multi-worker reindex should succeed");

    assert_eq!(
        single.source_count().expect("single source count"),
        1,
        "one metadata file should produce one source row"
    );
    assert_eq!(
        multi.source_count().expect("multi source count"),
        1,
        "feature-file sharding must not duplicate metadata sources"
    );
    assert_eq!(
        single.package_count().expect("single package count"),
        16,
        "all feature files should be indexed"
    );
    assert_eq!(
        single.package_count().expect("single package count"),
        multi.package_count().expect("multi package count"),
        "package counts should be stable across worker counts"
    );
    assert_eq!(
        single.cityobject_count().expect("single CityObject count"),
        multi.cityobject_count().expect("multi CityObject count"),
        "CityObject counts should be stable across worker counts"
    );

    let single_ids = collect_package_ids(&single).expect("single feature ids");
    let multi_ids = collect_package_ids(&multi).expect("multi feature ids");
    assert_eq!(
        single_ids, multi_ids,
        "feature identifiers should be stable across worker counts"
    );

    let sample_id = single_ids
        .iter()
        .nth(5)
        .cloned()
        .expect("dataset should contain a representative feature");
    let single_model = single
        .get_packages(&sample_id)
        .expect("single package lookup should succeed")
        .into_iter()
        .next()
        .expect("sample package should be indexed");
    let multi_model = multi
        .get_packages(&sample_id)
        .expect("multi package lookup should succeed")
        .into_iter()
        .next()
        .expect("sample package should be indexed");
    assert_eq!(
        model_value(&single_model),
        model_value(&multi_model),
        "representative reconstruction should be stable across worker counts"
    );
    assert!(
        model_contains_id(&multi_model, &sample_id),
        "sample feature should be present in the reconstructed model"
    );

    let bbox = bbox_for_model(&single_model).expect("sample model bbox should be computable");
    let single_hits = single
        .query_package_models(&bbox)
        .expect("single query should succeed");
    let multi_hits = multi
        .query_package_models(&bbox)
        .expect("multi query should succeed");
    assert_eq!(
        single_hits.len(),
        multi_hits.len(),
        "bbox hit counts should be stable across worker counts"
    );
    assert!(
        multi_hits
            .iter()
            .any(|candidate| model_contains_id(candidate, &sample_id)),
        "bbox results should include the representative feature"
    );
}

fn build_feature_files_index(root: &Path, index_path: &Path) -> cityjson_lib::Result<CityIndex> {
    let mut index = CityIndex::open(
        StorageLayout::FeatureFiles {
            root: root.to_path_buf(),
            metadata_glob: "**/metadata.json".to_owned(),
            feature_glob: "**/*.city.jsonl".to_owned(),
        },
        index_path,
    )?;
    index.reindex()?;
    Ok(index)
}

fn collect_package_ids(
    index: &CityIndex,
) -> cityjson_lib::Result<std::collections::BTreeSet<String>> {
    let mut ids = std::collections::BTreeSet::new();
    let mut after_record_id = None;
    loop {
        let page = index.package_ref_page_after_record_id(after_record_id, 128)?;
        if page.is_empty() {
            break;
        }
        after_record_id = page.last().map(|package| package.record_id);
        ids.extend(page.into_iter().map(|package| package.model_id));
    }
    Ok(ids)
}

fn model_value(model: &cityjson_lib::CityModel) -> Value {
    serde_json::from_str(&cityjson_lib::json::to_string(model).expect("model should serialize"))
        .expect("serialized model should be valid JSON")
}

fn materialize_parallel_feature_files_dataset(label: &str, source_count: usize) -> PathBuf {
    let source_root = feature_files_root();
    let metadata = source_root.join("metadata.json");
    let feature_files = [
        source_root.join("features/a/fixture-a.city.jsonl"),
        source_root.join("features/a/fixture-b.city.jsonl"),
    ];
    let root = temp_parallel_fixture_root(label);

    for source_index in 0..source_count {
        let source_dir = root.join(format!("source-{source_index:02}"));
        let features_dir = source_dir.join("features/a");
        fs::create_dir_all(&features_dir).expect("source feature directory should be creatable");
        fs::copy(&metadata, source_dir.join("metadata.json")).expect("metadata file should copy");

        for feature_path in &feature_files {
            let bytes = fs::read(feature_path).expect("fixture feature file should be readable");
            let rewritten = rewrite_feature_file(&bytes, source_index);
            fs::write(
                features_dir.join(feature_path.file_name().expect("feature file name")),
                rewritten,
            )
            .expect("feature file should write");
        }
    }

    root
}

fn materialize_single_metadata_feature_files_dataset(label: &str, feature_count: usize) -> PathBuf {
    let source_root = feature_files_root();
    let metadata = source_root.join("metadata.json");
    let feature_files = [
        source_root.join("features/a/fixture-a.city.jsonl"),
        source_root.join("features/a/fixture-b.city.jsonl"),
    ];
    let root = temp_parallel_fixture_root(label);
    let features_dir = root.join("features/a");
    fs::create_dir_all(&features_dir).expect("feature directory should be creatable");
    fs::copy(&metadata, root.join("metadata.json")).expect("metadata file should copy");

    for feature_index in 0..feature_count {
        let feature_path = &feature_files[feature_index % feature_files.len()];
        let bytes = fs::read(feature_path).expect("fixture feature file should be readable");
        let rewritten = rewrite_feature_file(&bytes, feature_index);
        fs::write(
            features_dir.join(format!("feature-{feature_index:03}.city.jsonl")),
            rewritten,
        )
        .expect("feature file should write");
    }

    root
}

fn rewrite_feature_file(bytes: &[u8], source_index: usize) -> Vec<u8> {
    let mut feature: Value = serde_json::from_slice(bytes).expect("fixture feature should parse");
    let base_id = feature
        .get("id")
        .and_then(Value::as_str)
        .expect("fixture feature should carry an id")
        .to_owned();
    let feature_id = format!("{base_id}-{source_index:02}");
    let shift = i64::try_from(source_index).expect("source index should fit in i64") * 1_000;

    let object = feature
        .as_object_mut()
        .expect("feature fixture should be a JSON object");
    object.insert("id".to_owned(), Value::String(feature_id.clone()));

    if let Some(cityobjects) = object.get_mut("CityObjects").and_then(Value::as_object_mut) {
        let mut renamed = Map::new();
        for (_, cityobject) in std::mem::take(cityobjects) {
            renamed.insert(feature_id.clone(), cityobject);
        }
        *cityobjects = renamed;
    }

    if let Some(vertices) = object.get_mut("vertices").and_then(Value::as_array_mut) {
        for vertex in vertices {
            let coords = vertex
                .as_array_mut()
                .expect("vertices should be coordinate arrays");
            coords[0] = Value::from(
                coords[0]
                    .as_i64()
                    .expect("x coordinate should be an integer")
                    + shift,
            );
            coords[1] = Value::from(
                coords[1]
                    .as_i64()
                    .expect("y coordinate should be an integer")
                    + shift,
            );
        }
    }

    serde_json::to_vec(&feature).expect("rewritten feature should serialize")
}

fn first_cityobject_id(value: &Value) -> String {
    value["CityObjects"]
        .as_object()
        .expect("feature file must contain CityObjects")
        .keys()
        .next()
        .expect("feature file must contain at least one CityObject")
        .to_owned()
}

fn materialize_feature_files_metadata(root: &Path) {
    let source_root = feature_files_root();
    fs::copy(
        source_root.join("metadata.json"),
        root.join("metadata.json"),
    )
    .expect("metadata file should copy");
}

fn write_feature_file(path: &Path, top_level_id: &str, objects: &[(&str, i64)]) {
    let parent = path.parent().expect("feature path should have a parent");
    fs::create_dir_all(parent).expect("feature directory should be creatable");

    let mut cityobjects = Map::new();
    let mut vertices = Vec::new();
    for (index, (id, base)) in objects.iter().enumerate() {
        let start = index * 3;
        cityobjects.insert(
            (*id).to_owned(),
            serde_json::json!({
                "type": "Building",
                "geometry": [{
                    "type": "MultiSurface",
                    "lod": "1.0",
                    "boundaries": [[[start, start + 1, start + 2]]]
                }]
            }),
        );
        vertices.push(serde_json::json!([base, base, 0]));
        vertices.push(serde_json::json!([base + 1, base, 0]));
        vertices.push(serde_json::json!([base, base + 1, 0]));
    }

    let feature = serde_json::json!({
        "type": "CityJSONFeature",
        "id": top_level_id,
        "CityObjects": cityobjects,
        "vertices": vertices
    });
    fs::write(
        path,
        serde_json::to_vec(&feature).expect("feature should serialize"),
    )
    .expect("feature file should write");
}

fn temp_parallel_fixture_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after the unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("cityjson-index-{label}-{unique}.dir"));
    fs::create_dir_all(&path).expect("parallel fixture root should be creatable");
    path
}

struct EnvGuard {
    previous: Option<std::ffi::OsString>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: the test serializes environment mutation with a mutex and
        // restores the previous value before releasing it.
        unsafe {
            match self.previous.take() {
                Some(previous) => std::env::set_var(cityjson_index::WORKER_COUNT_ENV, previous),
                None => std::env::remove_var(cityjson_index::WORKER_COUNT_ENV),
            }
        }
    }
}

fn with_worker_count_env<T>(worker_count: usize, f: impl FnOnce() -> T) -> T {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _lock = ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock");

    let previous = std::env::var_os(cityjson_index::WORKER_COUNT_ENV);
    let _guard = EnvGuard { previous };
    // SAFETY: the test holds a process-wide mutex while mutating the variable.
    unsafe {
        std::env::set_var(cityjson_index::WORKER_COUNT_ENV, worker_count.to_string());
    }
    f()
}
