//! Public API contract for the explicit `cityjson_lib::json` boundary layer.

use std::io::Cursor;

use cityjson_lib::cityjson_types::v2_0::{BBox, Transform};
use serde_json::value::RawValue;

use cityjson_lib::{CityJSONVersion, json};

fn assert_close_array(actual: [f64; 3], expected: [f64; 3]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, found {actual}"
        );
    }
}

#[test]
fn explicit_json_module_supports_document_and_stream_loading() -> cityjson_lib::Result<()> {
    let document = br#"{"type":"CityJSON","version":"2.0","transform":{"scale":[1.0,1.0,1.0],"translate":[0.0,0.0,0.0]},"CityObjects":{},"vertices":[]}"#;
    let stream = br#"{"type":"CityJSON","version":"2.0","transform":{"scale":[1.0,1.0,1.0],"translate":[0.0,0.0,0.0]},"CityObjects":{},"vertices":[]}
{"type":"CityJSONFeature","id":"feature-1","CityObjects":{"feature-1":{"type":"Building"}},"vertices":[]}
{"type":"CityJSONFeature","id":"feature-2","CityObjects":{"feature-2":{"type":"Building"}},"vertices":[]}
"#;
    let feature = br#"{"type":"CityJSONFeature","id":"feature-1","CityObjects":{"feature-1":{"type":"Building"}},"vertices":[]}"#;

    let probe = json::probe(document)?;
    assert_eq!(probe.kind(), json::RootKind::CityJSON);
    assert_eq!(probe.version(), Some(CityJSONVersion::V2_0));

    let _ = json::from_slice(document)?;
    let _ = json::from_feature_slice(feature)?;
    let _ = json::from_file("tests/data/v2_0/minimal.city.json")?;
    let _ = json::from_feature_file("tests/data/v2_0/minimal.city.jsonl")?;
    let models = json::read_feature_stream(Cursor::new(stream))?
        .collect::<cityjson_lib::Result<Vec<_>>>()?;
    assert_eq!(models.len(), 2);

    let mut writer = Vec::new();
    json::write_feature_stream(&mut writer, models.clone())?;

    let output = String::from_utf8(writer).expect("feature stream output is valid UTF-8");
    let expected = models
        .iter()
        .map(json::to_feature_string)
        .collect::<cityjson_lib::Result<Vec<_>>>()?
        .join("\n")
        + "\n";
    assert_eq!(output, expected);

    Ok(())
}

#[test]
fn explicit_json_module_rejects_malformed_feature_packages() {
    let base = r#"{"type":"CityJSON","version":"2.0","transform":{"scale":[1.0,1.0,1.0],"translate":[0.0,0.0,0.0]},"CityObjects":{},"vertices":[]}"#;
    let feature = r#"{"type":"CityJSONFeature","CityObjects":{},"vertices":[]}"#;

    assert!(
        json::staged::from_feature_slice_with_base(feature.as_bytes(), base.as_bytes()).is_err()
    );
}

#[test]
fn staged_direct_feature_slice_adds_missing_root_cityobject() -> cityjson_lib::Result<()> {
    let base = br#"{
        "type":"CityJSON",
        "version":"2.0",
        "metadata":{"title":"base-root"},
        "CityObjects":{},
        "vertices":[]
    }"#;
    let feature = br#"{
        "type":"CityJSONFeature",
        "id":"feature-root",
        "CityObjects":{
            "child-a":{"type":"BuildingPart"},
            "child-b":{"type":"BuildingPart"}
        },
        "vertices":[]
    }"#;

    let model = json::staged::from_feature_slice_with_base_direct(feature, base)?;
    let output: serde_json::Value = serde_json::from_str(&json::to_feature_string(&model)?)
        .expect("direct feature output should parse");

    assert_eq!(output["id"], "feature-root");
    assert_eq!(output["metadata"]["title"], "base-root");
    assert_eq!(
        output["CityObjects"]["feature-root"]["children"],
        serde_json::json!(["child-a", "child-b"]),
    );

    Ok(())
}

#[test]
fn explicit_json_module_can_write_strict_cityjsonseq_with_explicit_transform()
-> cityjson_lib::Result<()> {
    let base_bytes = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "transform":{"scale":[1.0,1.0,1.0],"translate":[0.0,0.0,0.0]},
            "metadata":{"title":"base-root"},
            "CityObjects":{},
            "vertices":[]
        }"#;
    let base_root = json::from_slice(base_bytes)?;
    let feature = json::staged::from_feature_slice_with_base(
        br#"{
            "type":"CityJSONFeature",
            "id":"feature-1",
            "CityObjects":{
                "feature-1":{
                    "type":"Building",
                    "geometry":[{"type":"MultiPoint","boundaries":[0,1]}]
                }
            },
            "vertices":[[10,20,30],[12,22,31]]
        }"#,
        base_bytes,
    )?;

    let mut transform = Transform::new();
    transform.set_scale([0.5, 0.5, 1.0]);
    transform.set_translate([10.0, 20.0, 30.0]);

    let mut output = Vec::new();
    let report = json::write_cityjsonseq_refs(&mut output, &base_root, [&feature], &transform)?;

    assert_eq!(report.feature_count, 1);
    assert_eq!(report.cityobject_count, 1);
    assert_eq!(
        report.geographical_extent,
        Some(BBox::new(10.0, 20.0, 30.0, 12.0, 22.0, 31.0))
    );

    let items = serde_json::Deserializer::from_slice(&output)
        .into_iter::<serde_json::Value>()
        .collect::<serde_json::Result<Vec<_>>>()
        .expect("strict CityJSONSeq output should parse");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["type"], "CityJSON");
    assert_eq!(items[0]["metadata"]["title"], "base-root");
    assert_eq!(
        items[0]["metadata"]["geographicalExtent"],
        serde_json::json!([10.0, 20.0, 30.0, 12.0, 22.0, 31.0])
    );
    assert_eq!(items[1]["type"], "CityJSONFeature");
    assert!(items[1].get("transform").is_none());
    assert_eq!(
        items[1]["vertices"],
        serde_json::json!([[0, 0, 0], [4, 4, 1]])
    );

    let roundtrip =
        json::read_cityjsonseq(Cursor::new(output))?.collect::<cityjson_lib::Result<Vec<_>>>()?;
    assert_eq!(roundtrip.len(), 1);
    assert_eq!(
        roundtrip[0]
            .metadata()
            .and_then(|metadata| metadata.title()),
        Some("base-root")
    );

    Ok(())
}

#[test]
fn explicit_json_module_can_write_strict_cityjsonseq_with_auto_transform()
-> cityjson_lib::Result<()> {
    let base_bytes = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "transform":{"scale":[1.0,1.0,1.0],"translate":[0.0,0.0,0.0]},
            "metadata":{"title":"base-root"},
            "CityObjects":{},
            "vertices":[]
        }"#;
    let base_root = json::from_slice(base_bytes)?;
    let feature_a = json::staged::from_feature_slice_with_base(
        br#"{
            "type":"CityJSONFeature",
            "id":"feature-a",
            "CityObjects":{
                "feature-a":{
                    "type":"Building",
                    "geometry":[{"type":"MultiPoint","boundaries":[0,1]}]
                }
            },
            "vertices":[[10,20,30],[12,23,35]]
        }"#,
        base_bytes,
    )?;
    let feature_b = json::staged::from_feature_slice_with_base(
        br#"{
            "type":"CityJSONFeature",
            "id":"feature-b",
            "CityObjects":{
                "feature-b":{
                    "type":"Building",
                    "geometry":[{"type":"MultiPoint","boundaries":[0]}]
                }
            },
            "vertices":[[9,21,40]]
        }"#,
        base_bytes,
    )?;

    let mut output = Vec::new();
    let report = json::write_cityjsonseq_auto_transform_refs(
        &mut output,
        &base_root,
        [&feature_a, &feature_b],
        [0.5, 1.0, 5.0],
    )?;

    assert_eq!(
        report.geographical_extent,
        Some(BBox::new(9.0, 20.0, 30.0, 12.0, 23.0, 40.0))
    );
    assert_close_array(report.transform.scale(), [0.5, 1.0, 5.0]);
    assert_close_array(report.transform.translate(), [9.0, 20.0, 30.0]);

    let items = serde_json::Deserializer::from_slice(&output)
        .into_iter::<serde_json::Value>()
        .collect::<serde_json::Result<Vec<_>>>()
        .expect("strict CityJSONSeq output should parse");
    assert_eq!(
        items[0]["transform"]["translate"],
        serde_json::json!([9.0, 20.0, 30.0])
    );

    Ok(())
}

#[test]
fn explicit_json_module_uses_base_root_for_mixed_source_cityjsonseq() -> cityjson_lib::Result<()> {
    let output_base_bytes = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "transform":{"scale":[1.0,1.0,1.0],"translate":[0.0,0.0,0.0]},
            "metadata":{"title":"tile-debug-stream"},
            "CityObjects":{},
            "vertices":[]
        }"#;
    let feature_a_origin_base = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "transform":{"scale":[0.001,0.001,0.001],"translate":[113994.269,471970.12,-5.829]},
            "metadata":{"identifier":"0","referenceSystem":"EPSG:7415"},
            "CityObjects":{},
            "vertices":[]
        }"#;
    let feature_b_shifted_base = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "transform":{"scale":[0.001,0.001,0.001],"translate":[113830.949,473978.031,-5.825]},
            "metadata":{"identifier":"13","referenceSystem":"EPSG:7415"},
            "CityObjects":{},
            "vertices":[]
        }"#;
    let output_base = json::from_slice(output_base_bytes)?;
    let feature_a = json::staged::from_feature_slice_with_base(
        br#"{
            "type":"CityJSONFeature",
            "id":"feature-a",
            "CityObjects":{"feature-a":{"type":"Building","geometry":[{"type":"MultiPoint","boundaries":[0]}]}},
            "vertices":[[0,0,0]]
        }"#,
        feature_a_origin_base,
    )?;
    let feature_b = json::staged::from_feature_slice_with_base(
        br#"{
            "type":"CityJSONFeature",
            "id":"feature-b",
            "CityObjects":{"feature-b":{"type":"Building","geometry":[{"type":"MultiPoint","boundaries":[0]}]}},
            "vertices":[[0,0,0]]
        }"#,
        feature_b_shifted_base,
    )?;

    let mut output = Vec::new();
    let report = json::write_cityjsonseq_auto_transform_refs(
        &mut output,
        &output_base,
        [&feature_a, &feature_b],
        [0.001, 0.001, 0.001],
    )?;
    let items = serde_json::Deserializer::from_slice(&output)
        .into_iter::<serde_json::Value>()
        .collect::<serde_json::Result<Vec<_>>>()
        .expect("strict CityJSONSeq output should parse");

    assert_eq!(report.feature_count, 2);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["metadata"]["title"], "tile-debug-stream");
    assert!(items[0]["metadata"].get("identifier").is_none());
    assert_eq!(items[1]["id"], "feature-a");
    assert_eq!(items[2]["id"], "feature-b");

    Ok(())
}

#[test]
fn explicit_json_module_can_materialize_standalone_features_with_a_base_document()
-> cityjson_lib::Result<()> {
    let document = br#"{
        "type":"CityJSON",
        "version":"2.0",
        "transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},
        "CityObjects":{},
        "vertices":[]
    }"#;
    let feature = br#"{
        "type":"CityJSONFeature",
        "id":"feature-1",
        "CityObjects":{"feature-1":{"type":"Building","geometry":[{"type":"MultiSurface","boundaries":[[[0,1,2]]]}]}},
        "vertices":[[0,0,0],[2,0,0],[2,4,5]]
    }"#;

    let model = json::staged::from_feature_slice_with_base(feature, document)?;
    let vertices = model.vertices();
    let v0 = vertices.as_slice()[0].to_array();
    let v2 = vertices.as_slice()[2].to_array();

    assert_close_array(v0, [10.0, 20.0, 30.0]);
    assert_close_array(v2, [11.0, 22.0, 35.0]);

    Ok(())
}

#[test]
fn explicit_json_module_can_materialize_feature_parts_with_a_base_document()
-> cityjson_lib::Result<()> {
    let document = br#"{
        "type":"CityJSON",
        "version":"2.0",
        "transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},
        "metadata":{"title":"base-root"},
        "CityObjects":{},
        "vertices":[]
    }"#;
    let object = RawValue::from_string(
        r#"{"type":"Building","geometry":[{"type":"MultiSurface","boundaries":[[[0,2,1]]]}]}"#
            .to_owned(),
    )
    .expect("raw feature object");
    let cityobjects = [json::staged::FeatureObjectFragment {
        id: "feature-1",
        object: object.as_ref(),
    }];
    let vertices = [[0, 0, 0], [2, 0, 0], [1, 0, 0]];
    let parts = json::staged::FeatureAssembly {
        id: "feature-1",
        cityobjects: &cityobjects,
        vertices: &vertices,
    };

    let model = json::staged::from_feature_assembly_with_base(parts, document)?;
    let vertices = model.vertices();
    let text = json::to_string(&model)?;
    let output: serde_json::Value =
        serde_json::from_str(&text).expect("feature serialization should stay valid JSON");

    assert_eq!(output["metadata"]["title"], "base-root");
    assert_eq!(
        output["vertices"],
        serde_json::json!([[0, 0, 0], [2, 0, 0], [1, 0, 0]])
    );
    assert_close_array(vertices.as_slice()[0].to_array(), [10.0, 20.0, 30.0]);
    assert_close_array(vertices.as_slice()[2].to_array(), [10.5, 20.0, 30.0]);

    Ok(())
}

#[test]
fn citymodel_constructors_are_aliases_for_the_default_json_path() {
    let citymodel_from_slice: fn(&[u8]) -> cityjson_lib::Result<cityjson_lib::CityModel> =
        cityjson_lib::json::from_slice;
    let json_from_slice: fn(&[u8]) -> cityjson_lib::Result<cityjson_lib::CityModel> =
        json::from_slice;

    let _ = citymodel_from_slice;
    let _ = json_from_slice;
}

#[test]
fn explicit_json_module_owns_serialization() -> cityjson_lib::Result<()> {
    let model = json::from_file("tests/data/v2_0/minimal.city.json")?;

    let bytes = json::to_vec(&model)?;
    let text = json::to_string(&model)?;

    let mut writer = Vec::new();
    json::to_writer(&mut writer, &model)?;

    assert!(!bytes.is_empty());
    assert!(!text.is_empty());
    assert!(!writer.is_empty());

    Ok(())
}

#[test]
fn explicit_json_module_can_write_features_without_building_a_string() -> cityjson_lib::Result<()> {
    let feature = br#"{"type":"CityJSONFeature","id":"feature-1","CityObjects":{"feature-1":{"type":"Building"}},"vertices":[]}"#;
    let model = json::from_feature_slice(feature)?;

    let mut writer = Vec::new();
    json::to_feature_writer(&mut writer, &model)?;

    assert_eq!(
        String::from_utf8(writer).expect("feature writer output is valid UTF-8"),
        json::to_feature_string(&model)?,
    );

    Ok(())
}

#[test]
fn document_loading_rejects_jsonl_streams() {
    let error = json::from_file("tests/data/v1_1/fake.city.jsonl").unwrap_err();
    assert_eq!(error.kind(), cityjson_lib::ErrorKind::Unsupported);
}
