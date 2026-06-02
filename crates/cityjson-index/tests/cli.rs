#![allow(
    clippy::let_and_return,
    clippy::redundant_closure_for_method_calls,
    clippy::similar_names,
    clippy::too_many_lines
)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cityjson_index::{CityIndex, StorageLayout};
use common::{
    bbox_for_model, cityjson_feature, cityjson_root, cityjsonseq_root, feature_files_root,
    find_first, hierarchy_vertices, materialize_subset, shared_child_cityobjects, temp_index_path,
    triangle_geometry, write_cityjson_fixture,
};
use serde_json::Value;
use walkdir::WalkDir;

#[test]
fn cli_get_and_query_emit_cityjsonseq_streams() {
    let source_root = feature_files_root();
    let sample = find_first(&source_root.join("features"), "city.jsonl", true);
    let root = materialize_subset(
        "cli-feature-files",
        &source_root,
        &[source_root.join("metadata.json"), sample.clone()],
    );
    let feature_id = feature_id_from_feature_file(&sample);
    let index_path = temp_index_path("cli");

    run_cli([
        "index",
        "--layout",
        "feature-files",
        "--root",
        root.as_os_str().to_str().expect("root path must be utf-8"),
        "--index",
        index_path
            .as_os_str()
            .to_str()
            .expect("index path must be utf-8"),
    ]);

    let model = load_model(&root, &index_path, &feature_id);
    let bbox = bbox_for_model(&model).expect("bbox should be computable");

    let stdout = run_cli([
        "get",
        "--layout",
        "feature-files",
        "--root",
        root.as_os_str().to_str().expect("root path must be utf-8"),
        "--index",
        index_path
            .as_os_str()
            .to_str()
            .expect("index path must be utf-8"),
        "--id",
        &feature_id,
    ]);
    let stdout_lines = parse_json_lines(&stdout);
    assert_eq!(
        stdout_lines.len(),
        2,
        "get should emit one header and one feature"
    );
    assert_eq!(stdout_lines[0]["type"], "CityJSON");
    assert!(
        stdout_lines[0]["CityObjects"]
            .as_object()
            .is_some_and(|objects| objects.is_empty()),
        "header CityObjects should be empty"
    );
    assert_eq!(stdout_lines[1]["type"], "CityJSONFeature");
    assert_eq!(stdout_lines[1]["id"], feature_id);

    let output_path = temp_output_path("cli-get");
    let _ = run_cli([
        "get",
        "--layout",
        "feature-files",
        "--root",
        root.as_os_str().to_str().expect("root path must be utf-8"),
        "--index",
        index_path
            .as_os_str()
            .to_str()
            .expect("index path must be utf-8"),
        "--id",
        &feature_id,
        "--output",
        output_path
            .as_os_str()
            .to_str()
            .expect("output path must be utf-8"),
    ]);
    assert_eq!(
        parse_json_lines(
            &fs::read_to_string(&output_path).expect("get output file should be readable")
        ),
        stdout_lines
    );

    let query_stdout = run_cli([
        "query",
        "--layout",
        "feature-files",
        "--root",
        root.as_os_str().to_str().expect("root path must be utf-8"),
        "--index",
        index_path
            .as_os_str()
            .to_str()
            .expect("index path must be utf-8"),
        "--min-x",
        &bbox.min_x.to_string(),
        "--max-x",
        &bbox.max_x.to_string(),
        "--min-y",
        &bbox.min_y.to_string(),
        "--max-y",
        &bbox.max_y.to_string(),
    ]);
    let query_lines = parse_json_lines(&query_stdout);
    assert_eq!(
        query_lines.len(),
        2,
        "query should emit one header and one feature for the selected bbox"
    );
    assert_eq!(query_lines[0]["type"], "CityJSON");
    assert_eq!(query_lines[1]["type"], "CityJSONFeature");
    assert_eq!(query_lines[1]["id"], feature_id);

    let query_output_path = temp_output_path("cli-query");
    let _ = run_cli([
        "query",
        "--layout",
        "feature-files",
        "--root",
        root.as_os_str().to_str().expect("root path must be utf-8"),
        "--index",
        index_path
            .as_os_str()
            .to_str()
            .expect("index path must be utf-8"),
        "--min-x",
        &bbox.min_x.to_string(),
        "--max-x",
        &bbox.max_x.to_string(),
        "--min-y",
        &bbox.min_y.to_string(),
        "--max-y",
        &bbox.max_y.to_string(),
        "--output",
        query_output_path
            .as_os_str()
            .to_str()
            .expect("output path must be utf-8"),
    ]);
    assert_eq!(
        parse_json_lines(
            &fs::read_to_string(&query_output_path).expect("query output file should be readable")
        ),
        query_lines
    );
}

#[test]
fn cli_query_rejects_incompatible_metadata_roots() {
    let source_root = feature_files_root();
    let mut features = first_two_feature_files(&source_root.join("features"));
    features.sort();
    let feature_a = features[0].clone();
    let feature_b = features[1].clone();
    let feature_a_id = feature_id_from_feature_file(&feature_a);
    let feature_b_id = feature_id_from_feature_file(&feature_b);

    let root = temp_fixture_root("cli-metadata-mismatch");
    fs::create_dir_all(root.join("features")).expect("features root should be creatable");
    fs::create_dir_all(root.join("alt")).expect("alt root should be creatable");
    fs::copy(
        source_root.join("metadata.json"),
        root.join("metadata.json"),
    )
    .expect("root metadata should copy");
    fs::copy(
        &feature_a,
        root.join("features").join("feature-a.city.jsonl"),
    )
    .expect("feature A should copy");

    let mut alt_metadata: Value =
        serde_json::from_slice(&fs::read(source_root.join("metadata.json")).expect("metadata"))
            .expect("source metadata should parse");
    alt_metadata
        .as_object_mut()
        .expect("metadata root should be an object")
        .insert(
            "cli-test-note".to_owned(),
            Value::String("mismatch".to_owned()),
        );
    fs::write(
        root.join("alt").join("metadata.json"),
        serde_json::to_vec(&alt_metadata).expect("alt metadata should serialize"),
    )
    .expect("alt metadata should copy");
    fs::copy(&feature_b, root.join("alt").join("feature-b.city.jsonl"))
        .expect("feature B should copy");

    let index_path = temp_index_path("cli-metadata-mismatch");
    run_cli([
        "index",
        "--layout",
        "feature-files",
        "--root",
        root.as_os_str().to_str().expect("root path must be utf-8"),
        "--index",
        index_path
            .as_os_str()
            .to_str()
            .expect("index path must be utf-8"),
    ]);

    let index = CityIndex::open(
        StorageLayout::FeatureFiles {
            root: root.clone(),
            metadata_glob: "**/metadata.json".to_owned(),
            feature_glob: "**/*.city.jsonl".to_owned(),
        },
        &index_path,
    )
    .expect("index should open");
    let model_a = index
        .get(&feature_a_id)
        .expect("feature A should load")
        .expect("feature A should be indexed");
    let model_b = index
        .get(&feature_b_id)
        .expect("feature B should load")
        .expect("feature B should be indexed");
    let bbox_a = bbox_for_model(&model_a).expect("feature A bbox should compute");
    let bbox_b = bbox_for_model(&model_b).expect("feature B bbox should compute");
    let min_x = bbox_a.min_x.min(bbox_b.min_x).to_string();
    let max_x = bbox_a.max_x.max(bbox_b.max_x).to_string();
    let min_y = bbox_a.min_y.min(bbox_b.min_y).to_string();
    let max_y = bbox_a.max_y.max(bbox_b.max_y).to_string();

    let output = run_cli_output([
        "query",
        "--layout",
        "feature-files",
        "--root",
        root.as_os_str().to_str().expect("root path must be utf-8"),
        "--index",
        index_path
            .as_os_str()
            .to_str()
            .expect("index path must be utf-8"),
        "--min-x",
        &min_x,
        "--max-x",
        &max_x,
        "--min-y",
        &min_y,
        "--max-y",
        &max_y,
    ]);
    assert!(
        !output.status.success(),
        "query should fail for incompatible metadata roots"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("incompatible metadata roots"),
        "query should explain the metadata mismatch"
    );
}

#[test]
fn cli_dataset_mode_get_query_and_metadata_work() {
    let source_root = feature_files_root();
    let sample = find_first(&source_root.join("features"), "city.jsonl", true);
    let root = materialize_subset(
        "cli-dataset-mode",
        &source_root,
        &[source_root.join("metadata.json"), sample.clone()],
    );
    let feature_id = feature_id_from_feature_file(&sample);

    run_cli(["index", root.to_str().expect("root path must be utf-8")]);

    let index_path = root.join(".cityjson-index.sqlite");
    assert!(
        index_path.exists(),
        "dataset-mode index should use sidecar path"
    );

    let model = load_model(&root, &index_path, &feature_id);
    let bbox = bbox_for_model(&model).expect("bbox should be computable");

    let get_stdout = run_cli([
        "get",
        root.to_str().expect("root path must be utf-8"),
        "--id",
        &feature_id,
    ]);
    let get_lines = parse_json_lines(&get_stdout);
    assert_eq!(get_lines[0]["type"], "CityJSON");
    assert_eq!(get_lines[1]["id"], feature_id);

    let query_stdout = run_cli([
        "query",
        root.to_str().expect("root path must be utf-8"),
        "--min-x",
        &bbox.min_x.to_string(),
        "--max-x",
        &bbox.max_x.to_string(),
        "--min-y",
        &bbox.min_y.to_string(),
        "--max-y",
        &bbox.max_y.to_string(),
    ]);
    let query_lines = parse_json_lines(&query_stdout);
    assert_eq!(query_lines[0]["type"], "CityJSON");
    assert_eq!(query_lines[1]["id"], feature_id);

    let metadata_stdout = run_cli(["metadata", root.to_str().expect("utf-8 path")]);
    let metadata: Value =
        serde_json::from_str(&metadata_stdout).expect("metadata output should be valid JSON");
    assert!(
        metadata.as_array().is_some_and(|items| !items.is_empty()),
        "metadata command should return cached metadata entries"
    );
}

#[test]
fn cli_inspect_autodetects_all_supported_layouts() {
    for (root, expected_layout) in [
        (feature_files_root(), "feature-files"),
        (cityjsonseq_root(), "cityjson-seq"),
        (cityjson_root(), "cityjson"),
    ] {
        let output = run_cli([
            "inspect",
            root.to_str().expect("root path must be utf-8"),
            "--json",
        ]);
        let report: Value =
            serde_json::from_str(&output).expect("inspect output should be valid JSON");
        assert_eq!(report["layout"], expected_layout);
        let canonical_root =
            fs::canonicalize(&root).expect("dataset root should canonicalize for inspect");
        assert_eq!(
            report["dataset_root"].as_str(),
            Some(canonical_root.to_str().expect("root path must be utf-8"))
        );
    }
}

#[test]
fn cli_inspect_and_validate_report_missing_and_stale_indexes() {
    let source_root = feature_files_root();
    let sample = find_first(&source_root.join("features"), "city.jsonl", true);
    let root = materialize_subset(
        "cli-status",
        &source_root,
        &[source_root.join("metadata.json"), sample.clone()],
    );

    let inspect_missing = run_cli([
        "inspect",
        root.to_str().expect("root path must be utf-8"),
        "--json",
    ]);
    let missing_report: Value =
        serde_json::from_str(&inspect_missing).expect("inspect output should be valid JSON");
    assert_eq!(missing_report["index"]["exists"], false);

    run_cli(["index", root.to_str().expect("root path must be utf-8")]);

    let inspect_present = run_cli([
        "inspect",
        root.to_str().expect("root path must be utf-8"),
        "--json",
    ]);
    let present_report: Value =
        serde_json::from_str(&inspect_present).expect("inspect output should be valid JSON");
    assert_eq!(present_report["index"]["exists"], true);
    assert_eq!(present_report["index"]["covered"], true);

    let copied_feature = root.join(
        sample
            .strip_prefix(&source_root)
            .expect("copied feature should live under the dataset root"),
    );
    let mut bytes = fs::read(&copied_feature).expect("feature file should be readable");
    bytes.push(b'\n');
    fs::write(&copied_feature, bytes).expect("feature file should be writable");

    let output = run_cli_output([
        "validate",
        root.to_str().expect("root path must be utf-8"),
        "--json",
    ]);
    assert!(
        !output.status.success(),
        "validate should fail for stale data"
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("validate json output should parse");
    assert_eq!(report["ok"], false);
    assert!(
        report["inspection"]["index"]["changed_feature_paths"]
            .as_array()
            .is_some_and(|paths| !paths.is_empty()),
        "validate should report the changed feature path"
    );
}

fn load_model(root: &Path, index_path: &Path, feature_id: &str) -> cityjson_lib::CityModel {
    let index = CityIndex::open(
        StorageLayout::FeatureFiles {
            root: root.to_path_buf(),
            metadata_glob: "**/metadata.json".to_owned(),
            feature_glob: "**/*.city.jsonl".to_owned(),
        },
        index_path,
    )
    .expect("index should open");
    let model = index
        .get(feature_id)
        .expect("get should succeed")
        .expect("feature should be indexed");
    model
}

fn feature_id_from_feature_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("feature file should be readable");
    let value: Value = serde_json::from_slice(&bytes).expect("feature file should be valid JSON");
    value
        .get("id")
        .and_then(Value::as_str)
        .expect("feature file must contain an id")
        .to_owned()
}

fn parse_json_lines(output: &str) -> Vec<Value> {
    output
        .lines()
        .map(|line| serde_json::from_str(line).expect("output line should be valid JSON"))
        .collect()
}

fn run_cli<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = run_cli_output(args);
    assert!(
        output.status.success(),
        "cjindex command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("cjindex stdout should be utf-8")
}

fn run_cli_output<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let binary = std::env::var_os("CARGO_BIN_EXE_cjindex").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("debug")
                .join(format!("cjindex{}", std::env::consts::EXE_SUFFIX))
        },
        PathBuf::from,
    );
    let output = Command::new(binary)
        .args(args)
        .output()
        .expect("cjindex command should run");
    output
}

fn temp_output_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "cityjson-index-{label}-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    ))
}

fn temp_fixture_root(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "cityjson-index-{label}-{}.dir",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("fixture root should be creatable");
    path
}

fn first_two_feature_files(root: &Path) -> Vec<PathBuf> {
    let mut features = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        if entry.metadata().map_or(true, |meta| meta.len() == 0) {
            continue;
        }
        features.push(entry.path().to_path_buf());
        if features.len() == 2 {
            break;
        }
    }
    assert_eq!(features.len(), 2, "expected at least two feature files");
    features
}

#[test]
fn cli_layout_cityjson_seq_is_accepted() {
    let source = find_first(&cityjsonseq_root(), "city.jsonl", true);
    let index_path = temp_index_path("cli-layout-cityjson-seq");
    let output = run_cli_output([
        "index",
        "--layout",
        "cityjson-seq",
        "--paths",
        source.to_str().expect("source path must be utf-8"),
        "--index",
        index_path.to_str().expect("index path must be utf-8"),
    ]);

    assert!(
        output.status.success(),
        "cityjson-seq layout should be accepted: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_layout_ndjson_is_rejected() {
    let source = find_first(&cityjsonseq_root(), "city.jsonl", true);
    let index_path = temp_index_path("cli-layout-ndjson-rejected");
    let output = run_cli_output([
        "index",
        "--layout",
        "ndjson",
        "--paths",
        source.to_str().expect("source path must be utf-8"),
        "--index",
        index_path.to_str().expect("index path must be utf-8"),
    ]);

    assert!(
        !output.status.success(),
        "removed ndjson alias should be rejected"
    );
}

#[test]
fn cli_get_child_id_emits_all_valid_containing_packages() {
    let root = write_cityjson_fixture(
        "cli-shared-child",
        shared_child_cityobjects(),
        hierarchy_vertices(),
    );
    let index_path = temp_index_path("cli-shared-child");
    run_cli([
        "index",
        "--layout",
        "cityjson",
        "--paths",
        root.to_str().expect("root path must be utf-8"),
        "--index",
        index_path.to_str().expect("index path must be utf-8"),
    ]);

    let lines = parse_json_lines(&run_cli([
        "get",
        "--layout",
        "cityjson",
        "--paths",
        root.to_str().expect("root path must be utf-8"),
        "--index",
        index_path.to_str().expect("index path must be utf-8"),
        "--id",
        "shared-part",
    ]));
    assert_eq!(
        lines.len(),
        3,
        "header plus both containing packages should be emitted"
    );
    assert!(lines[1]["CityObjects"].get("building-a").is_some());
    assert!(lines[2]["CityObjects"].get("building-b").is_some());
}

#[test]
fn cli_get_rejects_incompatible_metadata_roots() {
    let root = temp_fixture_root("cli-get-metadata-mismatch");
    fs::create_dir_all(root.join("features")).expect("feature root should be creatable");
    fs::create_dir_all(root.join("alt")).expect("alternate feature root should be creatable");
    let metadata: Value = serde_json::from_slice(
        &fs::read(feature_files_root().join("metadata.json")).expect("metadata fixture"),
    )
    .expect("metadata fixture should parse");
    fs::write(
        root.join("metadata.json"),
        serde_json::to_vec(&metadata).expect("metadata should serialize"),
    )
    .expect("metadata should be writable");
    let mut alt_metadata = metadata;
    alt_metadata["metadata"]["title"] = Value::String("incompatible".to_owned());
    fs::write(
        root.join("alt/metadata.json"),
        serde_json::to_vec(&alt_metadata).expect("alt metadata should serialize"),
    )
    .expect("alt metadata should be writable");
    write_cli_feature_file(&root.join("features/first.city.jsonl"), "first", 0);
    write_cli_feature_file(&root.join("alt/second.city.jsonl"), "second", 10);
    let index_path = temp_index_path("cli-get-metadata-mismatch");
    run_cli([
        "index",
        "--layout",
        "feature-files",
        "--root",
        root.to_str().expect("root path must be utf-8"),
        "--index",
        index_path.to_str().expect("index path must be utf-8"),
    ]);

    let output = run_cli_output([
        "get",
        "--layout",
        "feature-files",
        "--root",
        root.to_str().expect("root path must be utf-8"),
        "--index",
        index_path.to_str().expect("index path must be utf-8"),
        "--id",
        "duplicate",
    ]);
    assert!(
        !output.status.success(),
        "get should reject incompatible metadata roots"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("incompatible metadata roots"));
}

#[test]
fn cli_query_emits_each_matching_package_once() {
    let root = write_cityjson_fixture(
        "cli-query-package-dedup",
        common::hierarchy_cityobjects(),
        hierarchy_vertices(),
    );
    let index_path = temp_index_path("cli-query-package-dedup");
    run_cli([
        "index",
        "--layout",
        "cityjson",
        "--paths",
        root.to_str().expect("root path"),
        "--index",
        index_path.to_str().expect("index path"),
    ]);

    let lines = parse_json_lines(&run_cli([
        "query",
        "--layout",
        "cityjson",
        "--paths",
        root.to_str().expect("root path"),
        "--index",
        index_path.to_str().expect("index path"),
        "--min-x",
        "0",
        "--max-x",
        "100",
        "--min-y",
        "0",
        "--max-y",
        "100",
    ]));
    assert_eq!(
        lines.len(),
        2,
        "header plus one containing package should be emitted"
    );
}

#[test]
fn cli_inspect_reports_normalized_counts_and_schema() {
    let root = write_cityjson_fixture(
        "cli-inspect-normalized",
        common::hierarchy_cityobjects(),
        hierarchy_vertices(),
    );
    run_cli(["index", root.to_str().expect("root path must be utf-8")]);

    let report: Value = serde_json::from_str(&run_cli([
        "inspect",
        root.to_str().expect("root path must be utf-8"),
        "--json",
    ]))
    .expect("inspect output should parse");
    assert_eq!(report["index"]["schema_version"], 1);
    assert_eq!(report["index"]["indexed_source_count"], 1);
    assert_eq!(report["index"]["indexed_package_count"], 1);
    assert_eq!(report["index"]["indexed_cityobject_count"], 2);
    assert_eq!(report["index"]["indexed_cityobject_relationship_count"], 1);
}

fn write_cli_feature_file(path: &Path, id: &str, base: i64) {
    let feature = cityjson_feature(
        id,
        serde_json::json!({"duplicate": {"type": "Building", "geometry": [triangle_geometry(0, "1.0")]}}),
        serde_json::json!([[base, 0, 0], [base + 1, 0, 1], [base, 1, 2]]),
    );
    fs::write(
        path,
        serde_json::to_vec(&feature).expect("feature should serialize"),
    )
    .expect("feature should be writable");
}
