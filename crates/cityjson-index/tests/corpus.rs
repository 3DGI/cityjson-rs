mod common;

use std::fs;

use common::basisvoorziening_artifact;
use serde_json::Value;

#[test]
fn basisvoorziening_artifact_is_parseable_cityjson() {
    let Some(artifact) = basisvoorziening_artifact() else {
        return;
    };

    assert!(
        artifact.exists(),
        "pinned Basisvoorziening artifact should exist at {}",
        artifact.display()
    );

    let bytes = fs::read(&artifact).expect("artifact should be readable");
    let document: Value = serde_json::from_slice(&bytes).expect("artifact should be valid JSON");
    assert_eq!(document["type"], "CityJSON");
    assert!(
        document["CityObjects"]
            .as_object()
            .is_some_and(|objects| !objects.is_empty()),
        "artifact should contain CityObjects"
    );
    assert!(
        document.get("vertices").is_some(),
        "artifact should contain vertices"
    );
}

#[test]
fn shared_corpus_ops_3dbag_covers_normalization_cases() {
    let Some(root) = common::shared_corpus_root() else {
        return;
    };
    let catalog: Value = serde_json::from_slice(
        &fs::read(root.join("catalog/cases.json"))
            .expect("shared corpus catalog should be readable"),
    )
    .expect("shared corpus catalog should be valid JSON");
    let case = corpus_case(&catalog, "ops_3dbag");
    assert_eq!(case["representation"], "cityjson");
    for invariant in [
        "bounding_box_stable",
        "hierarchy_traversal_complete",
        "geometry_preserved",
        "semantic_surfaces_preserved",
    ] {
        assert!(
            case["assertions"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item == invariant))
        );
    }

    let acquisition: Value = serde_json::from_slice(
        &fs::read(
            root.join(
                case["artifact_paths"]["acquisition"]
                    .as_str()
                    .expect("acquisition manifest path"),
            ),
        )
        .expect("ops_3dbag acquisition manifest should be readable"),
    )
    .expect("ops_3dbag acquisition manifest should be valid JSON");
    let artifact = root.join(
        acquisition["outputs"][0]["path"]
            .as_str()
            .expect("ops_3dbag artifact path"),
    );
    let document: Value =
        serde_json::from_slice(&fs::read(artifact).expect("ops_3dbag artifact should be readable"))
            .expect("ops_3dbag artifact should be valid JSON");
    let objects = document["CityObjects"]
        .as_object()
        .expect("ops_3dbag CityObjects");
    assert_eq!(objects.len(), 2);
    assert!(objects.values().any(|object| {
        object["children"]
            .as_array()
            .is_some_and(|children| !children.is_empty())
    }));
    assert!(objects.values().any(|object| {
        object["geometry"]
            .as_array()
            .is_some_and(|geometries| geometries.len() > 1)
    }));
    let vertices = document["vertices"].as_array().expect("ops_3dbag vertices");
    let min_z = vertices
        .iter()
        .filter_map(|vertex| vertex[2].as_i64())
        .min()
        .expect("minimum Z");
    let max_z = vertices
        .iter()
        .filter_map(|vertex| vertex[2].as_i64())
        .max()
        .expect("maximum Z");
    assert!(min_z < max_z);
}

#[test]
fn shared_corpus_cityjson_seq_case_preserves_feature_boundaries() {
    let Some(root) = common::shared_corpus_root() else {
        return;
    };
    let catalog: Value = serde_json::from_slice(
        &fs::read(root.join("catalog/cases.json"))
            .expect("shared corpus catalog should be readable"),
    )
    .expect("shared corpus catalog should be valid JSON");
    let case = corpus_case(&catalog, "ops_cityjsonseq_feature_root_id_not_shared");
    let artifact = root.join(
        case["artifact_paths"]["source"]
            .as_str()
            .expect("CityJSONSeq source path"),
    );
    let lines = fs::read_to_string(artifact).expect("CityJSONSeq case should be readable");
    let records = lines
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("CityJSONSeq record should parse"))
        .collect::<Vec<_>>();

    assert_eq!(records.len(), 3);
    assert_eq!(records[0]["type"], "CityJSON");
    assert_eq!(records[1]["id"], "building-1");
    assert_eq!(records[2]["id"], "building-2");
    assert!(records[1]["CityObjects"].get("building-1").is_some());
    assert!(records[2]["CityObjects"].get("building-2").is_some());
}

fn corpus_case<'a>(catalog: &'a Value, id: &str) -> &'a Value {
    catalog["cases"]
        .as_array()
        .expect("catalog cases should be an array")
        .iter()
        .find(|case| case["id"] == id)
        .expect("shared corpus case should exist")
}
