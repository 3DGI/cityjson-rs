#![allow(
    clippy::needless_pass_by_value,
    reason = "fixture builders intentionally take owned JSON values"
)]

use std::fs;
use std::path::{Path, PathBuf};

use cityjson_index::{CityIndex, StorageLayout};
use serde_json::{Value, json};

use super::temp_fixture_root;

pub fn hierarchy_cityobjects() -> Value {
    json!({
        "building": {
            "type": "Building",
            "children": ["part"]
        },
        "part": {
            "type": "BuildingPart",
            "parents": ["building"],
            "geometry": [triangle_geometry(0, "2.0")]
        }
    })
}

pub fn hierarchy_vertices() -> Value {
    json!([[10, 20, 30], [14, 20, 36], [10, 26, 33]])
}

pub fn shared_child_cityobjects() -> Value {
    json!({
        "building-a": {
            "type": "Building",
            "children": ["shared-part"]
        },
        "building-b": {
            "type": "Building",
            "children": ["shared-part"]
        },
        "shared-part": {
            "type": "BuildingPart",
            "parents": ["building-a", "building-b"],
            "geometry": [triangle_geometry(0, "2.0")]
        }
    })
}

pub fn cityjson_feature(id: &str, cityobjects: Value, vertices: Value) -> Value {
    json!({
        "type": "CityJSONFeature",
        "id": id,
        "CityObjects": cityobjects,
        "vertices": vertices
    })
}

pub fn write_cityjson_fixture(label: &str, cityobjects: Value, vertices: Value) -> PathBuf {
    let root = temp_fixture_root(label);
    write_json(
        &root.join("fixture.city.json"),
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": transform(),
            "metadata": {"title": label},
            "CityObjects": cityobjects,
            "vertices": vertices
        }),
    );
    root
}

pub fn write_cityjson_seq_fixture(label: &str, features: &[Value]) -> PathBuf {
    let root = temp_fixture_root(label);
    let path = root.join("fixture.city.jsonl");
    let mut lines = vec![
        serde_json::to_string(&json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": transform(),
            "metadata": {"title": label},
            "CityObjects": {},
            "vertices": []
        }))
        .expect("CityJSONSeq header should serialize"),
    ];
    lines.extend(
        features
            .iter()
            .map(|feature| serde_json::to_string(feature).expect("feature should serialize")),
    );
    fs::write(path, format!("{}\n", lines.join("\n")))
        .expect("CityJSONSeq fixture should be writable");
    root
}

pub fn write_feature_files_fixture(label: &str, feature: &Value) -> PathBuf {
    let root = temp_fixture_root(label);
    write_json(
        &root.join("metadata.json"),
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": transform(),
            "metadata": {"title": label},
            "CityObjects": {},
            "vertices": []
        }),
    );
    let feature_path = root.join("features/fixture.city.jsonl");
    fs::create_dir_all(feature_path.parent().expect("feature parent"))
        .expect("feature parent should be writable");
    write_json(&feature_path, feature);
    root
}

pub fn open_cityjson_index(root: &Path, index_path: &Path) -> CityIndex {
    CityIndex::open(
        StorageLayout::CityJson {
            paths: vec![root.to_path_buf()],
        },
        index_path,
    )
    .expect("CityJSON index should open")
}

pub fn open_cityjson_seq_index(root: &Path, index_path: &Path) -> CityIndex {
    CityIndex::open(
        StorageLayout::Ndjson {
            paths: vec![root.to_path_buf()],
        },
        index_path,
    )
    .expect("CityJSONSeq index should open")
}

pub fn open_feature_files_index(root: &Path, index_path: &Path) -> CityIndex {
    CityIndex::open(
        StorageLayout::FeatureFiles {
            root: root.to_path_buf(),
            metadata_glob: "**/metadata.json".to_owned(),
            feature_glob: "**/*.city.jsonl".to_owned(),
        },
        index_path,
    )
    .expect("feature-files index should open")
}

pub fn triangle_geometry(start: usize, lod: &str) -> Value {
    json!({
        "type": "MultiSurface",
        "lod": lod,
        "boundaries": [[[start, start + 1, start + 2]]]
    })
}

fn transform() -> Value {
    json!({"scale": [1.0, 1.0, 1.0], "translate": [0.0, 0.0, 0.0]})
}

fn write_json(path: &Path, value: &Value) {
    fs::write(
        path,
        serde_json::to_vec(value).expect("fixture should serialize"),
    )
    .expect("fixture should be writable");
}
