//! Geometry editing API tests.

use super::fixtures;
use cityjson_types::prelude::*;
use cityjson_types::v2_0::geometry::{
    build_linestring_semantic_map, build_point_semantic_map, build_surface_material_map,
    build_surface_semantic_map,
};
use cityjson_types::v2_0::{
    CityObject, CityObjectIdentifier, CityObjectType, Geometry, GeometryDraft, GeometryType,
    PointDraft,
};

/// Inputs: an S1 stored geometry cloned through raw stored parts. Assertions:
/// rebuilding from the clone reproduces the same stored payload. Purpose:
/// protect the low-level editing escape hatch used by replacement workflows.
#[test]
fn clone_stored_parts_round_trips_geometry() {
    let fixtures::S1Result { model, handle, .. } = fixtures::build_s1(GeometryType::MultiSurface);
    let geometry = model.get_geometry(handle).unwrap();

    let cloned = Geometry::from_stored_parts(geometry.clone_stored_parts());

    assert_eq!(cloned.clone_stored_parts(), geometry.clone_stored_parts());
}

/// Inputs: an S1 geometry referenced by a city object, a valid point geometry
/// replacement, an invalid handle, and an invalid replacement payload.
/// Assertions: replacement keeps the handle visible through the city object,
/// invalid replacement paths fail, and the original slot remains valid. Purpose:
/// compact coverage for handle-stable geometry replacement validation.
#[test]
fn replace_geometry_updates_existing_slot_and_validates_replacement() {
    let fixtures::S1Result {
        mut model, handle, ..
    } = fixtures::build_s1(GeometryType::MultiSurface);
    let point_handle = GeometryDraft::multi_point(None, [PointDraft::new([10.0, 0.0, 0.0])])
        .insert_into(&mut model)
        .unwrap();

    let mut cityobject = CityObject::new(
        CityObjectIdentifier::new("building-1".to_string()),
        CityObjectType::Building,
    );
    cityobject.add_geometry(handle);
    let cityobject_handle = model.cityobjects_mut().add(cityobject).unwrap();

    let replacement = model.get_geometry(point_handle).unwrap().clone();
    let old = model.replace_geometry(handle, replacement).unwrap();
    assert_eq!(old.type_geometry(), &GeometryType::MultiSurface);

    let cityobject = model.cityobjects().get(cityobject_handle).unwrap();
    let visible_handle = cityobject.geometry().unwrap()[0];
    assert_eq!(visible_handle, handle);
    assert_eq!(
        model.get_geometry(visible_handle).unwrap().type_geometry(),
        &GeometryType::MultiPoint
    );

    let valid_replacement = model.get_geometry(handle).unwrap().clone();
    let invalid_handle = unsafe { GeometryHandle::from_raw_parts_unchecked(99, 0) };
    assert!(
        model
            .replace_geometry(invalid_handle, valid_replacement)
            .is_err()
    );

    let mut invalid_parts = model.get_geometry(handle).unwrap().clone_stored_parts();
    invalid_parts.boundaries = None;
    let invalid_replacement = Geometry::from_stored_parts(invalid_parts);
    assert!(model.replace_geometry(handle, invalid_replacement).is_err());
    assert_eq!(
        model.get_geometry(handle).unwrap().type_geometry(),
        &GeometryType::MultiPoint
    );
}

/// Inputs: map-builder calls over matching geometry families, mismatched
/// geometry families, and missing resource handles. Assertions: builders accept
/// only the matching primitive bucket and reject missing resources. Purpose:
/// construction-level coverage for semantic/material map shape rules.
#[test]
fn map_builders_accept_matching_geometry_and_existing_resources() {
    let fixtures::S1Result { model, handle, .. } = fixtures::build_s1(GeometryType::MultiSurface);
    let material = model.iter_materials().next().unwrap().0;
    let surface_geometry = model.get_geometry(handle).unwrap();
    let map = build_surface_material_map(&model, surface_geometry, |_| Some(material)).unwrap();
    assert_eq!(
        map.surfaces().len(),
        surface_geometry.boundaries().unwrap().surfaces().len()
    );

    let semantic = model.iter_semantics().next().unwrap().0;
    assert!(build_surface_semantic_map(&model, surface_geometry, |_| Some(semantic)).is_ok());
    let missing_material = unsafe { MaterialHandle::from_raw_parts_unchecked(99, 0) };
    let missing_semantic = unsafe { SemanticHandle::from_raw_parts_unchecked(99, 0) };
    assert!(
        build_surface_material_map(&model, surface_geometry, |_| Some(missing_material)).is_err()
    );
    assert!(
        build_surface_semantic_map(&model, surface_geometry, |_| Some(missing_semantic)).is_err()
    );

    let fixtures::P1Result { model, handle } = fixtures::build_p1();
    let point_geometry = model.get_geometry(handle).unwrap();
    let semantic = model.iter_semantics().next().unwrap().0;
    assert!(build_point_semantic_map(&model, point_geometry, |_| Some(semantic)).is_ok());
    assert!(build_linestring_semantic_map(&model, point_geometry, |_| None).is_err());
    assert!(build_surface_semantic_map(&model, point_geometry, |_| None).is_err());
    assert!(build_surface_material_map(&model, point_geometry, |_| None).is_err());

    let fixtures::L1Result { model, handle } = fixtures::build_l1();
    let line_geometry = model.get_geometry(handle).unwrap();
    let semantic = model.iter_semantics().next().unwrap().0;
    assert!(build_linestring_semantic_map(&model, line_geometry, |_| Some(semantic)).is_ok());
    assert!(build_point_semantic_map(&model, line_geometry, |_| None).is_err());
    assert!(build_surface_material_map(&model, line_geometry, |_| None).is_err());

    let fixtures::I1Result {
        model,
        instance_handle,
        ..
    } = fixtures::build_i1();
    assert!(
        build_surface_material_map(&model, model.get_geometry(instance_handle).unwrap(), |_| {
            None
        })
        .is_err()
    );
}
