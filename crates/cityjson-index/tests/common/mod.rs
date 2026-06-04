#![allow(dead_code)]

mod normalized;

#[allow(unused_imports)]
pub use normalized::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use cityjson_index::BBox;
use cityjson_lib::{CityModel, Result};
use serde_json::Value;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn data_root() -> PathBuf {
    repo_root().join("tests/data")
}

pub fn shared_corpus_root() -> Option<PathBuf> {
    std::env::var_os("CITYJSON_SHARED_CORPUS_ROOT").map(PathBuf::from)
}

pub fn basisvoorziening_artifact() -> Option<PathBuf> {
    shared_corpus_root().map(|root| {
        root.join("artifacts/acquired/basisvoorziening-3d/2022/3d_volledig_84000_450000.city.json")
    })
}

pub fn feature_files_root() -> PathBuf {
    data_root().join("feature-files")
}

pub fn cityjsonseq_root() -> PathBuf {
    data_root().join("cityjsonseq")
}

pub fn cityjson_root() -> PathBuf {
    data_root().join("cityjson")
}

pub fn temp_index_path(label: &str) -> PathBuf {
    unique_temp_path(label, "sqlite")
}

pub fn temp_output_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "cityjson-index-{label}-{}.jsonl",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the unix epoch")
            .as_nanos()
    ))
}

pub fn temp_fixture_root(label: &str) -> PathBuf {
    let path = unique_temp_path(label, "dir");
    fs::create_dir_all(&path).expect("temp fixture root should be creatable");
    path
}

pub fn materialize_subset(label: &str, source_root: &Path, files: &[PathBuf]) -> PathBuf {
    let dest_root = temp_fixture_root(label);
    for source in files {
        let rel = source
            .strip_prefix(source_root)
            .expect("subset file must live under the source root");
        let dest = dest_root.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).expect("subset parent directory should be creatable");
        }
        fs::copy(source, &dest).expect("subset file should copy");
    }
    dest_root
}

pub fn run_cli<I, S>(args: I) -> String
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

pub fn run_cli_output<I, S>(args: I) -> Output
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
    Command::new(binary)
        .args(args)
        .output()
        .expect("cjindex command should run")
}

pub fn parse_json_lines(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .map(|line| serde_json::from_str(line).expect("output line should be valid JSON"))
        .collect()
}

pub fn first_two_feature_files(root: &Path) -> Vec<PathBuf> {
    let mut features = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
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

fn unique_temp_path(label: &str, suffix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after the unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cityjson-index-{label}-{unique}.{suffix}"))
}

pub fn find_first(root: &std::path::Path, suffix: &str, require_nonempty: bool) -> PathBuf {
    for entry in WalkDir::new(root) {
        let entry = entry.expect("directory entry");
        if !entry.file_type().is_file() || !entry.path().to_string_lossy().ends_with(suffix) {
            continue;
        }
        if require_nonempty && entry.metadata().map_or(true, |meta| meta.len() == 0) {
            continue;
        }
        return entry.path().to_path_buf();
    }

    panic!("no {suffix} file found in {}", root.display());
}

pub fn model_contains_id(model: &CityModel, id: &str) -> bool {
    let value = model_json(model).expect("serialized model JSON");
    value["CityObjects"]
        .as_object()
        .is_some_and(|cityobjects| cityobjects.contains_key(id))
}

pub fn bbox_for_model(model: &CityModel) -> Result<BBox> {
    let value = model_json(model)?;
    let vertices = value
        .get("vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| cityjson_lib::Error::Import("model JSON is missing vertices".into()))?;
    let transform = value
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| cityjson_lib::Error::Import("model JSON is missing transform".into()))?;
    let scale = parse_transform_component(transform, "scale")?;
    let translate = parse_transform_component(transform, "translate")?;

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for vertex in vertices {
        let coords = vertex
            .as_array()
            .ok_or_else(|| cityjson_lib::Error::Import("vertex must be an array".into()))?;
        if coords.len() != 3 {
            return Err(cityjson_lib::Error::Import(
                "vertex must have three coordinates".into(),
            ));
        }
        let x = translate[0] + scale[0] * value_as_f64(&coords[0])?;
        let y = translate[1] + scale[1] * value_as_f64(&coords[1])?;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return Err(cityjson_lib::Error::Import(
            "could not compute a finite bbox from the model".into(),
        ));
    }

    Ok(BBox {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn model_json(model: &CityModel) -> Result<Value> {
    let text = cityjson_lib::json::to_string(model)?;
    serde_json::from_str(&text).map_err(|error| cityjson_lib::Error::Import(error.to_string()))
}

fn parse_transform_component(
    transform: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<[f64; 3]> {
    let values = transform
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| cityjson_lib::Error::Import(format!("transform is missing {key}")))?;
    if values.len() != 3 {
        return Err(cityjson_lib::Error::Import(format!(
            "transform {key} must contain three values"
        )));
    }
    Ok([
        value_as_f64(&values[0])?,
        value_as_f64(&values[1])?,
        value_as_f64(&values[2])?,
    ])
}

fn value_as_f64(value: &Value) -> Result<f64> {
    value
        .as_f64()
        .ok_or_else(|| cityjson_lib::Error::Import("expected a numeric value".into()))
}
