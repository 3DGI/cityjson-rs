use std::io::Cursor;

use serde_json::{Value, json};

use cityjson_json::{
    CityJsonSeqWriteOptions, FeatureStreamTransform, ReadOptions, append, read_feature,
    read_feature_stream, read_feature_with_base, read_model, write_feature_stream,
    write_feature_stream_with_base,
};
use cityjson_types::v2_0::{
    AffineTransform3D, BBox, CityModelType, CityObject, CityObjectIdentifier, CityObjectType,
    GeometryDraft, LoD, OwnedCityModel, PointDraft, RealWorldCoordinate, Texture, Transform,
    UVCoordinate,
};
use cityjson_types::v2_0::{ImageType, TextureType};
use common::*;

mod common;

macro_rules! conformance_roundtrip_tests {
    ($assert_fn:ident; $($case_id:ident),+ $(,)?) => {
        $(
            #[test]
            fn $case_id() {
                let json_input = conformance_case_input(stringify!($case_id));
                $assert_fn(&json_input);
            }
        )+
    };
}

fn read_feature_with_base_str(
    feature_input: &str,
    base: &OwnedCityModel,
) -> cityjson_json::Result<OwnedCityModel> {
    read_feature_with_base(feature_input.as_bytes(), base, &ReadOptions::default())
}

fn stream_items(bytes: &[u8]) -> Vec<Value> {
    serde_json::Deserializer::from_slice(bytes)
        .into_iter::<Value>()
        .collect::<serde_json::Result<Vec<_>>>()
        .unwrap()
}

fn assert_vertex_eq(actual: [f64; 3], expected: [f64; 3]) {
    for (actual_coord, expected_coord) in actual.into_iter().zip(expected) {
        assert!((actual_coord - expected_coord).abs() < f64::EPSILON);
    }
}

#[test]
fn read_feature_with_base_materializes_a_self_contained_feature_model() {
    let base = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [0.5, 0.5, 1.0],
                "translate": [10.0, 20.0, 30.0]
            },
            "metadata": {
                "title": "base-root"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let feature_input = json!({
        "type": "CityJSONFeature",
        "id": "feature-1",
        "CityObjects": {
            "feature-1": {
                "type": "Building",
                "geometry": [{
                    "type": "MultiSurface",
                    "lod": "0",
                    "boundaries": [[[0, 2, 1]]]
                }]
            }
        },
        "vertices": [[0, 0, 0], [2, 0, 0], [1, 0, 0]]
    })
    .to_string();

    let model = read_feature_with_base_str(&feature_input, &base).unwrap();
    let vertices = model.vertices();
    let written = write_value(&model);

    assert_eq!(written["metadata"]["title"], "base-root");
    assert_eq!(
        written["transform"],
        json!({
            "scale": [0.5, 0.5, 1.0],
            "translate": [10.0, 20.0, 30.0]
        })
    );
    assert_eq!(
        written["vertices"],
        json!([[0, 0, 0], [2, 0, 0], [1, 0, 0]])
    );
    assert_eq!(
        written["CityObjects"]["feature-1"]["geometry"][0]["boundaries"],
        json!([[[0, 2, 1]]])
    );
    assert_vertex_eq(vertices.as_slice()[0].to_array(), [10.0, 20.0, 30.0]);
    assert_vertex_eq(vertices.as_slice()[2].to_array(), [10.5, 20.0, 30.0]);
}

#[test]
fn read_model_rejects_cityjsonfeature_roots() {
    let input = json!({
        "type": "CityJSONFeature",
        "id": "f1",
        "CityObjects": {
            "f1": { "type": "Building" }
        },
        "vertices": []
    })
    .to_string();

    let error = read_model(input.as_bytes(), &ReadOptions::default()).unwrap_err();
    assert!(error.to_string().contains("CityJSONFeature"));
}

#[test]
fn read_feature_rejects_cityjson_roots() {
    let input = json!({
        "type": "CityJSON",
        "version": "2.0",
        "transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},
        "CityObjects": {},
        "vertices": []
    })
    .to_string();

    let error = read_feature(input.as_bytes(), &ReadOptions::default()).unwrap_err();
    assert!(error.to_string().contains("CityJSON"));
}

#[test]
fn serialize_quantizes_root_vertices_only() {
    let mut model = OwnedCityModel::new(CityModelType::CityJSON);
    model.transform_mut();
    model
        .metadata_mut()
        .set_geographical_extent(BBox::new(1.1, 2.2, 3.3, 4.4, 5.5, 6.6));
    model
        .add_vertex(RealWorldCoordinate::new(1.25, 2.5, 3.75))
        .unwrap();
    model
        .add_template_vertex(RealWorldCoordinate::new(4.125, 5.25, 6.875))
        .unwrap();
    model
        .add_uv_coordinate(UVCoordinate::new(0.125, 0.875))
        .unwrap();
    let mut texture = Texture::new("texture.png".to_string(), ImageType::Png);
    texture.set_texture_type(Some(TextureType::Specific));
    model.add_texture(texture).unwrap();

    let written = write_value(&model);

    let root_vertices = written["vertices"].as_array().unwrap();
    assert_eq!(root_vertices.len(), 1);
    assert!(
        root_vertices[0]
            .as_array()
            .unwrap()
            .iter()
            .all(|coordinate| coordinate.is_i64() || coordinate.is_u64())
    );

    let template_vertices = written["geometry-templates"]["vertices-templates"]
        .as_array()
        .unwrap();
    assert_eq!(template_vertices.len(), 1);
    assert!(
        template_vertices[0]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_f64)
    );

    let texture_vertices = written["appearance"]["vertices-texture"]
        .as_array()
        .unwrap();
    assert_eq!(texture_vertices.len(), 1);
    assert!(
        texture_vertices[0]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_f64)
    );

    let extent = written["metadata"]["geographicalExtent"]
        .as_array()
        .unwrap();
    assert_eq!(extent.len(), 6);
    assert!(extent.iter().all(Value::is_f64));
}

#[test]
fn serialize_omits_empty_appearance_and_geometry_templates_sections() {
    let model = OwnedCityModel::new(CityModelType::CityJSON);
    let written = write_value(&model);

    assert!(written.get("appearance").is_none());
    assert!(written.get("geometry-templates").is_none());
}

#[test]
fn serialize_geometry_instance_keeps_float_sections() {
    let mut model = OwnedCityModel::new(CityModelType::CityJSON);

    let template = GeometryDraft::multi_point(
        Some(LoD::LoD1),
        [PointDraft::new(RealWorldCoordinate::new(0.25, 0.5, 0.75))],
    )
    .insert_template_into(&mut model)
    .unwrap();

    let geometry = GeometryDraft::instance(
        template,
        RealWorldCoordinate::new(1.25, 2.5, 3.75),
        AffineTransform3D::default(),
    )
    .insert_into(&mut model)
    .unwrap();

    let mut cityobject = CityObject::new(
        CityObjectIdentifier::new("instance-1".to_string()),
        CityObjectType::Building,
    );
    cityobject.add_geometry(geometry);
    model.cityobjects_mut().add(cityobject).unwrap();

    let written = write_value(&model);
    let root_vertices = written["vertices"].as_array().unwrap();
    assert_eq!(root_vertices.len(), 1);
    assert!(
        root_vertices[0]
            .as_array()
            .unwrap()
            .iter()
            .all(|coordinate| coordinate.is_i64() || coordinate.is_u64())
    );

    let geometry = &written["CityObjects"]["instance-1"]["geometry"][0];
    let boundaries = geometry["boundaries"].as_array().unwrap();
    assert!(boundaries[0].as_f64().is_some());
    assert!(
        geometry["transformationMatrix"]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_f64)
    );
}

#[test]
fn read_feature_stream_materializes_models_from_header_and_features() {
    let input = concat!(
        r#"{"type":"CityJSON","version":"2.0","transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},"metadata":{"title":"base-root"},"CityObjects":{},"vertices":[]}"#,
        "\n",
        r#"{"type":"CityJSONFeature","id":"building-1","CityObjects":{"building-1":{"type":"Building","geometry":[{"type":"MultiSurface","lod":"0","boundaries":[[[0,1,2]]]}]}},"vertices":[[0,0,0],[2,0,0],[1,0,0]]}"#,
        "\n"
    );

    let mut reader =
        read_feature_stream(Cursor::new(input.as_bytes()), &ReadOptions::default()).unwrap();
    let model = reader.next().unwrap().unwrap();
    let written = write_value(&model);

    assert_eq!(written["metadata"]["title"], "base-root");
    assert_eq!(written["id"], "building-1");
    assert_eq!(
        written["vertices"],
        json!([[0, 0, 0], [2, 0, 0], [1, 0, 0]])
    );
    assert!(reader.next().is_none());
}

#[test]
fn read_feature_stream_rejects_duplicate_cityobject_ids() {
    let input = concat!(
        r#"{"type":"CityJSON","version":"2.0","transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},"CityObjects":{},"vertices":[]}"#,
        "\n",
        r#"{"type":"CityJSONFeature","id":"building-1","CityObjects":{"building-1":{"type":"Building"}},"vertices":[]}"#,
        "\n",
        r#"{"type":"CityJSONFeature","id":"building-2","CityObjects":{"building-1":{"type":"Building"}},"vertices":[]}"#,
        "\n"
    );

    let mut reader =
        read_feature_stream(Cursor::new(input.as_bytes()), &ReadOptions::default()).unwrap();
    assert!(reader.next().unwrap().is_ok());
    let error = reader.next().unwrap().unwrap_err();
    assert!(error.to_string().contains("duplicate CityObject id"));
}

#[test]
fn append_accepts_mismatched_transforms_and_clears_the_result() {
    let mut left = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [1.0, 1.0, 1.0],
                "translate": [0.0, 0.0, 0.0]
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let right = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [2.0, 2.0, 2.0],
                "translate": [10.0, 0.0, 0.0]
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );

    append(&mut left, &right).expect("append should accept mismatched transforms");

    assert!(left.transform().is_none());
    assert!(write_value(&left).get("transform").is_none());
}

#[test]
fn write_feature_stream_writes_header_and_features() {
    let base = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},
            "metadata": {
                "title": "base-root"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let feature = read_feature_with_base_str(
        &json!({
            "type": "CityJSONFeature",
            "id": "building-1",
            "CityObjects": {
                "building-1": {
                    "type": "Building",
                    "geometry": [{
                        "type": "MultiPoint",
                        "boundaries": [0]
                    }]
                }
            },
            "vertices": [[10, 20, 30]]
        })
        .to_string(),
        &base,
    )
    .unwrap();

    let mut output = Vec::new();
    let report =
        write_feature_stream(&mut output, [feature], &CityJsonSeqWriteOptions::default()).unwrap();
    let items = stream_items(&output);

    assert_eq!(report.feature_count, 1);
    assert_eq!(items[0]["type"], "CityJSON");
    assert_eq!(items[0]["metadata"]["title"], "base-root");
    assert_eq!(items[1]["type"], "CityJSONFeature");
    assert_eq!(items[1]["id"], "building-1");
}

#[test]
fn write_feature_stream_rejects_incompatible_root_state() {
    let base_a = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},
            "metadata": {
                "title": "base-a"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let base_b = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},
            "metadata": {
                "title": "base-b"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let feature_a = read_feature_with_base_str(
        &json!({
            "type": "CityJSONFeature",
            "id": "building-1",
            "CityObjects": {
                "building-1": {
                    "type": "Building"
                }
            },
            "vertices": []
        })
        .to_string(),
        &base_a,
    )
    .unwrap();
    let feature_b = read_feature_with_base_str(
        &json!({
            "type": "CityJSONFeature",
            "id": "building-2",
            "CityObjects": {
                "building-2": {
                    "type": "Building"
                }
            },
            "vertices": []
        })
        .to_string(),
        &base_b,
    )
    .unwrap();

    let error = write_feature_stream(
        Vec::new(),
        [feature_a, feature_b],
        &CityJsonSeqWriteOptions {
            transform: FeatureStreamTransform::Explicit(Transform::new()),
            ..CityJsonSeqWriteOptions::default()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("incompatible root state"));
}

#[test]
fn write_feature_stream_with_base_accepts_incompatible_feature_metadata() {
    let output_base = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform":{"scale":[1.0,1.0,1.0],"translate":[0.0,0.0,0.0]},
            "metadata": {
                "title": "tile-debug-stream"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let base_a = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform":{"scale":[0.001,0.001,0.001],"translate":[113_994.269,471_970.12,-5.829]},
            "metadata": {
                "identifier": "0",
                "referenceSystem": "EPSG:7415"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let base_b = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform":{"scale":[0.001,0.001,0.001],"translate":[113_830.949,473_978.031,-5.825]},
            "metadata": {
                "identifier": "13",
                "referenceSystem": "EPSG:7415"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let feature_a = read_feature_with_base_str(
        &json!({
            "type": "CityJSONFeature",
            "id": "building-1",
            "CityObjects": {
                "building-1": {
                    "type": "Building"
                }
            },
            "vertices": []
        })
        .to_string(),
        &base_a,
    )
    .unwrap();
    let feature_b = read_feature_with_base_str(
        &json!({
            "type": "CityJSONFeature",
            "id": "building-2",
            "CityObjects": {
                "building-2": {
                    "type": "Building"
                }
            },
            "vertices": []
        })
        .to_string(),
        &base_b,
    )
    .unwrap();

    let mut output = Vec::new();
    let report = write_feature_stream_with_base(
        &mut output,
        &output_base,
        [feature_a, feature_b],
        &CityJsonSeqWriteOptions {
            transform: FeatureStreamTransform::Explicit(Transform::new()),
            ..CityJsonSeqWriteOptions::default()
        },
    )
    .unwrap();
    let items = stream_items(&output);

    assert_eq!(report.feature_count, 2);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["metadata"]["title"], "tile-debug-stream");
    assert!(items[0]["metadata"].get("identifier").is_none());
    assert_eq!(items[1]["id"], "building-1");
    assert_eq!(items[2]["id"], "building-2");
}

#[test]
fn write_feature_stream_preserves_feature_local_ids() {
    let base = read_model_str(
        &json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform":{"scale":[0.5,0.5,1.0],"translate":[10.0,20.0,30.0]},
            "metadata": {
                "title": "base-root"
            },
            "CityObjects": {},
            "vertices": []
        })
        .to_string(),
    );
    let feature_a = read_feature_with_base_str(
        &json!({
            "type": "CityJSONFeature",
            "id": "building-1",
            "CityObjects": {
                "building-1": {
                    "type": "Building"
                }
            },
            "vertices": []
        })
        .to_string(),
        &base,
    )
    .unwrap();
    let feature_b = read_feature_with_base_str(
        &json!({
            "type": "CityJSONFeature",
            "id": "building-2",
            "CityObjects": {
                "building-2": {
                    "type": "Building"
                }
            },
            "vertices": []
        })
        .to_string(),
        &base,
    )
    .unwrap();

    let mut output = Vec::new();
    write_feature_stream(
        &mut output,
        [feature_a, feature_b],
        &CityJsonSeqWriteOptions {
            transform: FeatureStreamTransform::Explicit(Transform::new()),
            ..CityJsonSeqWriteOptions::default()
        },
    )
    .unwrap();

    let items = stream_items(&output);
    assert_eq!(items[1]["id"], "building-1");
    assert_eq!(items[2]["id"], "building-2");
}

conformance_roundtrip_tests!(
    assert_eq_roundtrip;
    appearance_complete,
    cityobject_building_address,
    cityobject_complete,
    cityobject_extended,
    cityobject_all_types,
    coordinates_precision_ecef,
    coordinates_precision_local,
    coordinates_precision_stateplane,
    coordinates_precision_utm,
    coordinates_precision_wgs84,
    coordinates_precision_worst,
    geometry_instance,
    geometry_material_solid,
    geometry_material_multisolid,
    geometry_material_multisurface,
    geometry_texture_solid,
    geometry_texture_multisolid,
    geometry_texture_multisurface,
    geometry_semantics_solid,
    geometry_semantics_multisolid,
    geometry_semantics_multisurface,
    geometry_semantics_multilinestring,
    geometry_semantics_multipoint,
    cityjson_extended,
    cityjsonfeature_minimal,
    cityjson_fake_complete,
    cityjson_minimal,
    metadata_complete,
    metadata_extra_properties,
    semantic_all_types,
    semantic_complete,
    semantic_extended,
    vertices,
    extension,
);
