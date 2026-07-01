//! Shared boundary fixtures for unit tests.

use super::Boundary;
use crate::cityjson::core::boundary::nested::{
    BoundaryNestedMultiLineString32, BoundaryNestedMultiOrCompositeSolid32,
    BoundaryNestedMultiOrCompositeSurface32, BoundaryNestedMultiPoint32, BoundaryNestedSolid32,
};
use crate::cityjson::core::coordinate::RealWorldCoordinate;
use crate::cityjson::core::vertices::Vertices;

pub(crate) fn vertices() -> Vertices<u32, RealWorldCoordinate> {
    Vertices::from(vec![
        RealWorldCoordinate::new(0.0, 0.0, 0.0),
        RealWorldCoordinate::new(1.5, 0.0, -2.0),
        RealWorldCoordinate::new(1.5, 1.25, 3.0),
        RealWorldCoordinate::new(0.0, 1.25, 0.5),
        RealWorldCoordinate::new(0.25, 0.25, 1.0),
        RealWorldCoordinate::new(1.25, 0.25, 1.5),
        RealWorldCoordinate::new(1.25, 1.0, 2.0),
        RealWorldCoordinate::new(0.25, 1.0, 2.5),
        RealWorldCoordinate::new(2.0, 0.0, 0.0),
        RealWorldCoordinate::new(3.0, 0.0, 0.25),
        RealWorldCoordinate::new(3.0, 1.0, 0.5),
        RealWorldCoordinate::new(2.0, 1.0, 0.75),
    ])
}

pub(crate) fn multipoint_repeated_refs() -> Boundary<u32> {
    let nested: BoundaryNestedMultiPoint32 = vec![0, 2, 0, 3];
    nested.into()
}

pub(crate) fn multilinestring_variable_segments() -> Boundary<u32> {
    let nested: BoundaryNestedMultiLineString32 = vec![vec![0, 1], vec![], vec![2, 3, 0]];
    nested.try_into().unwrap()
}

pub(crate) fn surface_open_triangle() -> Boundary<u32> {
    let nested: BoundaryNestedMultiOrCompositeSurface32 = vec![vec![vec![0, 1, 2]]];
    nested.try_into().unwrap()
}

pub(crate) fn surface_with_hole() -> Boundary<u32> {
    let nested: BoundaryNestedMultiOrCompositeSurface32 =
        vec![vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]]];
    nested.try_into().unwrap()
}

pub(crate) fn multi_surface_two_polygons() -> Boundary<u32> {
    let nested: BoundaryNestedMultiOrCompositeSurface32 =
        vec![vec![vec![0, 1, 2, 3]], vec![vec![8, 9, 10, 11]]];
    nested.try_into().unwrap()
}

pub(crate) fn legacy_closed_surface() -> Boundary<u32> {
    let nested: BoundaryNestedMultiOrCompositeSurface32 = vec![vec![vec![0, 1, 2, 3, 0]]];
    nested.try_into().unwrap()
}

pub(crate) fn solid_two_shells() -> Boundary<u32> {
    let nested: BoundaryNestedSolid32 =
        vec![vec![vec![vec![4, 5, 6, 7]]], vec![vec![vec![0, 1, 2, 3]]]];
    nested.try_into().unwrap()
}

pub(crate) fn multi_solid_ordered() -> Boundary<u32> {
    let nested: BoundaryNestedMultiOrCompositeSolid32 = vec![
        vec![vec![vec![vec![4, 5, 6, 7]]]],
        vec![vec![vec![vec![0, 1, 2, 3]]], vec![vec![vec![8, 9, 10, 11]]]],
    ];
    nested.try_into().unwrap()
}
