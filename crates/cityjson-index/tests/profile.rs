#![allow(
    clippy::doc_markdown,
    reason = "test docstrings use domain terminology plainly"
)]
#![allow(clippy::let_and_return)]

mod common;

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;

    use super::common::{
        feature_files_root, find_first, materialize_subset, run_cli_output, temp_output_path,
    };
    use serde_json::Value;

    #[test]
    fn profile_output_stays_separate_from_stdout() {
        let context = profile_success_context();
        assert_index_profile(&context.root, &context.profile_path);
        assert_inspect_profile(&context.root);
        assert_get_profile(&context.root, &context.feature_id);
        assert!(
            context.index_path.exists(),
            "index command should create the sidecar index"
        );
    }

    #[test]
    fn profile_output_is_written_for_failures() {
        let source_root = feature_files_root();
        let sample = find_first(&source_root.join("features"), "city.jsonl", true);
        let root = materialize_subset(
            "profile-failure",
            &source_root,
            &[source_root.join("metadata.json"), sample],
        );
        let profile_path = temp_output_path("profile-failure");

        let output = run_cli_output([
            "get",
            root.to_str().expect("root path must be utf-8"),
            "--id",
            "missing-feature",
            "--profile",
            profile_path.to_str().expect("profile path must be utf-8"),
        ]);
        assert!(
            !output.status.success(),
            "missing feature lookup should fail"
        );
        let profile: Value = serde_json::from_str(
            &fs::read_to_string(&profile_path).expect("profile output should be readable"),
        )
        .expect("profile json should parse");
        assert_eq!(profile["command"], "get");
        assert_eq!(profile["success"], false);
        assert!(
            profile["error"]
                .as_str()
                .is_some_and(|message| message.contains("missing-feature")),
            "failure profile should preserve the error message"
        );
    }

    fn feature_id_from_feature_file(path: &std::path::Path) -> String {
        let bytes = fs::read(path).expect("feature file should be readable");
        let value: Value =
            serde_json::from_slice(&bytes).expect("feature file should be valid JSON");
        value
            .get("id")
            .and_then(Value::as_str)
            .expect("feature file must contain an id")
            .to_owned()
    }

    struct ProfileContext {
        root: std::path::PathBuf,
        feature_id: String,
        index_path: std::path::PathBuf,
        profile_path: std::path::PathBuf,
    }

    fn profile_success_context() -> ProfileContext {
        let source_root = feature_files_root();
        let sample = find_first(&source_root.join("features"), "city.jsonl", true);
        let root = materialize_subset(
            "profile-success",
            &source_root,
            &[source_root.join("metadata.json"), sample.clone()],
        );
        let feature_id = feature_id_from_feature_file(&sample);
        let index_path = root.join(".cityjson-index.sqlite");
        let profile_path = temp_output_path("profile-success");

        let index_output = run_cli_output([
            "index",
            root.to_str().expect("root path must be utf-8"),
            "--profile",
            profile_path.to_str().expect("profile path must be utf-8"),
        ]);
        assert!(
            index_output.status.success(),
            "index command should succeed: {}",
            String::from_utf8_lossy(&index_output.stderr)
        );

        let profile: Value = serde_json::from_str(
            &fs::read_to_string(&profile_path).expect("profile output should be readable"),
        )
        .expect("profile json should parse");
        assert_eq!(profile["command"], "index");
        assert_eq!(profile["success"], true);
        assert_eq!(
            profile["worker_count"].as_u64(),
            Some(
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get) as u64
            ),
            "index profile should record the effective worker count"
        );
        assert_indexing_stage_names(&profile);

        ProfileContext {
            root,
            feature_id,
            index_path,
            profile_path,
        }
    }

    fn assert_index_profile(root: &std::path::Path, profile_path: &std::path::Path) {
        let profile_json: Value = serde_json::from_str(
            &fs::read_to_string(profile_path).expect("inspect profile should be readable"),
        )
        .expect("inspect profile should parse");
        assert_eq!(profile_json["command"], "index");
        assert_eq!(profile_json["success"], true);
        assert!(root.exists(), "profile context root should exist");
    }

    fn assert_inspect_profile(root: &std::path::Path) {
        let inspect_profile = temp_output_path("inspect-profile");
        let inspect_output = run_cli_output([
            "inspect",
            root.to_str().expect("root path must be utf-8"),
            "--json",
            "--profile",
            inspect_profile
                .to_str()
                .expect("profile path must be utf-8"),
        ]);
        assert!(
            inspect_output.status.success(),
            "inspect command should succeed: {}",
            String::from_utf8_lossy(&inspect_output.stderr)
        );
        let inspect_stdout =
            String::from_utf8(inspect_output.stdout).expect("stdout should be utf-8");
        let inspect_report: Value =
            serde_json::from_str(&inspect_stdout).expect("inspect stdout should parse as JSON");
        assert_eq!(inspect_report["index"]["exists"], true);

        let inspect_profile_json: Value = serde_json::from_str(
            &fs::read_to_string(&inspect_profile).expect("inspect profile should be readable"),
        )
        .expect("inspect profile should parse");
        assert_eq!(inspect_profile_json["command"], "inspect");
        assert_eq!(inspect_profile_json["success"], true);
        assert!(
            inspect_profile_json["stages"]
                .as_array()
                .is_some_and(|stages| stages
                    .iter()
                    .all(|stage| stage["elapsed_ns"].as_u64().is_some())),
            "profile stages should have elapsed time"
        );
        assert!(
            inspect_profile_json["stages"]
                .as_array()
                .is_some_and(|stages| stages.iter().all(|stage| {
                    stage["memory_start"]["current_rss_bytes"]
                        .as_u64()
                        .is_some()
                        && stage["memory_end"]["process_peak_rss_bytes"]
                            .as_u64()
                            .is_some()
                        && stage["memory_end"]["peak_rss_bytes"].as_u64().is_some()
                })),
            "profile stages should include rss snapshots"
        );
    }

    fn assert_get_profile(root: &std::path::Path, feature_id: &str) {
        let get_profile = temp_output_path("get-profile");
        let get_output = run_cli_output([
            "get",
            root.to_str().expect("root path must be utf-8"),
            "--id",
            feature_id,
            "--profile",
            get_profile.to_str().expect("profile path must be utf-8"),
            "--output",
            root.join("feature.cityjsonseq")
                .to_str()
                .expect("output path must be utf-8"),
        ]);
        assert!(
            get_output.status.success(),
            "get command should succeed: {}",
            String::from_utf8_lossy(&get_output.stderr)
        );
        let get_profile_json: Value = serde_json::from_str(
            &fs::read_to_string(&get_profile).expect("get profile should be readable"),
        )
        .expect("get profile should parse");
        assert_eq!(get_profile_json["command"], "get");
        assert_eq!(get_profile_json["success"], true);
        assert!(
            get_profile_json["stages"]
                .as_array()
                .is_some_and(|stages| stages
                    .iter()
                    .any(|stage| stage["name"] == "output serialization/write")),
            "profile should include output serialization/write"
        );
    }

    fn assert_indexing_stage_names(profile: &Value) {
        let stages = profile["stages"]
            .as_array()
            .expect("profile stages should be an array");
        assert!(
            stages
                .iter()
                .any(|stage| stage["name"] == "scan and sqlite rebuild"),
            "profile should record the real indexing stage"
        );
        for absent in [
            "source/file sharding",
            "scan/parse",
            "sqlite insert/write",
            "sidecar publish/replace",
        ] {
            assert!(
                stages.iter().all(|stage| stage["name"] != absent),
                "profile should not contain fake stage {absent}"
            );
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {}
