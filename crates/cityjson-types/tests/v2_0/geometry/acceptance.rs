//! Integrated acceptance tests for the canonical geometry fixtures.

use super::fixtures::*;
use cityjson_types::v2_0::geometry::SemanticMapView;
use cityjson_types::v2_0::*;

fn assert_boundary_shape(
    boundary: &Boundary<u32>,
    expected_type: BoundaryType,
    vertices: usize,
    rings: usize,
    surfaces: usize,
    shells: usize,
    solids: usize,
) {
    assert!(boundary.is_consistent());
    assert_eq!(boundary.check_type(), expected_type);
    assert_eq!(boundary.vertices().len(), vertices);
    assert_eq!(boundary.rings().len(), rings);
    assert_eq!(boundary.surfaces().len(), surfaces);
    assert_eq!(boundary.shells().len(), shells);
    assert_eq!(boundary.solids().len(), solids);
}

/// Inputs: P1, L1, S1, D1, MS1, T1, and I1 canonical fixtures.
/// Assertions: stored geometry kind, boundary hierarchy, and instance payload
/// match the fixture contract. Purpose: one high-signal smoke test for valid
/// canonical geometry acceptance.
#[test]
fn canonical_fixture_acceptance() {
    let P1Result { model, handle } = build_p1();
    let geom = model.get_geometry(handle).unwrap();
    assert_eq!(geom.type_geometry(), &GeometryType::MultiPoint);
    assert_boundary_shape(
        geom.boundaries().unwrap(),
        BoundaryType::MultiPoint,
        3,
        0,
        0,
        0,
        0,
    );

    let L1Result { model, handle } = build_l1();
    let geom = model.get_geometry(handle).unwrap();
    assert_eq!(geom.type_geometry(), &GeometryType::MultiLineString);
    assert_boundary_shape(
        geom.boundaries().unwrap(),
        BoundaryType::MultiLineString,
        5,
        2,
        0,
        0,
        0,
    );

    for geometry_type in [GeometryType::MultiSurface, GeometryType::CompositeSurface] {
        let S1Result { model, handle, .. } = build_s1(geometry_type);
        let geom = model.get_geometry(handle).unwrap();
        assert_eq!(geom.type_geometry(), &geometry_type);
        assert_boundary_shape(
            geom.boundaries().unwrap(),
            BoundaryType::MultiOrCompositeSurface,
            10,
            3,
            2,
            0,
            0,
        );
    }

    let D1Result { model, handle } = build_d1();
    let geom = model.get_geometry(handle).unwrap();
    assert_eq!(geom.type_geometry(), &GeometryType::Solid);
    assert_boundary_shape(
        geom.boundaries().unwrap(),
        BoundaryType::Solid,
        12,
        4,
        4,
        2,
        0,
    );

    for geometry_type in [GeometryType::MultiSolid, GeometryType::CompositeSolid] {
        let MS1Result { model, handle } = build_ms1(geometry_type);
        let geom = model.get_geometry(handle).unwrap();
        assert_eq!(geom.type_geometry(), &geometry_type);
        assert_boundary_shape(
            geom.boundaries().unwrap(),
            BoundaryType::MultiOrCompositeSolid,
            12,
            4,
            4,
            2,
            2,
        );
    }

    let T1Result {
        model,
        template_handle,
    } = build_t1();
    let geom = model.get_geometry_template(template_handle).unwrap();
    assert_eq!(geom.type_geometry(), &GeometryType::MultiSurface);
    assert_boundary_shape(
        geom.boundaries().unwrap(),
        BoundaryType::MultiOrCompositeSurface,
        6,
        2,
        2,
        0,
        0,
    );

    let I1Result {
        model,
        template_handle,
        instance_handle,
    } = build_i1();
    let geom = model.get_geometry(instance_handle).unwrap();
    assert_eq!(geom.type_geometry(), &GeometryType::GeometryInstance);
    assert!(geom.boundaries().is_none());
    assert_eq!(geom.instance().unwrap().template(), template_handle);
}

fn assert_only_points_bucket(semantics: SemanticMapView<'_, u32>, expected_len: usize) {
    assert_eq!(semantics.points().len(), expected_len);
    assert!(semantics.linestrings().is_empty());
    assert!(semantics.surfaces().is_empty());
}

fn assert_only_linestrings_bucket(semantics: SemanticMapView<'_, u32>, expected_len: usize) {
    assert!(semantics.points().is_empty());
    assert_eq!(semantics.linestrings().len(), expected_len);
    assert!(semantics.surfaces().is_empty());
}

fn assert_only_surfaces_bucket(semantics: SemanticMapView<'_, u32>, expected_len: usize) {
    assert!(semantics.points().is_empty());
    assert!(semantics.linestrings().is_empty());
    assert_eq!(semantics.surfaces().len(), expected_len);
}

/// Inputs: canonical fixtures with semantic/material payloads. Assertions:
/// maps populate the single bucket that matches the geometry primitive family,
/// are dense, and preserve null placeholders. Purpose: acceptance coverage for
/// dense semantic and material maps without per-fixture duplication.
#[test]
fn dense_semantic_and_material_maps_are_accepted() {
    let P1Result { model, handle } = build_p1();
    let semantics = model.get_geometry(handle).unwrap().semantics().unwrap();
    assert_only_points_bucket(semantics, 3);
    assert!(semantics.points()[0].is_some());
    assert!(semantics.points()[1].is_none());
    assert!(semantics.points()[2].is_some());

    let L1Result { model, handle } = build_l1();
    let semantics = model.get_geometry(handle).unwrap().semantics().unwrap();
    assert_only_linestrings_bucket(semantics, 2);
    assert!(semantics.linestrings()[0].is_none());
    assert!(semantics.linestrings()[1].is_some());

    let S1Result { model, handle, .. } = build_s1(GeometryType::MultiSurface);
    let geom = model.get_geometry(handle).unwrap();
    assert_only_surfaces_bucket(geom.semantics().unwrap(), 2);
    let materials = geom.materials().unwrap();
    let (_, material_map) = materials.first().unwrap();
    assert_eq!(material_map.surfaces().len(), 2);
    assert!(material_map.surfaces()[0].is_some());
    assert!(material_map.surfaces()[1].is_none());

    let D1Result { model, handle } = build_d1();
    assert_only_surfaces_bucket(model.get_geometry(handle).unwrap().semantics().unwrap(), 4);

    let MS1Result { model, handle } = build_ms1(GeometryType::MultiSolid);
    assert_only_surfaces_bucket(model.get_geometry(handle).unwrap().semantics().unwrap(), 4);
}

/// Inputs: S1 and D1 fixtures carrying semantics, materials, textures, and UVs.
/// Assertions: every non-null handle resolves in the model pools. Purpose:
/// positive resource-reference coverage for dense exported maps.
#[test]
fn resource_references_resolve_from_geometry_maps() {
    let S1Result { model, handle, .. } = build_s1(GeometryType::MultiSurface);
    let geom = model.get_geometry(handle).unwrap();

    for handle in geom.semantics().unwrap().surfaces().iter().flatten() {
        assert!(model.get_semantic(*handle).is_some());
    }
    let (_, material_map) = geom.materials().unwrap().first().unwrap();
    for handle in material_map.surfaces().iter().flatten() {
        assert!(model.get_material(*handle).is_some());
    }
    let (_, texture_map) = geom.textures().unwrap().first().unwrap();
    for handle in texture_map.ring_textures().into_iter().flatten() {
        assert!(model.get_texture(*handle).is_some());
    }
    for uv in texture_map.vertices().iter().flatten() {
        assert!(model.get_uv_coordinate(*uv).is_some());
    }

    let D1Result { model, handle } = build_d1();
    for handle in model
        .get_geometry(handle)
        .unwrap()
        .semantics()
        .unwrap()
        .surfaces()
        .iter()
        .flatten()
    {
        assert!(model.get_semantic(*handle).is_some());
    }
}

/// Inputs: S1 with textured outer rings and an untextured inner ring.
/// Assertions: texture maps align to boundary rings and vertex occurrences,
/// null UVs are limited to untextured rings, and reused geometry vertices may
/// have distinct UV handles. Purpose: compact positive coverage for texture
/// topology and occurrence-level UV semantics.
#[test]
fn dense_texture_maps_are_accepted() {
    let S1Result { model, handle, .. } = build_s1(GeometryType::MultiSurface);
    let geom = model.get_geometry(handle).unwrap();
    let boundary = geom.boundaries().unwrap();
    let (_, texture_map) = geom.textures().unwrap().first().unwrap();

    assert_eq!(texture_map.rings(), boundary.rings());
    assert_eq!(texture_map.ring_textures().len(), boundary.rings().len());
    assert_eq!(texture_map.vertices().len(), boundary.vertices().len());
    assert!(texture_map.ring_textures()[0].is_some());
    assert!(texture_map.ring_textures()[1].is_none());
    assert!(texture_map.ring_textures()[2].is_some());
    assert!(texture_map.vertices()[0..3].iter().all(Option::is_some));
    assert!(texture_map.vertices()[3..6].iter().all(Option::is_none));
    assert!(texture_map.vertices()[6..10].iter().all(Option::is_some));

    let ring0_v4_uv = texture_map.vertices()[2].unwrap();
    let ring2_v4_uv = texture_map.vertices()[8].unwrap();
    assert_ne!(ring0_v4_uv, ring2_v4_uv);
}
