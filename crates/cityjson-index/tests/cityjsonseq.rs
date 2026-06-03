mod common;

use std::fs;
use std::path::Path;

use cityjson_index::{CityIndex, StorageLayout};
use common::{bbox_for_model, cityjsonseq_root, find_first, model_contains_id, temp_index_path};

/// Input: a derived single-feature CityJSONSeq stream with metadata, geometry, and transform.
/// Assertions: indexing, get, bbox query, iterators, id-aware iteration, and metadata retrieval all return the selected feature.
#[test]
fn cityjson_seq_cityindex_supports_end_to_end_queries() {
    let source_root = cityjsonseq_root();
    let sample = find_first(&source_root, "city.jsonl", true);
    let sample_fixture = derive_small_cityjson_seq_fixture(&sample);
    let feature_id = "cityjson-seq-test-feature".to_owned();

    let index_path = temp_index_path("CityJSONSeq");
    let mut index = CityIndex::open(
        StorageLayout::Ndjson {
            paths: vec![sample_fixture.clone()],
        },
        &index_path,
    )
    .expect("CityJSONSeq index should open");

    index.reindex().expect("CityJSONSeq reindex should succeed");

    let model = index
        .get(&feature_id)
        .expect("CityJSONSeq get should succeed")
        .expect("feature id should be indexed");
    assert!(model_contains_id(&model, &feature_id));

    let bbox = bbox_for_model(&model).expect("bbox should be computable from indexed model");
    let query_hits = index
        .query(&bbox)
        .expect("CityJSONSeq query should succeed");
    assert!(
        query_hits
            .iter()
            .any(|candidate| model_contains_id(candidate, &feature_id)),
        "query should return the selected feature"
    );

    let iter_hits = index
        .query_iter(&bbox)
        .expect("CityJSONSeq query_iter should succeed")
        .collect::<cityjson_lib::Result<Vec<_>>>()
        .expect("CityJSONSeq query_iter items should succeed");
    assert!(
        iter_hits
            .iter()
            .any(|candidate| model_contains_id(candidate, &feature_id)),
        "query_iter should return the selected feature"
    );

    let iter_hits_with_ids = index
        .query_iter_with_ids(&bbox)
        .expect("CityJSONSeq query_iter_with_ids should succeed")
        .collect::<cityjson_lib::Result<Vec<_>>>()
        .expect("CityJSONSeq query_iter_with_ids items should succeed");
    assert!(
        iter_hits_with_ids
            .iter()
            .any(|(candidate_id, candidate)| candidate_id == &feature_id
                && model_contains_id(candidate, &feature_id)),
        "query_iter_with_ids should return the selected feature id and model"
    );

    let metadata = index
        .metadata()
        .expect("CityJSONSeq metadata should succeed");
    assert!(
        metadata
            .iter()
            .any(|entry| entry.get("transform").is_some()),
        "CityJSONSeq metadata should include at least one transform"
    );
}

/// Input: one CityJSONSeq feature whose top-level id differs from two CityObject keys.
/// Assertions: the top-level id is not addressable while both CityObject keys reconstruct the complete package.
#[test]
fn cityjson_seq_indexes_every_cityobject_key_and_ignores_top_level_id() {
    let source_root = cityjsonseq_root();
    let sample = find_first(&source_root, "city.jsonl", true);
    let fixture = derive_cityjson_seq_fixture(
        &sample,
        &[feature_line(
            "ignored-cityjson-seq-id",
            &[("cityjson-seq-key-a", 0), ("cityjson-seq-key-b", 10)],
        )],
        "cityjson-seq-cityobject-keys",
    );

    let index_path = temp_index_path("cityjson-seq-cityobject-keys");
    let mut index = CityIndex::open(
        StorageLayout::Ndjson {
            paths: vec![fixture],
        },
        &index_path,
    )
    .expect("CityJSONSeq index should open");
    index.reindex().expect("CityJSONSeq reindex should succeed");

    assert!(
        index
            .get("ignored-cityjson-seq-id")
            .expect("top-level id lookup should succeed")
            .is_none()
    );

    for feature_id in ["cityjson-seq-key-a", "cityjson-seq-key-b"] {
        let model = index
            .get(feature_id)
            .expect("cityobject key lookup should succeed")
            .expect("cityobject key should be indexed");
        assert!(model_contains_id(&model, "cityjson-seq-key-a"));
        assert!(model_contains_id(&model, "cityjson-seq-key-b"));
    }
}

/// Input: two CityJSONSeq feature lines that reuse the same CityObject key at different positions.
/// Assertions: plural lookup returns both duplicate occurrences in increasing record order.
#[test]
fn cityjson_seq_allows_duplicate_cityobject_keys() {
    let source_root = cityjsonseq_root();
    let sample = find_first(&source_root, "city.jsonl", true);
    let fixture = derive_cityjson_seq_fixture(
        &sample,
        &[
            feature_line("ignored-first", &[("duplicate-cityjson-seq-key", 0)]),
            feature_line("ignored-second", &[("duplicate-cityjson-seq-key", 20)]),
        ],
        "cityjson-seq-duplicate-keys",
    );

    let index_path = temp_index_path("cityjson-seq-duplicate-keys");
    let mut index = CityIndex::open(
        StorageLayout::Ndjson {
            paths: vec![fixture],
        },
        &index_path,
    )
    .expect("CityJSONSeq index should open");
    index
        .reindex()
        .expect("duplicate CityJSONSeq ids should reindex");

    let refs = index
        .lookup_feature_refs("duplicate-cityjson-seq-key")
        .expect("plural lookup should succeed");
    assert_eq!(refs.len(), 2, "both duplicate aliases should be indexed");
    assert!(refs[0].row_id < refs[1].row_id);
}

fn derive_small_cityjson_seq_fixture(source: &Path) -> std::path::PathBuf {
    let contents = fs::read_to_string(source).expect("sample CityJSONSeq tile must be readable");
    let mut lines = contents.lines();
    let metadata = lines.next().expect("sample tile must contain metadata");
    let path = std::env::temp_dir().join(format!(
        "cityjson-index-cityjson-seq-sample-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after the unix epoch")
            .as_nanos()
    ));
    let feature = serde_json::json!({
        "type": "CityJSONFeature",
        "id": "cityjson-seq-test-feature",
        "CityObjects": {
            "cityjson-seq-test-feature": {
                "type": "Building",
                "geometry": [{
                    "type": "MultiSurface",
                    "lod": "1.0",
                    "boundaries": [[[0, 1, 2]]]
                }]
            }
        },
        "vertices": [
            [0, 0, 0],
            [1, 0, 0],
            [0, 1, 0]
        ]
    });

    fs::write(
        &path,
        format!(
            "{metadata}\n{}\n",
            serde_json::to_string(&feature).expect("feature JSON")
        ),
    )
    .expect("derived CityJSONSeq fixture must be writable");
    path
}

fn derive_cityjson_seq_fixture(
    source: &Path,
    features: &[serde_json::Value],
    label: &str,
) -> std::path::PathBuf {
    let contents = fs::read_to_string(source).expect("sample CityJSONSeq tile must be readable");
    let metadata = contents
        .lines()
        .next()
        .expect("sample tile must contain metadata");
    let path = std::env::temp_dir().join(format!(
        "cityjson-index-{label}-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after the unix epoch")
            .as_nanos()
    ));
    let mut output = String::from(metadata);
    output.push('\n');
    for feature in features {
        output.push_str(&serde_json::to_string(feature).expect("feature JSON"));
        output.push('\n');
    }
    fs::write(&path, output).expect("derived CityJSONSeq fixture must be writable");
    path
}

fn feature_line(top_level_id: &str, objects: &[(&str, i64)]) -> serde_json::Value {
    let mut cityobjects = serde_json::Map::new();
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

    serde_json::json!({
        "type": "CityJSONFeature",
        "id": top_level_id,
        "CityObjects": cityobjects,
        "vertices": vertices
    })
}
