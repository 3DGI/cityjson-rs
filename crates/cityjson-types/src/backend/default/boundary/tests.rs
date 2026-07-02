use super::*;
use crate::cityjson::core::boundary::nested::{
    BoundaryNestedMultiLineString32, BoundaryNestedMultiOrCompositeSolid32,
    BoundaryNestedMultiOrCompositeSurface32, BoundaryNestedMultiPoint32, BoundaryNestedSolid32,
};
use crate::cityjson::core::vertex::VertexIndex;
use crate::v2_0::{GeometryVertices32, RealWorldCoordinate};

// Helper function to create vertex indices
fn vi<T: VertexRef>(value: T) -> VertexIndex<T> {
    VertexIndex::new(value)
}

/// Inputs: flat boundaries with progressively populated hierarchy levels.
/// Assertions: `check_type()` reports the highest populated level. Purpose:
/// define boundary type detection independently from geometry validation.
#[test]
fn boundary_type_detection_reports_highest_populated_level() {
    // Create various boundary types
    let mut multi_point_boundary: Boundary<u32> = Boundary::new();
    multi_point_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    assert_eq!(multi_point_boundary.check_type(), BoundaryType::MultiPoint);

    let mut multi_line_boundary: Boundary<u32> = Boundary::new();
    multi_line_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    multi_line_boundary.rings = vec![vi(0)];
    assert_eq!(
        multi_line_boundary.check_type(),
        BoundaryType::MultiLineString
    );

    let mut multi_surface_boundary: Boundary<u32> = Boundary::new();
    multi_surface_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    multi_surface_boundary.rings = vec![vi(0)];
    multi_surface_boundary.surfaces = vec![vi(0)];
    assert_eq!(
        multi_surface_boundary.check_type(),
        BoundaryType::MultiOrCompositeSurface
    );

    let mut solid_boundary: Boundary<u32> = Boundary::new();
    solid_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    solid_boundary.rings = vec![vi(0)];
    solid_boundary.surfaces = vec![vi(0)];
    solid_boundary.shells = vec![vi(0)];
    assert_eq!(solid_boundary.check_type(), BoundaryType::Solid);

    let mut multi_solid_boundary: Boundary<u32> = Boundary::new();
    multi_solid_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    multi_solid_boundary.rings = vec![vi(0)];
    multi_solid_boundary.surfaces = vec![vi(0)];
    multi_solid_boundary.shells = vec![vi(0)];
    multi_solid_boundary.solids = vec![vi(0)];
    assert_eq!(
        multi_solid_boundary.check_type(),
        BoundaryType::MultiOrCompositeSolid
    );
}

#[test]
fn boundary_consistency() {
    // Consistent boundary - basic multilinestring
    let mut consistent: Boundary<u32> = Boundary::new();
    consistent.vertices = vec![vi(0), vi(1), vi(2), vi(3)];
    consistent.rings = vec![vi(0), vi(2)];
    assert!(consistent.is_consistent());

    // Consistent boundary - multi-surface
    let mut consistent2: Boundary<u32> = Boundary::new();
    consistent2.vertices = vec![vi(0), vi(1), vi(2), vi(3), vi(4), vi(5)];
    consistent2.rings = vec![vi(0), vi(3), vi(6)]; // Note: vi(6) is out of bounds, but it's allowed as the "end" pointer
    consistent2.surfaces = vec![vi(0), vi(2)];
    assert!(consistent2.is_consistent());

    // Inconsistent boundary - ring references out of bounds
    let mut inconsistent: Boundary<u32> = Boundary::new();
    inconsistent.vertices = vec![vi(0), vi(1)];
    inconsistent.rings = vec![vi(0), vi(3)]; // references vertex 3, which doesn't exist
    assert!(!inconsistent.is_consistent());

    // Inconsistent boundary - surface references out of bounds
    let mut inconsistent2: Boundary<u32> = Boundary::new();
    inconsistent2.vertices = vec![vi(0), vi(1), vi(2), vi(3)];
    inconsistent2.rings = vec![vi(0)];
    inconsistent2.surfaces = vec![vi(0), vi(2)]; // references ring 2, which doesn't exist
    assert!(!inconsistent2.is_consistent());
}

#[test]
fn boundary_consistency_allows_empty_segments() {
    let nested: BoundaryNestedMultiLineString32 = vec![vec![0, 1, 2], vec![], vec![3, 4]];
    let flattened: Boundary<u32> = nested.clone().try_into().unwrap();

    assert!(flattened.is_consistent());
    assert_eq!(flattened.to_nested_multi_linestring().unwrap(), nested);
}

#[test]
fn boundary_consistency_rejects_singleton_out_of_bounds_offsets() {
    let mut ring_boundary: Boundary<u32> = Boundary::new();
    ring_boundary.vertices = vec![vi(0), vi(1)];
    ring_boundary.rings = vec![vi(3)];
    assert!(!ring_boundary.is_consistent());
    assert!(ring_boundary.to_nested_multi_linestring().is_err());

    let mut surface_boundary: Boundary<u32> = Boundary::new();
    surface_boundary.vertices = vec![vi(0)];
    surface_boundary.rings = vec![vi(0)];
    surface_boundary.surfaces = vec![vi(2)];
    assert!(!surface_boundary.is_consistent());
    assert!(
        surface_boundary
            .to_nested_multi_or_composite_surface()
            .is_err()
    );

    let mut shell_boundary: Boundary<u32> = Boundary::new();
    shell_boundary.vertices = vec![vi(0)];
    shell_boundary.rings = vec![vi(0)];
    shell_boundary.surfaces = vec![vi(0)];
    shell_boundary.shells = vec![vi(2)];
    assert!(!shell_boundary.is_consistent());
    assert!(shell_boundary.to_nested_solid().is_err());

    let mut solid_boundary: Boundary<u32> = Boundary::new();
    solid_boundary.vertices = vec![vi(0)];
    solid_boundary.rings = vec![vi(0)];
    solid_boundary.surfaces = vec![vi(0)];
    solid_boundary.shells = vec![vi(0)];
    solid_boundary.solids = vec![vi(2)];
    assert!(!solid_boundary.is_consistent());
    assert!(solid_boundary.to_nested_multi_or_composite_solid().is_err());
}

#[test]
fn multi_point_conversion() {
    // Create nested multi-point
    let nested: BoundaryNestedMultiPoint32 = vec![0, 1, 2, 3];

    // Convert to flattened
    let flattened: Boundary<u32> = nested.clone().into();
    assert_eq!(flattened.check_type(), BoundaryType::MultiPoint);

    // Convert back to nested
    let round_trip = flattened.to_nested_multi_point().unwrap();
    assert_eq!(round_trip, nested);

    // Test incompatible conversion
    let mut multi_line_boundary: Boundary<u32> = Boundary::new();
    multi_line_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    multi_line_boundary.rings = vec![vi(0)];
    assert!(multi_line_boundary.to_nested_multi_point().is_err());
}

#[test]
fn multi_linestring_conversion() {
    // Create nested multi-linestring
    let nested: BoundaryNestedMultiLineString32 = vec![vec![0, 1, 2], vec![3, 4, 5, 6]];

    // Convert to flattened
    let flattened: Boundary<u32> = nested.clone().try_into().unwrap();
    assert_eq!(flattened.check_type(), BoundaryType::MultiLineString);

    // Convert back to nested
    let round_trip = flattened.to_nested_multi_linestring().unwrap();
    assert_eq!(round_trip, nested);

    // Test incompatible conversion
    let mut multi_point_boundary: Boundary<u32> = Boundary::new();
    multi_point_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    assert!(multi_point_boundary.to_nested_multi_linestring().is_err());
}

#[test]
fn coordinates_iterates_boundary_order() {
    let boundary: Boundary<u32> = vec![vec![0, 1, 0, 2]].try_into().unwrap();
    let vertices = GeometryVertices32::from(vec![
        RealWorldCoordinate::new(10.0, 0.0, 0.0),
        RealWorldCoordinate::new(20.0, 0.0, 0.0),
        RealWorldCoordinate::new(30.0, 0.0, 0.0),
    ]);

    let xs: Vec<f64> = boundary
        .coordinates(&vertices)
        .map(RealWorldCoordinate::x)
        .collect();

    assert_eq!(xs, vec![10.0, 20.0, 10.0, 30.0]);
}

#[test]
fn unique_coordinates_deduplicates_vertex_references() {
    let boundary: Boundary<u32> = vec![vec![0, 1, 0, 2], vec![2, 1, 0]].try_into().unwrap();
    let vertices = GeometryVertices32::from(vec![
        RealWorldCoordinate::new(10.0, 0.0, 0.0),
        RealWorldCoordinate::new(20.0, 0.0, 0.0),
        RealWorldCoordinate::new(30.0, 0.0, 0.0),
    ]);
    let mut scratch = Vec::new();

    let xs: Vec<f64> = boundary
        .unique_coordinates(&vertices, &mut scratch)
        .map(RealWorldCoordinate::x)
        .collect();

    assert_eq!(xs, vec![10.0, 20.0, 30.0]);
    assert_eq!(
        boundary
            .unique_vertex_indices(&mut scratch)
            .iter()
            .map(VertexIndex::to_usize)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn multi_surface_conversion() {
    // Create nested multi-surface
    let nested: BoundaryNestedMultiOrCompositeSurface32 = vec![
        // First surface with one ring
        vec![vec![0, 1, 2, 0]],
        // Second surface with two rings (outer and inner)
        vec![vec![3, 4, 5, 3], vec![6, 7, 8, 6]],
    ];

    // Convert to flattened
    let flattened: Boundary<u32> = nested.clone().try_into().unwrap();
    assert_eq!(
        flattened.check_type(),
        BoundaryType::MultiOrCompositeSurface
    );

    // Convert back to nested
    let round_trip = flattened.to_nested_multi_or_composite_surface().unwrap();
    assert_eq!(round_trip, nested);

    // Test incompatible conversion
    let mut multi_point_boundary: Boundary<u32> = Boundary::new();
    multi_point_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    assert!(
        multi_point_boundary
            .to_nested_multi_or_composite_surface()
            .is_err()
    );
}

#[test]
fn solid_conversion() {
    // Create nested solid (a simple cube)
    let nested: BoundaryNestedSolid32 = vec![
        // Outer shell with 6 faces
        vec![
            vec![vec![0, 1, 2, 3, 0]], // front face
            vec![vec![4, 5, 6, 7, 4]], // back face
            vec![vec![0, 3, 7, 4, 0]], // left face
            vec![vec![1, 2, 6, 5, 1]], // right face
            vec![vec![0, 1, 5, 4, 0]], // bottom face
            vec![vec![3, 2, 6, 7, 3]], // top face
        ],
    ];

    // Convert to flattened
    let flattened: Boundary<u32> = nested.clone().try_into().unwrap();
    assert_eq!(flattened.check_type(), BoundaryType::Solid);

    // Convert back to nested
    let round_trip = flattened.to_nested_solid().unwrap();
    assert_eq!(round_trip, nested);

    // Test incompatible conversion
    let mut multi_point_boundary: Boundary<u32> = Boundary::new();
    multi_point_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    assert!(multi_point_boundary.to_nested_solid().is_err());
}

#[test]
fn multi_solid_conversion() {
    // Create nested multi-solid (two simple cubes)
    let nested: BoundaryNestedMultiOrCompositeSolid32 = vec![
        // First solid - just a single triangular face for simplicity
        vec![vec![vec![vec![0, 1, 2, 0]]]],
        // Second solid - also a single triangular face
        vec![vec![vec![vec![3, 4, 5, 3]]]],
    ];

    // Convert to flattened
    let flattened: Boundary<u32> = nested.clone().try_into().unwrap();
    assert_eq!(flattened.check_type(), BoundaryType::MultiOrCompositeSolid);

    // Convert back to nested
    let round_trip = flattened.to_nested_multi_or_composite_solid().unwrap();
    assert_eq!(round_trip, nested);

    // Test incompatible conversion
    let mut multi_point_boundary: Boundary<u32> = Boundary::new();
    multi_point_boundary.vertices = vec![vi(0), vi(1), vi(2)];
    assert!(
        multi_point_boundary
            .to_nested_multi_or_composite_solid()
            .is_err()
    );
}

#[test]
fn from_parts_unchecked_matches_checked_layout_for_valid_input() {
    let nested: BoundaryNestedMultiOrCompositeSolid32 = vec![
        vec![
            vec![
                vec![vec![0, 1, 2, 0]],
                vec![vec![3, 4, 5, 3], vec![6, 7, 8, 6]],
            ],
            vec![vec![vec![9, 10, 11, 9]]],
        ],
        vec![vec![vec![vec![12, 13, 14, 12]]]],
    ];
    let checked: Boundary<u32> = nested.clone().try_into().unwrap();

    let unchecked = unsafe {
        Boundary::from_parts_unchecked(
            checked.vertices().to_vec(),
            checked.rings().to_vec(),
            checked.surfaces().to_vec(),
            checked.shells().to_vec(),
            checked.solids().to_vec(),
        )
    };

    assert_eq!(unchecked, checked);
    assert!(unchecked.is_consistent());
    assert_eq!(
        unchecked.to_nested_multi_or_composite_solid().unwrap(),
        nested
    );
}

#[test]
fn from_parts_checked_rejects_invalid_offsets_but_unchecked_preserves_them() {
    let vertices = vec![vi(0_u32), vi(1_u32), vi(2_u32), vi(3_u32)];
    let rings = vec![vi(1_u32), vi(4_u32)];

    let error = Boundary::from_parts(
        vertices.clone(),
        rings.clone(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(error, error::Error::InvalidGeometry(_)));

    let unchecked = unsafe {
        Boundary::from_parts_unchecked(vertices, rings, Vec::new(), Vec::new(), Vec::new())
    };
    assert_eq!(unchecked.check_type(), BoundaryType::MultiLineString);
    assert!(!unchecked.is_consistent());
    assert_eq!(&*unchecked.vertices_raw(), &[0, 1, 2, 3]);
    assert_eq!(&*unchecked.rings_raw(), &[1, 4]);
    assert!(unchecked.to_nested_multi_linestring().is_err());
}

#[cfg(test)]
mod nested_tests {
    use super::*;
    use crate::cityjson::core::boundary::nested::{
        BoundaryNestedMultiLineString16, BoundaryNestedMultiLineString32,
        BoundaryNestedMultiOrCompositeSolid16, BoundaryNestedMultiOrCompositeSolid32,
        BoundaryNestedMultiOrCompositeSurface16, BoundaryNestedMultiOrCompositeSurface32,
        BoundaryNestedMultiPoint32, BoundaryNestedSolid16, BoundaryNestedSolid32,
    };
    use crate::cityjson::core::vertex::VertexIndex;

    const U16_MAX: usize = u16::MAX as usize;
    const U16_MAX_PLUS_ONE: usize = (u16::MAX as usize) + 1;

    #[test]
    fn empty_nested_conversions() {
        // Test empty MultiPoint
        let empty_multi_point: BoundaryNestedMultiPoint32 = vec![];
        let boundary: Boundary<u32> = empty_multi_point.into();
        assert_eq!(boundary.check_type(), BoundaryType::None);

        // Test empty MultiLineString
        let empty_multi_linestring: BoundaryNestedMultiLineString32 = vec![];
        let boundary: Boundary<u32> = empty_multi_linestring.try_into().unwrap();
        assert_eq!(boundary.check_type(), BoundaryType::None);

        // Test empty MultiSurface
        let empty_multi_surface: BoundaryNestedMultiOrCompositeSurface32 = vec![];
        let boundary: Boundary<u32> = empty_multi_surface.try_into().unwrap();
        assert_eq!(boundary.check_type(), BoundaryType::None);

        // Test empty Solid
        let empty_solid: BoundaryNestedSolid32 = vec![];
        let boundary: Boundary<u32> = empty_solid.try_into().unwrap();
        assert_eq!(boundary.check_type(), BoundaryType::None);

        // Test empty MultiSolid
        let empty_multisolid: BoundaryNestedMultiOrCompositeSolid32 = vec![];
        let boundary: Boundary<u32> = empty_multisolid.try_into().unwrap();
        assert_eq!(boundary.check_type(), BoundaryType::None);
    }

    #[test]
    fn nested_multilinestring_with_empty_linestrings() {
        // Create a nested multi-linestring with an empty linestring
        let nested: BoundaryNestedMultiLineString32 = vec![
            vec![0, 1, 2],
            vec![], // Empty linestring
            vec![3, 4, 5],
        ];

        // Convert to flattened
        let flattened: Boundary<u32> = nested.clone().try_into().unwrap();
        assert_eq!(flattened.check_type(), BoundaryType::MultiLineString);
        assert!(flattened.is_consistent());

        // Convert back to nested
        let round_trip = flattened.to_nested_multi_linestring().unwrap();

        // The empty linestring should be preserved
        assert_eq!(round_trip.len(), 3);
        assert_eq!(round_trip[0], vec![0, 1, 2]);
        assert_eq!(round_trip[1], Vec::<u32>::new()); // Empty linestring preserved
        assert_eq!(round_trip[2], vec![3, 4, 5]);
    }

    #[test]
    fn nested_multisurface_with_empty_components() {
        // Create a nested multi-surface with an empty surface
        let nested: BoundaryNestedMultiOrCompositeSurface<u32> = vec![
            vec![vec![0, 1, 2, 0]],
            vec![], // Empty surface (no rings)
            vec![vec![3, 4, 5, 3]],
        ];

        // Convert to flattened
        let flattened: Boundary<u32> = nested.clone().try_into().unwrap();
        assert_eq!(
            flattened.check_type(),
            BoundaryType::MultiOrCompositeSurface
        );
        assert!(flattened.is_consistent());

        // Convert back to nested
        let round_trip = flattened.to_nested_multi_or_composite_surface().unwrap();

        let empty_surface = BoundaryNestedMultiLineString32::default();
        // The empty surface should be preserved
        assert_eq!(round_trip.len(), 3);
        assert_eq!(round_trip[1], empty_surface); // Empty surface preserved
    }

    #[test]
    fn multisolid_with_complex_structure() {
        // Test a multi-solid with multiple levels of nesting
        let nested: BoundaryNestedMultiOrCompositeSolid32 = vec![
            // First solid with two shells
            vec![
                // Outer shell with two surfaces
                vec![
                    vec![vec![0, 1, 2, 0]], // First surface
                    vec![vec![3, 4, 5, 3]], // Second surface
                ],
                // Inner shell with one surface
                vec![vec![vec![6, 7, 8, 6]]],
            ],
            // Second solid with one shell
            vec![
                // One shell with one surface with two rings (outer and inner)
                vec![vec![vec![9, 10, 11, 9], vec![12, 13, 14, 12]]],
            ],
        ];

        // Convert to flattened
        let flattened: Boundary<u32> = nested.clone().try_into().unwrap();
        assert_eq!(flattened.check_type(), BoundaryType::MultiOrCompositeSolid);
        assert!(flattened.is_consistent());

        // Convert back to nested
        let round_trip = flattened.to_nested_multi_or_composite_solid().unwrap();

        // The complex structure should be preserved
        assert_eq!(round_trip, nested);
    }

    #[test]
    fn nested_solid_with_empty_shells_is_consistent() {
        let nested: BoundaryNestedSolid32 = vec![vec![vec![vec![0, 1, 2, 0]]], vec![]];
        let flattened: Boundary<u32> = nested.clone().try_into().unwrap();

        assert_eq!(flattened.check_type(), BoundaryType::Solid);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.to_nested_solid().unwrap(), nested);
    }

    #[test]
    fn nested_multisolid_with_empty_solids_is_consistent() {
        let nested: BoundaryNestedMultiOrCompositeSolid32 =
            vec![vec![vec![vec![vec![0, 1, 2, 0]]]], vec![]];
        let flattened: Boundary<u32> = nested.clone().try_into().unwrap();

        assert_eq!(flattened.check_type(), BoundaryType::MultiOrCompositeSolid);
        assert!(flattened.is_consistent());
        assert_eq!(
            flattened.to_nested_multi_or_composite_solid().unwrap(),
            nested
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_multilinestring_u16_max_vertices_roundtrips() {
        let nested: BoundaryNestedMultiLineString16 = vec![vec![0; U16_MAX]];
        let flattened = Boundary::<u16>::try_from(nested.clone()).unwrap();

        assert_eq!(flattened.check_type(), BoundaryType::MultiLineString);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.vertices.len(), U16_MAX);
        assert_eq!(flattened.to_nested_multi_linestring().unwrap(), nested);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_multisurface_u16_max_rings_roundtrips() {
        let nested: BoundaryNestedMultiOrCompositeSurface16 = vec![vec![vec![]; U16_MAX], vec![]];
        let flattened = Boundary::<u16>::try_from(nested.clone()).unwrap();

        assert_eq!(
            flattened.check_type(),
            BoundaryType::MultiOrCompositeSurface
        );
        assert!(flattened.is_consistent());
        assert_eq!(flattened.rings.len(), U16_MAX);
        assert_eq!(
            flattened.to_nested_multi_or_composite_surface().unwrap(),
            nested
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_solid_u16_max_surfaces_roundtrips() {
        let nested: BoundaryNestedSolid16 = vec![vec![vec![]; U16_MAX], vec![]];
        let flattened = Boundary::<u16>::try_from(nested.clone()).unwrap();

        assert_eq!(flattened.check_type(), BoundaryType::Solid);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.surfaces.len(), U16_MAX);
        assert_eq!(flattened.to_nested_solid().unwrap(), nested);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_multisolid_u16_max_shells_roundtrips() {
        let nested: BoundaryNestedMultiOrCompositeSolid16 = vec![vec![vec![]; U16_MAX], vec![]];
        let flattened = Boundary::<u16>::try_from(nested.clone()).unwrap();

        assert_eq!(flattened.check_type(), BoundaryType::MultiOrCompositeSolid);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.shells.len(), U16_MAX);
        assert_eq!(
            flattened.to_nested_multi_or_composite_solid().unwrap(),
            nested
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_multilinestring_overflow_returns_index_overflow() {
        let nested: BoundaryNestedMultiLineString16 = vec![vec![0; U16_MAX_PLUS_ONE]];
        let result = Boundary::<u16>::try_from(nested);
        assert_eq!(
            result.unwrap_err(),
            error::Error::IndexOverflow {
                index_type: std::any::type_name::<u16>().to_string(),
                value: u16::MAX.to_string(),
            }
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_multisurface_overflow_returns_index_conversion() {
        let nested: BoundaryNestedMultiOrCompositeSurface16 =
            vec![vec![vec![]; U16_MAX_PLUS_ONE], vec![]];
        let result = Boundary::<u16>::try_from(nested);
        assert_eq!(
            result.unwrap_err(),
            error::Error::IndexConversion {
                source_type: "usize".to_string(),
                target_type: std::any::type_name::<u16>().to_string(),
                value: U16_MAX_PLUS_ONE.to_string(),
            }
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_solid_overflow_returns_index_conversion() {
        let nested: BoundaryNestedSolid16 = vec![vec![vec![]; U16_MAX_PLUS_ONE], vec![]];
        let result = Boundary::<u16>::try_from(nested);
        assert_eq!(
            result.unwrap_err(),
            error::Error::IndexConversion {
                source_type: "usize".to_string(),
                target_type: std::any::type_name::<u16>().to_string(),
                value: U16_MAX_PLUS_ONE.to_string(),
            }
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn nested_multisolid_overflow_returns_index_conversion() {
        let nested: BoundaryNestedMultiOrCompositeSolid16 =
            vec![vec![vec![]; U16_MAX_PLUS_ONE], vec![]];
        let result = Boundary::<u16>::try_from(nested);
        assert_eq!(
            result.unwrap_err(),
            error::Error::IndexConversion {
                source_type: "usize".to_string(),
                target_type: std::any::type_name::<u16>().to_string(),
                value: U16_MAX_PLUS_ONE.to_string(),
            }
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn to_nested_multilinestring_overflow_returns_err_without_panic() {
        let mut boundary = Boundary::<u16>::new();
        boundary.vertices = vec![VertexIndex::new(0); U16_MAX_PLUS_ONE];
        boundary.rings = vec![VertexIndex::new(0)];

        let result = std::panic::catch_unwind(|| boundary.to_nested_multi_linestring());
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn to_nested_multisurface_overflow_returns_err_without_panic() {
        let mut boundary = Boundary::<u16>::new();
        boundary.rings = vec![VertexIndex::new(0); U16_MAX_PLUS_ONE];
        boundary.surfaces = vec![VertexIndex::new(0)];

        let result = std::panic::catch_unwind(|| boundary.to_nested_multi_or_composite_surface());
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn to_nested_solid_overflow_returns_err_without_panic() {
        let mut boundary = Boundary::<u16>::new();
        boundary.surfaces = vec![VertexIndex::new(0); U16_MAX_PLUS_ONE];
        boundary.shells = vec![VertexIndex::new(0)];

        let result = std::panic::catch_unwind(|| boundary.to_nested_solid());
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K allocations too slow under Miri
    fn to_nested_multisolid_overflow_returns_err_without_panic() {
        let mut boundary = Boundary::<u16>::new();
        boundary.shells = vec![VertexIndex::new(0); U16_MAX_PLUS_ONE];
        boundary.solids = vec![VertexIndex::new(0)];

        let result = std::panic::catch_unwind(|| boundary.to_nested_multi_or_composite_solid());
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
    }
}

// ---------------------------------------------------------------------------
// Geometry test families 2 & 3: boundary offset and kind-shape invariants
// ---------------------------------------------------------------------------

/// Family 2: Reject invalid boundary offsets.
///
/// Each mutation starts from a valid S1-like boundary and breaks one offset rule.
/// The expected result is `is_consistent()` returning `false` and nested
/// conversion returning an error.
#[cfg(test)]
mod boundary_offsets {
    use super::*;

    /// Build a minimal valid `MultiSurface` boundary (2 surfaces, 3 rings, 10 vertices).
    fn valid_multisurface() -> Boundary<u32> {
        // surface 0: ring 0 (outer, 3 verts) + ring 1 (inner, 3 verts)
        // surface 1: ring 2 (outer, 4 verts)
        let mut b = Boundary::<u32>::new();
        b.vertices = (0u32..10).map(VertexIndex::new).collect();
        // rings: [0, 3, 6]  (ring 0 starts at vertex 0, ring 1 at 3, ring 2 at 6)
        b.rings = vec![vi(0), vi(3), vi(6)];
        // surfaces: [0, 2]  (surface 0 starts at ring 0, surface 1 starts at ring 2)
        b.surfaces = vec![vi(0), vi(2)];
        b
    }

    #[test]
    fn valid_baseline_is_consistent() {
        let b = valid_multisurface();
        assert!(b.is_consistent(), "baseline must be consistent");
    }

    #[test]
    fn first_ring_offset_not_zero_is_inconsistent() {
        let mut b = valid_multisurface();
        b.rings[0] = vi(1); // first ring offset must be 0
        assert!(
            !b.is_consistent(),
            "non-zero first offset must be inconsistent"
        );
        assert!(b.to_nested_multi_or_composite_surface().is_err());
    }

    #[test]
    fn decreasing_ring_offset_is_inconsistent() {
        let mut b = valid_multisurface();
        // rings = [0, 5, 3]: offset at index 2 is less than at index 1
        b.rings = vec![vi(0), vi(5), vi(3)];
        assert!(!b.is_consistent(), "decreasing offset must be inconsistent");
        assert!(b.to_nested_multi_or_composite_surface().is_err());
    }

    #[test]
    fn ring_offset_exceeding_vertex_count_is_inconsistent() {
        let mut b = valid_multisurface();
        // Extend the last ring offset beyond the vertex array length
        let vlen = u32::try_from(b.vertices.len()).expect("test boundary length must fit u32");
        *b.rings.last_mut().unwrap() = vi(vlen + 5);
        assert!(
            !b.is_consistent(),
            "offset exceeding child length must be inconsistent"
        );
        assert!(b.to_nested_multi_or_composite_surface().is_err());
    }

    #[test]
    fn first_surface_offset_not_zero_is_inconsistent() {
        let mut b = valid_multisurface();
        b.surfaces[0] = vi(1); // first surface offset must be 0
        assert!(!b.is_consistent());
    }

    #[test]
    fn decreasing_surface_offset_is_inconsistent() {
        let mut b = valid_multisurface();
        // Make the second surface offset less than the first
        b.surfaces = vec![vi(0), vi(0)]; // not decreasing but identical (allowed)
        // then actually break it
        b.surfaces = vec![vi(2), vi(0)]; // decreasing: 2 then 0
        assert!(!b.is_consistent());
    }

    #[test]
    fn surface_offset_exceeding_ring_count_is_inconsistent() {
        let mut b = valid_multisurface();
        let rlen = u32::try_from(b.rings.len()).expect("test ring length must fit u32");
        *b.surfaces.last_mut().unwrap() = vi(rlen + 1);
        assert!(!b.is_consistent());
    }
}

/// Family 3: Reject invalid geometry-kind shapes.
///
/// Tests that `check_type()` correctly reflects the populated hierarchy
/// and that conversion to the wrong nested type is rejected.
#[cfg(test)]
mod boundary_kind_shapes {
    use super::*;

    /// `MultiSurface` boundary with extra shells populated:
    /// `check_type()` should report Solid, not `MultiOrCompositeSurface`,
    /// and `to_nested_multi_or_composite_surface()` should fail.
    #[test]
    fn multisurface_with_shells_is_reported_as_solid() {
        let mut b = Boundary::<u32>::new();
        b.vertices = vec![vi(0), vi(1), vi(2)];
        b.rings = vec![vi(0)];
        b.surfaces = vec![vi(0)];
        b.shells = vec![vi(0)]; // ← this makes it a Solid, not a MultiSurface
        assert_eq!(b.check_type(), BoundaryType::Solid);
        assert!(b.to_nested_multi_or_composite_surface().is_err());
    }

    /// `MultiSurface` boundary with shells and solids:
    /// `check_type()` should report `MultiOrCompositeSolid`.
    #[test]
    fn multisurface_with_shells_and_solids_is_reported_as_multisolid() {
        let mut b = Boundary::<u32>::new();
        b.vertices = vec![vi(0), vi(1), vi(2)];
        b.rings = vec![vi(0)];
        b.surfaces = vec![vi(0)];
        b.shells = vec![vi(0)];
        b.solids = vec![vi(0)]; // ← makes it MultiSolid
        assert_eq!(b.check_type(), BoundaryType::MultiOrCompositeSolid);
        assert!(b.to_nested_multi_or_composite_surface().is_err());
    }

    /// Solid boundary with solids array populated:
    /// `check_type()` reports `MultiOrCompositeSolid` (because solids is non-empty).
    #[test]
    fn solid_with_solids_array_is_reported_as_multisolid() {
        let mut b = Boundary::<u32>::new();
        b.vertices = vec![vi(0), vi(1), vi(2)];
        b.rings = vec![vi(0)];
        b.surfaces = vec![vi(0)];
        b.shells = vec![vi(0)];
        b.solids = vec![vi(0)];
        assert_eq!(b.check_type(), BoundaryType::MultiOrCompositeSolid);
        assert!(b.to_nested_solid().is_err());
    }

    /// A boundary declared as `MultiSurface` but with missing vertices is
    /// caught by `is_consistent()`.
    #[test]
    fn multisurface_with_empty_vertices_fails_conversion() {
        let mut b = Boundary::<u32>::new();
        // surfaces and rings present but no vertices
        b.rings = vec![vi(0)];
        b.surfaces = vec![vi(0)];
        // vertex array empty → ring[0]=0 is within range (0 <= 0 vertices? no, 0 >= 0 is ok per the rule)
        // Actually 0 <= vertices.len() == 0 → ok, but ring[0]=vi(0) and vertices.len()=0, so last offset check
        // Actually let me just add an out-of-bounds ring offset
        b.rings = vec![vi(0), vi(1)]; // ring 1 starts at vertex 1, but vertices is empty
        assert!(!b.is_consistent());
    }

    /// A Solid boundary is converted correctly when shells is non-empty and solids is empty.
    #[test]
    fn solid_shape_requires_non_empty_shells_and_empty_solids() {
        let mut b = Boundary::<u32>::new();
        b.vertices = vec![vi(0), vi(1), vi(2)];
        b.rings = vec![vi(0)];
        b.surfaces = vec![vi(0)];
        b.shells = vec![vi(0)];
        // solids is empty → this is a Solid
        assert_eq!(b.check_type(), BoundaryType::Solid);
        assert!(b.to_nested_solid().is_ok());
    }

    /// A `MultiSolid` boundary must have all of vertices, rings, surfaces, shells, solids non-empty.
    #[test]
    fn multisolid_with_missing_solids_is_reported_as_solid() {
        let mut b = Boundary::<u32>::new();
        b.vertices = vec![vi(0), vi(1), vi(2)];
        b.rings = vec![vi(0)];
        b.surfaces = vec![vi(0)];
        b.shells = vec![vi(0)];
        // solids empty → Solid not MultiSolid
        assert_eq!(b.check_type(), BoundaryType::Solid);
        assert!(b.to_nested_multi_or_composite_solid().is_err());
    }
}

#[cfg(test)]
// Proptest calls `getcwd` which is blocked by Miri's isolation mode,
// and the generated test cases are too slow for interpreted execution.
#[cfg(not(miri))]
mod property_tests {
    use super::*;
    use crate::cityjson::core::boundary::nested::{
        BoundaryNestedMultiLineString16, BoundaryNestedMultiOrCompositeSolid16,
        BoundaryNestedMultiOrCompositeSurface16, BoundaryNestedMultiPoint16, BoundaryNestedSolid16,
    };
    use proptest::prelude::*;

    fn cast_index<T: VertexRef>(value: u16) -> T {
        T::try_from(u32::from(value)).ok().unwrap()
    }

    fn cast_multi_point<T: VertexRef>(
        value: BoundaryNestedMultiPoint16,
    ) -> BoundaryNestedMultiPoint<T> {
        value.into_iter().map(cast_index::<T>).collect()
    }

    fn cast_multi_linestring<T: VertexRef>(
        value: BoundaryNestedMultiLineString16,
    ) -> BoundaryNestedMultiLineString<T> {
        value.into_iter().map(cast_multi_point::<T>).collect()
    }

    fn cast_multi_surface<T: VertexRef>(
        value: BoundaryNestedMultiOrCompositeSurface16,
    ) -> BoundaryNestedMultiOrCompositeSurface<T> {
        value.into_iter().map(cast_multi_linestring::<T>).collect()
    }

    fn cast_solid<T: VertexRef>(value: BoundaryNestedSolid16) -> BoundaryNestedSolid<T> {
        value.into_iter().map(cast_multi_surface::<T>).collect()
    }

    fn cast_multisolid<T: VertexRef>(
        value: BoundaryNestedMultiOrCompositeSolid16,
    ) -> BoundaryNestedMultiOrCompositeSolid<T> {
        value.into_iter().map(cast_solid::<T>).collect()
    }

    fn assert_multi_point_roundtrip<T: VertexRef + std::fmt::Debug>(
        nested: &BoundaryNestedMultiPoint<T>,
    ) {
        let flattened: Boundary<T> = nested.clone().into();
        assert_eq!(flattened.check_type(), BoundaryType::MultiPoint);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.vertices.len(), nested.len());
        assert_eq!(flattened.to_nested_multi_point().unwrap(), *nested);
    }

    fn assert_multi_linestring_roundtrip<T: VertexRef + std::fmt::Debug>(
        nested: &BoundaryNestedMultiLineString<T>,
    ) {
        let expected_vertices = nested.iter().map(std::vec::Vec::len).sum::<usize>();
        let expected_rings = nested.len();

        let flattened = Boundary::<T>::try_from(nested.clone()).unwrap();
        assert_eq!(flattened.check_type(), BoundaryType::MultiLineString);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.vertices.len(), expected_vertices);
        assert_eq!(flattened.rings.len(), expected_rings);
        assert_eq!(flattened.to_nested_multi_linestring().unwrap(), *nested);
    }

    fn assert_multi_surface_roundtrip<T: VertexRef + std::fmt::Debug>(
        nested: &BoundaryNestedMultiOrCompositeSurface<T>,
    ) {
        let expected_vertices = nested
            .iter()
            .flatten()
            .map(std::vec::Vec::len)
            .sum::<usize>();
        let expected_rings = nested.iter().map(std::vec::Vec::len).sum::<usize>();
        let expected_surfaces = nested.len();

        let flattened = Boundary::<T>::try_from(nested.clone()).unwrap();
        assert_eq!(
            flattened.check_type(),
            BoundaryType::MultiOrCompositeSurface
        );
        assert!(flattened.is_consistent());
        assert_eq!(flattened.vertices.len(), expected_vertices);
        assert_eq!(flattened.rings.len(), expected_rings);
        assert_eq!(flattened.surfaces.len(), expected_surfaces);
        assert_eq!(
            flattened.to_nested_multi_or_composite_surface().unwrap(),
            *nested
        );
    }

    fn assert_solid_roundtrip<T: VertexRef + std::fmt::Debug>(nested: &BoundaryNestedSolid<T>) {
        let expected_vertices = nested
            .iter()
            .flatten()
            .flatten()
            .map(std::vec::Vec::len)
            .sum::<usize>();
        let expected_rings = nested
            .iter()
            .flatten()
            .map(std::vec::Vec::len)
            .sum::<usize>();
        let expected_surfaces = nested.iter().map(std::vec::Vec::len).sum::<usize>();
        let expected_shells = nested.len();

        let flattened = Boundary::<T>::try_from(nested.clone()).unwrap();
        assert_eq!(flattened.check_type(), BoundaryType::Solid);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.vertices.len(), expected_vertices);
        assert_eq!(flattened.rings.len(), expected_rings);
        assert_eq!(flattened.surfaces.len(), expected_surfaces);
        assert_eq!(flattened.shells.len(), expected_shells);
        assert_eq!(flattened.to_nested_solid().unwrap(), *nested);
    }

    fn assert_multisolid_roundtrip<T: VertexRef + std::fmt::Debug>(
        nested: &BoundaryNestedMultiOrCompositeSolid<T>,
    ) {
        let expected_vertices = nested
            .iter()
            .flatten()
            .flatten()
            .flatten()
            .map(std::vec::Vec::len)
            .sum::<usize>();
        let expected_rings = nested
            .iter()
            .flatten()
            .flatten()
            .map(std::vec::Vec::len)
            .sum::<usize>();
        let expected_surfaces = nested
            .iter()
            .flatten()
            .map(std::vec::Vec::len)
            .sum::<usize>();
        let expected_shells = nested.iter().map(std::vec::Vec::len).sum::<usize>();
        let expected_solids = nested.len();

        let flattened = Boundary::<T>::try_from(nested.clone()).unwrap();
        assert_eq!(flattened.check_type(), BoundaryType::MultiOrCompositeSolid);
        assert!(flattened.is_consistent());
        assert_eq!(flattened.vertices.len(), expected_vertices);
        assert_eq!(flattened.rings.len(), expected_rings);
        assert_eq!(flattened.surfaces.len(), expected_surfaces);
        assert_eq!(flattened.shells.len(), expected_shells);
        assert_eq!(flattened.solids.len(), expected_solids);
        assert_eq!(
            flattened.to_nested_multi_or_composite_solid().unwrap(),
            *nested
        );
    }

    fn assert_across_widths<I16, F16, F32, F64>(
        nested: &I16,
        assert_u16: F16,
        assert_u32: F32,
        assert_u64: F64,
    ) where
        I16: Clone,
        F16: FnOnce(&I16),
        F32: FnOnce(I16),
        F64: FnOnce(I16),
    {
        assert_u16(nested);
        assert_u32(nested.clone());
        assert_u64(nested.clone());
    }

    fn offset_u32(value: usize) -> VertexIndex<u32> {
        VertexIndex::try_from(value).unwrap()
    }

    fn malformed_boundary_strategy() -> impl Strategy<Value = Boundary<u32>> {
        prop_oneof![
            (1usize..6, 0usize..4).prop_map(|(start, extra)| {
                let mut boundary = Boundary::new();
                boundary.vertices = vec![VertexIndex::new(0u32); start + extra];
                boundary.rings = vec![offset_u32(start)];
                boundary
            }),
            (2usize..8, 1usize..4).prop_map(|(mid, tail)| {
                let mut boundary = Boundary::new();
                boundary.vertices = vec![VertexIndex::new(0u32); mid + tail];
                boundary.rings = vec![offset_u32(0), offset_u32(mid), offset_u32(mid - 1)];
                boundary
            }),
            (0usize..6, 1usize..4).prop_map(|(len, delta)| {
                let mut boundary = Boundary::new();
                boundary.vertices = vec![VertexIndex::new(0u32); len];
                boundary.rings = vec![offset_u32(0), offset_u32(len + delta)];
                boundary
            }),
            (1usize..4).prop_map(|delta| {
                let mut boundary = Boundary::new();
                boundary.vertices = vec![VertexIndex::new(0u32)];
                boundary.rings = vec![offset_u32(0)];
                boundary.surfaces = vec![offset_u32(1 + delta)];
                boundary
            }),
            (1usize..4).prop_map(|delta| {
                let mut boundary = Boundary::new();
                boundary.vertices = vec![VertexIndex::new(0u32)];
                boundary.rings = vec![offset_u32(0)];
                boundary.surfaces = vec![offset_u32(0)];
                boundary.shells = vec![offset_u32(1 + delta)];
                boundary
            }),
            (1usize..4).prop_map(|delta| {
                let mut boundary = Boundary::new();
                boundary.vertices = vec![VertexIndex::new(0u32)];
                boundary.rings = vec![offset_u32(0)];
                boundary.surfaces = vec![offset_u32(0)];
                boundary.shells = vec![offset_u32(0)];
                boundary.solids = vec![offset_u32(1 + delta)];
                boundary
            }),
        ]
    }

    fn assert_inconsistent_boundary_rejected(boundary: &Boundary<u32>) {
        assert!(!boundary.is_consistent());

        let result = std::panic::catch_unwind(|| match boundary.check_type() {
            BoundaryType::MultiLineString => boundary.to_nested_multi_linestring().map(|_| ()),
            BoundaryType::MultiOrCompositeSurface => {
                boundary.to_nested_multi_or_composite_surface().map(|_| ())
            }
            BoundaryType::Solid => boundary.to_nested_solid().map(|_| ()),
            BoundaryType::MultiOrCompositeSolid => {
                boundary.to_nested_multi_or_composite_solid().map(|_| ())
            }
            other => panic!("unexpected malformed boundary type: {other:?}"),
        });

        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
    }

    fn valid_multi_point_strategy() -> impl Strategy<Value = BoundaryNestedMultiPoint16> {
        prop::collection::vec(0u16..128, 1..8)
    }

    fn valid_multi_linestring_strategy() -> impl Strategy<Value = BoundaryNestedMultiLineString16> {
        prop::collection::vec(prop::collection::vec(0u16..128, 0..6), 1..5)
    }

    fn valid_multi_surface_strategy()
    -> impl Strategy<Value = BoundaryNestedMultiOrCompositeSurface16> {
        prop::collection::vec(valid_multi_linestring_strategy(), 1..4)
    }

    fn valid_solid_strategy() -> impl Strategy<Value = BoundaryNestedSolid16> {
        prop::collection::vec(valid_multi_surface_strategy(), 1..3)
    }

    fn valid_multisolid_strategy() -> impl Strategy<Value = BoundaryNestedMultiOrCompositeSolid16> {
        prop::collection::vec(valid_solid_strategy(), 1..3)
    }

    proptest! {
        #[test]
        fn valid_multi_point_roundtrips_across_widths(nested in valid_multi_point_strategy()) {
            assert_across_widths(
                &nested,
                assert_multi_point_roundtrip::<u16>,
                |value| assert_multi_point_roundtrip::<u32>(&cast_multi_point::<u32>(value.clone())),
                |value| assert_multi_point_roundtrip::<u64>(&cast_multi_point::<u64>(value.clone())),
            );
        }

        #[test]
        fn valid_multi_linestring_roundtrips_across_widths(
            nested in valid_multi_linestring_strategy()
        ) {
            assert_across_widths(
                &nested,
                assert_multi_linestring_roundtrip::<u16>,
                |value| assert_multi_linestring_roundtrip::<u32>(&cast_multi_linestring::<u32>(value.clone())),
                |value| assert_multi_linestring_roundtrip::<u64>(&cast_multi_linestring::<u64>(value.clone())),
            );
        }

        #[test]
        fn valid_multi_surface_roundtrips_across_widths(
            nested in valid_multi_surface_strategy()
        ) {
            assert_across_widths(
                &nested,
                assert_multi_surface_roundtrip::<u16>,
                |value| assert_multi_surface_roundtrip::<u32>(&cast_multi_surface::<u32>(value.clone())),
                |value| assert_multi_surface_roundtrip::<u64>(&cast_multi_surface::<u64>(value.clone())),
            );
        }

        #[test]
        fn valid_solid_roundtrips_across_widths(nested in valid_solid_strategy()) {
            assert_across_widths(
                &nested,
                assert_solid_roundtrip::<u16>,
                |value| assert_solid_roundtrip::<u32>(&cast_solid::<u32>(value.clone())),
                |value| assert_solid_roundtrip::<u64>(&cast_solid::<u64>(value.clone())),
            );
        }

        #[test]
        fn valid_multisolid_roundtrips_across_widths(nested in valid_multisolid_strategy()) {
            assert_across_widths(
                &nested,
                assert_multisolid_roundtrip::<u16>,
                |value| assert_multisolid_roundtrip::<u32>(&cast_multisolid::<u32>(value.clone())),
                |value| assert_multisolid_roundtrip::<u64>(&cast_multisolid::<u64>(value.clone())),
            );
        }

        #[test]
        fn malformed_flattened_boundaries_are_rejected(boundary in malformed_boundary_strategy()) {
            assert_inconsistent_boundary_rejected(&boundary);
        }
    }
}
