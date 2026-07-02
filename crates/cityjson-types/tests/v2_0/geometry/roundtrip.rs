//! Boundary roundtrip tests for canonical and explicit nested topology cases.

use super::fixtures::*;
use cityjson_types::v2_0::*;

fn assert_boundary_consistent(boundary: &Boundary<u32>) {
    assert!(boundary.is_consistent());
}

/// Inputs: P1, L1, S1, D1, MS1, and T1 canonical fixture boundaries.
/// Assertions: flat -> nested -> flat preserves every populated offset and
/// vertex buffer. Purpose: broad canonical roundtrip coverage without a test per
/// fixture and direction.
#[test]
fn flat_fixture_boundaries_roundtrip_by_type() {
    let P1Result { model, handle } = build_p1();
    let boundary = model.get_geometry(handle).unwrap().boundaries().unwrap();
    let back: Boundary<u32> = boundary.to_nested_multi_point().unwrap().into();
    assert_boundary_consistent(&back);
    assert_eq!(back.vertices(), boundary.vertices());

    let L1Result { model, handle } = build_l1();
    let boundary = model.get_geometry(handle).unwrap().boundaries().unwrap();
    let nested: BoundaryNestedMultiLineString32 = boundary.to_nested_multi_linestring().unwrap();
    let back: Boundary<u32> = nested.try_into().unwrap();
    assert_boundary_consistent(&back);
    assert_eq!(back.vertices(), boundary.vertices());
    assert_eq!(back.rings(), boundary.rings());

    let S1Result { model, handle, .. } = build_s1(GeometryType::MultiSurface);
    let boundary = model.get_geometry(handle).unwrap().boundaries().unwrap();
    let nested: BoundaryNestedMultiOrCompositeSurface32 =
        boundary.to_nested_multi_or_composite_surface().unwrap();
    let back: Boundary<u32> = nested.try_into().unwrap();
    assert_boundary_consistent(&back);
    assert_eq!(back.vertices(), boundary.vertices());
    assert_eq!(back.rings(), boundary.rings());
    assert_eq!(back.surfaces(), boundary.surfaces());

    let D1Result { model, handle } = build_d1();
    let boundary = model.get_geometry(handle).unwrap().boundaries().unwrap();
    let nested: BoundaryNestedSolid32 = boundary.to_nested_solid().unwrap();
    let back: Boundary<u32> = nested.try_into().unwrap();
    assert_boundary_consistent(&back);
    assert_eq!(back.vertices(), boundary.vertices());
    assert_eq!(back.rings(), boundary.rings());
    assert_eq!(back.surfaces(), boundary.surfaces());
    assert_eq!(back.shells(), boundary.shells());

    let MS1Result { model, handle } = build_ms1(GeometryType::MultiSolid);
    let boundary = model.get_geometry(handle).unwrap().boundaries().unwrap();
    let nested: BoundaryNestedMultiOrCompositeSolid32 =
        boundary.to_nested_multi_or_composite_solid().unwrap();
    let back: Boundary<u32> = nested.try_into().unwrap();
    assert_boundary_consistent(&back);
    assert_eq!(back.vertices(), boundary.vertices());
    assert_eq!(back.rings(), boundary.rings());
    assert_eq!(back.surfaces(), boundary.surfaces());
    assert_eq!(back.shells(), boundary.shells());
    assert_eq!(back.solids(), boundary.solids());

    let T1Result {
        model,
        template_handle,
    } = build_t1();
    let boundary = model
        .get_geometry_template(template_handle)
        .unwrap()
        .boundaries()
        .unwrap();
    let nested: BoundaryNestedMultiOrCompositeSurface32 =
        boundary.to_nested_multi_or_composite_surface().unwrap();
    let back: Boundary<u32> = nested.try_into().unwrap();
    assert_boundary_consistent(&back);
    assert_eq!(back.vertices(), boundary.vertices());
    assert_eq!(back.rings(), boundary.rings());
    assert_eq!(back.surfaces(), boundary.surfaces());
}

/// Inputs: explicit nested line, surface, solid, and multi-solid boundaries.
/// Assertions: nested -> flat -> nested preserves line grouping, inner-ring
/// attachment, shell contents, and solid ordering. Purpose: targeted topology
/// preservation coverage that canonical fixtures alone do not make obvious.
#[test]
fn nested_boundaries_roundtrip_and_preserve_grouping() {
    let lines: BoundaryNestedMultiLineString32 = vec![vec![0u32, 1], vec![1, 2, 3]];
    let flat: Boundary<u32> = lines.clone().try_into().unwrap();
    assert_boundary_consistent(&flat);
    assert_eq!(flat.to_nested_multi_linestring().unwrap(), lines);

    let surfaces: BoundaryNestedMultiOrCompositeSurface32 = vec![
        vec![vec![0u32, 1, 4], vec![0, 2, 1]],
        vec![vec![2, 3, 4, 5]],
    ];
    let flat: Boundary<u32> = surfaces.clone().try_into().unwrap();
    assert_boundary_consistent(&flat);
    assert_eq!(flat.surfaces().len(), 2);
    assert_eq!(flat.rings().len(), 3);
    let back = flat.to_nested_multi_or_composite_surface().unwrap();
    assert_eq!(back, surfaces);
    assert_eq!(back[0].len(), 2);
    assert_eq!(back[1].len(), 1);

    let solid: BoundaryNestedSolid32 = vec![
        vec![vec![vec![0u32, 1, 2]], vec![vec![2, 3, 4]]],
        vec![vec![vec![4u32, 5, 0]], vec![vec![1, 6, 7]]],
    ];
    let flat: Boundary<u32> = solid.clone().try_into().unwrap();
    assert_boundary_consistent(&flat);
    assert_eq!(flat.shells().len(), 2);
    assert_eq!(flat.surfaces().len(), 4);
    assert_eq!(flat.to_nested_solid().unwrap(), solid);

    let multisolid: BoundaryNestedMultiOrCompositeSolid32 = vec![
        vec![vec![vec![vec![0u32, 1, 2]], vec![vec![0, 2, 1]]]],
        vec![vec![vec![vec![3u32, 4, 5]], vec![vec![3, 5, 4]]]],
    ];
    let flat: Boundary<u32> = multisolid.clone().try_into().unwrap();
    assert_boundary_consistent(&flat);
    assert_eq!(flat.solids().len(), 2);
    assert_eq!(flat.shells().len(), 2);
    assert_eq!(
        flat.to_nested_multi_or_composite_solid().unwrap(),
        multisolid
    );
}
