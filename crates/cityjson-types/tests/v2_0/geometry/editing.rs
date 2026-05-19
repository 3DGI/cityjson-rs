//! Geometry editing API tests.

use super::fixtures;
use cityjson_types::prelude::*;
use cityjson_types::v2_0::geometry::{
    build_linestring_semantic_map, build_point_semantic_map, build_surface_material_map,
    build_surface_semantic_map,
};
use cityjson_types::v2_0::{
    CityObject, CityObjectIdentifier, CityObjectType, Geometry, GeometryType,
};

#[test]
fn clone_stored_parts_round_trips_geometry() {
    let fixtures::S1Result { model, handle, .. } = fixtures::build_s1(GeometryType::MultiSurface);
    let geometry = model.get_geometry(handle).unwrap();

    let cloned = Geometry::from_stored_parts(geometry.clone_stored_parts());

    assert_eq!(cloned.clone_stored_parts(), geometry.clone_stored_parts());
}

#[test]
fn replace_geometry_preserves_handle_and_updates_cityobject_visible_geometry() {
    let fixtures::S1Result {
        mut model, handle, ..
    } = fixtures::build_s1(GeometryType::MultiSurface);
    let point_handle = cityjson_types::v2_0::GeometryDraft::multi_point(
        None,
        [cityjson_types::v2_0::PointDraft::new([10.0, 0.0, 0.0])],
    )
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
}

#[test]
fn replace_geometry_rejects_invalid_handle_and_invalid_replacement() {
    let fixtures::S1Result {
        mut model, handle, ..
    } = fixtures::build_s1(GeometryType::MultiSurface);
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
        &GeometryType::MultiSurface
    );
}

#[test]
fn material_builder_accepts_surface_geometries_and_rejects_other_families() {
    let fixtures::S1Result { model, handle, .. } = fixtures::build_s1(GeometryType::MultiSurface);
    let material = model.iter_materials().next().unwrap().0;
    let geometry = model.get_geometry(handle).unwrap();

    let map = build_surface_material_map(&model, geometry, |_| Some(material)).unwrap();
    assert_eq!(
        map.surfaces().len(),
        geometry.boundaries().unwrap().surfaces().len()
    );

    let fixtures::P1Result { model, handle } = fixtures::build_p1();
    assert!(
        build_surface_material_map(&model, model.get_geometry(handle).unwrap(), |_| None).is_err()
    );

    let fixtures::L1Result { model, handle } = fixtures::build_l1();
    assert!(
        build_surface_material_map(&model, model.get_geometry(handle).unwrap(), |_| None).is_err()
    );

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

#[test]
fn semantic_builders_accept_only_matching_primitive_families() {
    let fixtures::P1Result { model, handle } = fixtures::build_p1();
    let point_geometry = model.get_geometry(handle).unwrap();
    let semantic = model.iter_semantics().next().unwrap().0;
    assert!(build_point_semantic_map(&model, point_geometry, |_| Some(semantic)).is_ok());
    assert!(build_linestring_semantic_map(&model, point_geometry, |_| None).is_err());
    assert!(build_surface_semantic_map(&model, point_geometry, |_| None).is_err());

    let fixtures::L1Result { model, handle } = fixtures::build_l1();
    let line_geometry = model.get_geometry(handle).unwrap();
    let semantic = model.iter_semantics().next().unwrap().0;
    assert!(build_linestring_semantic_map(&model, line_geometry, |_| Some(semantic)).is_ok());
    assert!(build_point_semantic_map(&model, line_geometry, |_| None).is_err());

    let fixtures::S1Result { model, handle, .. } = fixtures::build_s1(GeometryType::MultiSurface);
    let surface_geometry = model.get_geometry(handle).unwrap();
    let semantic = model.iter_semantics().next().unwrap().0;
    assert!(build_surface_semantic_map(&model, surface_geometry, |_| Some(semantic)).is_ok());
    assert!(build_point_semantic_map(&model, surface_geometry, |_| None).is_err());
}

#[test]
fn builders_reject_missing_resource_handles() {
    let fixtures::S1Result { model, handle, .. } = fixtures::build_s1(GeometryType::MultiSurface);
    let geometry = model.get_geometry(handle).unwrap();
    let missing_material = unsafe { MaterialHandle::from_raw_parts_unchecked(99, 0) };
    let missing_semantic = unsafe { SemanticHandle::from_raw_parts_unchecked(99, 0) };

    assert!(build_surface_material_map(&model, geometry, |_| Some(missing_material)).is_err());
    assert!(build_surface_semantic_map(&model, geometry, |_| Some(missing_semantic)).is_err());
}
