//! Tests for the public vertices API.

const FLOAT_EPSILON: f64 = 1.0e-12;

fn assert_f64_eq(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= FLOAT_EPSILON,
        "expected {expected}, got {actual} (epsilon {FLOAT_EPSILON})"
    );
}

fn assert_f64_slice_eq(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual_item, expected_item) in actual.iter().zip(expected.iter()) {
        assert_f64_eq(*actual_item, *expected_item);
    }
}

mod basic {
    use super::{assert_f64_eq, assert_f64_slice_eq};
    use cityjson_types::CityModelType;
    use cityjson_types::v2_0::*;

    #[test]
    fn new_starts_empty() {
        let vertices = GeometryVertices16::new();

        assert!(vertices.is_empty());
        assert_eq!(vertices.len(), 0);
    }

    #[test]
    fn with_capacity_starts_empty() {
        let vertices = GeometryVertices16::with_capacity(4);

        assert!(vertices.is_empty());
        assert_eq!(vertices.len(), 0);
    }

    #[test]
    fn reserve_allows_future_insertions() {
        let mut vertices = GeometryVertices16::new();

        vertices.reserve(2).unwrap();

        let index = vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();

        assert_eq!(index, VertexIndex16::new(0));
        assert_eq!(vertices.len(), 1);
    }

    #[test]
    fn push_returns_index_of_inserted_coordinate() {
        let mut vertices = GeometryVertices32::new();

        let index = vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();

        assert_eq!(index, VertexIndex32::new(0));
        assert_eq!(vertices.len(), 1);
    }

    #[test]
    fn get_returns_inserted_coordinate() {
        let mut vertices = GeometryVertices16::new();
        let index = vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();

        let coordinate = vertices.get(index).unwrap();

        assert_f64_slice_eq(&coordinate.to_array(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn len_tracks_number_of_coordinates() {
        let mut vertices = GeometryVertices16::new();
        vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();
        vertices
            .push(RealWorldCoordinate::new(4.0, 5.0, 6.0))
            .unwrap();

        assert_eq!(vertices.len(), 2);
    }

    #[test]
    fn is_empty_reflects_insertions() {
        let mut vertices = GeometryVertices16::new();
        assert!(vertices.is_empty());

        vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();

        assert!(!vertices.is_empty());
    }

    #[test]
    fn as_slice_exposes_coordinates_in_order() {
        let mut vertices = GeometryVertices32::new();
        vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();
        vertices
            .push(RealWorldCoordinate::new(4.0, 5.0, 6.0))
            .unwrap();

        let coordinates = vertices.as_slice();

        assert_eq!(coordinates.len(), 2);
        assert_f64_eq(coordinates[0].x(), 1.0);
        assert_f64_eq(coordinates[1].x(), 4.0);
    }

    #[test]
    fn as_mut_slice_allows_in_place_coordinate_updates() {
        let mut vertices = GeometryVertices32::new();
        vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();

        vertices.as_mut_slice()[0] = RealWorldCoordinate::new(4.0, 5.0, 6.0);

        assert_f64_slice_eq(&vertices.as_slice()[0].to_array(), &[4.0, 5.0, 6.0]);
    }

    #[test]
    fn clear_removes_all_coordinates() {
        let mut vertices = GeometryVertices32::new();
        vertices
            .push(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();
        vertices
            .push(RealWorldCoordinate::new(4.0, 5.0, 6.0))
            .unwrap();

        vertices.clear();

        assert!(vertices.is_empty());
        assert_eq!(vertices.len(), 0);
    }

    #[test]
    fn default_creates_empty_collection() {
        let vertices: GeometryVertices32 = Vertices::default();

        assert!(vertices.is_empty());
        assert_eq!(vertices.len(), 0);
    }

    #[test]
    fn from_vec_populates_collection() {
        let coordinates = vec![
            RealWorldCoordinate::new(1.0, 2.0, 3.0),
            RealWorldCoordinate::new(4.0, 5.0, 6.0),
        ];

        let vertices = GeometryVertices32::from(coordinates);

        assert_eq!(vertices.len(), 2);
        assert_f64_slice_eq(&vertices.as_slice()[1].to_array(), &[4.0, 5.0, 6.0]);
    }

    #[test]
    fn from_slice_copies_coordinates() {
        let coordinates = [
            RealWorldCoordinate::new(1.0, 2.0, 3.0),
            RealWorldCoordinate::new(4.0, 5.0, 6.0),
        ];

        let vertices = GeometryVertices32::from(&coordinates[..]);

        assert_eq!(vertices.len(), 2);
        assert_f64_slice_eq(&vertices.as_slice()[0].to_array(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn extend_from_slice_empty_returns_empty_range() {
        let mut vertices = GeometryVertices32::new();
        let range = vertices.extend_from_slice(&[]).unwrap();

        assert_eq!(range.start, VertexIndex32::new(0));
        assert_eq!(range.end, VertexIndex32::new(0));
        assert!(vertices.is_empty());
    }

    #[test]
    fn extend_from_slice_single_vertex_returns_inserted_range() {
        let mut vertices = GeometryVertices32::new();
        let input = [RealWorldCoordinate::new(1.0, 2.0, 3.0)];

        let range = vertices.extend_from_slice(&input).unwrap();

        assert_eq!(range.start, VertexIndex32::new(0));
        assert_eq!(range.end, VertexIndex32::new(1));
        assert_eq!(vertices.len(), 1);
        assert_f64_slice_eq(&vertices.as_slice()[0].to_array(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn extend_from_slice_multiple_vertices_preserves_order_and_continuity() {
        let mut vertices = GeometryVertices32::new();
        vertices
            .push(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap();
        let input = [
            RealWorldCoordinate::new(1.0, 2.0, 3.0),
            RealWorldCoordinate::new(4.0, 5.0, 6.0),
            RealWorldCoordinate::new(7.0, 8.0, 9.0),
        ];

        let range = vertices.extend_from_slice(&input).unwrap();

        assert_eq!(range.start, VertexIndex32::new(1));
        assert_eq!(range.end, VertexIndex32::new(4));
        assert_eq!(vertices.len(), 4);
        assert_f64_eq(vertices.as_slice()[1].x(), 1.0);
        assert_f64_eq(vertices.as_slice()[2].x(), 4.0);
        assert_f64_eq(vertices.as_slice()[3].x(), 7.0);
    }

    #[test]
    fn citymodel_add_vertices_returns_contiguous_range() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model
            .add_vertex(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap();

        let input = [
            RealWorldCoordinate::new(1.0, 0.0, 0.0),
            RealWorldCoordinate::new(2.0, 0.0, 0.0),
        ];
        let range = model.add_vertices(&input).unwrap();

        assert_eq!(range.start, VertexIndex32::new(1));
        assert_eq!(range.end, VertexIndex32::new(3));
        assert_f64_eq(model.get_vertex(VertexIndex32::new(1)).unwrap().x(), 1.0);
        assert_f64_eq(model.get_vertex(VertexIndex32::new(2)).unwrap().x(), 2.0);
    }

    #[test]
    fn citymodel_exposes_mutable_vertex_pool() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model
            .add_vertex(RealWorldCoordinate::new(1.0, 2.0, 3.0))
            .unwrap();

        model.vertices_mut().as_mut_slice()[0] = RealWorldCoordinate::new(7.0, 8.0, 9.0);

        assert_f64_slice_eq(&model.vertices().as_slice()[0].to_array(), &[7.0, 8.0, 9.0]);
    }
}

mod edge_cases {
    use cityjson_types::error::Error;
    use cityjson_types::v2_0::*;

    #[test]
    fn get_returns_none_for_missing_index() {
        let vertices = GeometryVertices16::new();

        assert!(vertices.get(VertexIndex16::new(0)).is_none());
    }

    #[test]
    fn reserve_rejects_capacity_past_index_limit() {
        let mut vertices = GeometryVertices16::new();

        let error = vertices.reserve(usize::from(u16::MAX) + 1).unwrap_err();

        assert!(matches!(
            error,
            Error::VerticesContainerFull {
                attempted: 1,
                maximum
            } if maximum == usize::from(u16::MAX)
        ));
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K push loop too slow under Miri
    fn push_rejects_more_than_the_index_type_can_store() {
        let mut vertices = GeometryVertices16::new();

        for i in 0..5 {
            vertices
                .push(RealWorldCoordinate::new(f64::from(i), 0.0, 0.0))
                .unwrap();
        }

        for _ in 5..usize::from(u16::MAX) {
            vertices
                .push(RealWorldCoordinate::new(0.0, 0.0, 0.0))
                .unwrap();
        }

        let error = vertices
            .push(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap_err();

        assert!(matches!(
            error,
            Error::VerticesContainerFull {
                attempted,
                maximum
            } if attempted == usize::from(u16::MAX) + 1 && maximum == usize::from(u16::MAX)
        ));
    }

    #[test]
    #[cfg_attr(miri, ignore)] // 65K+ allocation too slow under Miri
    fn extend_from_slice_rejects_more_than_the_index_type_can_store() {
        let mut vertices = GeometryVertices16::new();
        let batch = vec![RealWorldCoordinate::new(0.0, 0.0, 0.0); usize::from(u16::MAX) + 1];

        let error = vertices.extend_from_slice(&batch).unwrap_err();

        assert!(matches!(
            error,
            Error::VerticesContainerFull {
                attempted,
                maximum
            } if attempted == usize::from(u16::MAX) + 1 && maximum == usize::from(u16::MAX)
        ));
    }
}
