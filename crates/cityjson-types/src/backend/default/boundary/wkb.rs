//! WKB export for flattened `CityJSON` boundaries.
//!
//! This module writes WKB directly from [`Boundary`] without building an intermediate geometry
//! object. That is possible because the flattened boundary layout is already close to WKB's nested
//! geometry layout: both describe the same hierarchy of points, lines, rings, and polygons, but
//! they count children differently.
//!
//! [`Boundary`] stores all vertex references in one flat `vertices` buffer and uses offset buffers
//! to mark where each higher-level component starts:
//!
//! ```text
//! vertices: [v0, v1, v2, v3, v4, v5, ...]
//! rings:    [0,      4,          ...]  offsets into vertices
//! surfaces: [0,      1,          ...]  offsets into rings
//! shells:   [0,                 ...]   offsets into surfaces
//! solids:   [0,                 ...]   offsets into shells
//! ```
//!
//! An offset buffer contains the start offset for each child. The end offset is either the next
//! offset in the same buffer or, for the last child, the length of the child buffer. For example,
//! `rings = [0, 4]` with `vertices.len() == 9` describes two rings: `vertices[0..4]` and
//! `vertices[4..9]`. This is the same convention used by Arrow-style offset arrays, except the
//! final child-buffer length is implicit instead of stored as an additional sentinel offset.
//!
//! WKB stores the same hierarchy inline in one byte buffer. There are no side offset buffers:
//! every geometry starts with its own byte order marker and type code, and every variable-length
//! container stores an explicit `u32` count immediately before its children. The corresponding
//! `MultiPolygonZ` layout is:
//!
//! ```text
//! bytes: [bo, type=MultiPolygonZ, polygon_count,
//!         bo, type=PolygonZ,      ring_count,
//!                                coordinate_count, x0, y0, z0, x1, y1, z1, ...,
//!                                coordinate_count, x0, y0, z0, x1, y1, z1, ...,
//!         bo, type=PolygonZ,      ring_count,
//!                                coordinate_count, x0, y0, z0, x1, y1, z1, ...,
//!         ...]
//! ```
//!
//! In that layout `bo` is the one-byte endian marker, each `type` is a `u32` geometry type code,
//! each `*_count` is a `u32`, and each coordinate component is an `f64`. Child boundaries are found
//! by reading a count and then consuming exactly that many inline child records. For example, where
//! [`Boundary`] represents two rings as `rings = [0, 4]` plus `vertices.len() == 9`, WKB represents
//! the same split as two inline ring records with coordinate counts `4` and `5`.
//!
//! Conversion therefore consists mostly of turning ranges from the boundary offset buffers into WKB
//! counts:
//!
//! - `MultiPoint`: `vertices.len()` becomes the `MultiPointZ` count; every referenced coordinate is
//!   emitted as a WKB `PointZ`.
//! - `MultiLineString`: `rings.len()` becomes the `MultiLineStringZ` count; each ring offset range
//!   becomes one `LineStringZ`, and the range length becomes that line string's coordinate count.
//! - `MultiSurface` and `CompositeSurface`: `surfaces.len()` becomes the `MultiPolygonZ` count; each
//!   surface offset range becomes one `PolygonZ`, and the number of rings in that range becomes the
//!   polygon ring count.
//! - `Solid`, `MultiSolid`, and `CompositeSolid`: standard WKB has no solid geometry type, so shells
//!   and solids are not encoded as containers. All surfaces are flattened and emitted as one
//!   `MultiPolygonZ`; the polygon count is still `surfaces.len()`.
//!
//! Coordinates are resolved lazily from the caller-provided vertex pool, so repeated vertex
//! references remain repeated in the WKB output. Ring closure, ring order, shell order, and surface
//! order are preserved exactly; the writer does not repair, close, orient, or deduplicate geometry.
//! Output is little-endian ISO SQL/MM WKB using the 3D type codes (`PointZ = 1001` through
//! `MultiPolygonZ = 1006`).

use super::{Boundary, BoundaryType};
use crate::cityjson::core::coordinate::RealWorldCoordinate;
use crate::cityjson::core::vertex::{VertexIndex, VertexRef};
use crate::cityjson::core::vertices::Vertices;
use crate::error;
use std::ops::Range;

const LITTLE_ENDIAN: u8 = 1;
const POINT_Z: u32 = 1_001;
const LINE_STRING_Z: u32 = 1_002;
const POLYGON_Z: u32 = 1_003;
const MULTI_POINT_Z: u32 = 1_004;
const MULTI_LINE_STRING_Z: u32 = 1_005;
const MULTI_POLYGON_Z: u32 = 1_006;
const COORDINATE_Z_BYTES: usize = 3 * std::mem::size_of::<f64>();
const WKB_HEADER_BYTES: usize = std::mem::size_of::<u8>() + std::mem::size_of::<u32>();
const WKB_COUNT_BYTES: usize = std::mem::size_of::<u32>();

impl<VR: VertexRef> Boundary<VR> {
    /// Converts this boundary to little-endian ISO WKB with XYZ coordinates.
    ///
    /// The boundary stores vertex indices, so the caller must provide the vertex pool used to
    /// resolve those indices to real-world coordinates. Surface-backed `CityJSON` boundaries are
    /// emitted as `MultiPolygonZ`; solids are flattened because standard WKB has no solid type.
    ///
    /// # Errors
    ///
    /// Returns [`error::Error::InvalidGeometry`] when the boundary is empty or its offset buffers
    /// are inconsistent. Returns [`error::Error::InvalidReference`] when a boundary vertex index is
    /// not present in `vertices`. Returns [`error::Error::IndexConversion`] when a WKB count cannot
    /// fit in `u32`.
    pub fn to_wkb(&self, vertices: &Vertices<VR, RealWorldCoordinate>) -> error::Result<Vec<u8>> {
        WkbWriter::new(self, vertices).write()
    }
}

struct WkbWriter<'a, VR: VertexRef> {
    boundary: &'a Boundary<VR>,
    vertices: &'a Vertices<VR, RealWorldCoordinate>,
    bytes: Vec<u8>,
}

impl<'a, VR: VertexRef> WkbWriter<'a, VR> {
    fn new(boundary: &'a Boundary<VR>, vertices: &'a Vertices<VR, RealWorldCoordinate>) -> Self {
        Self {
            boundary,
            vertices,
            bytes: Vec::with_capacity(estimated_wkb_size(boundary)),
        }
    }

    fn write(mut self) -> error::Result<Vec<u8>> {
        if !self.boundary.is_consistent() {
            return Err(error::Error::InvalidGeometry(
                "inconsistent boundary offsets".to_owned(),
            ));
        }

        match self.boundary.check_type() {
            BoundaryType::MultiPoint => self.write_multi_point()?,
            BoundaryType::MultiLineString => self.write_multi_line_string()?,
            BoundaryType::MultiOrCompositeSurface
            | BoundaryType::Solid
            | BoundaryType::MultiOrCompositeSolid => {
                self.write_multi_polygon()?;
            }
            BoundaryType::None => {
                return Err(error::Error::InvalidGeometry(
                    "cannot write an empty boundary as WKB".to_owned(),
                ));
            }
        }

        Ok(self.bytes)
    }

    fn write_multi_point(&mut self) -> error::Result<()> {
        self.write_header(MULTI_POINT_Z);
        self.write_count(self.boundary.vertices.len())?;

        for &vertex_index in &self.boundary.vertices {
            self.write_header(POINT_Z);
            self.write_coordinate(vertex_index)?;
        }

        Ok(())
    }

    fn write_multi_line_string(&mut self) -> error::Result<()> {
        self.write_header(MULTI_LINE_STRING_Z);
        self.write_count(self.boundary.rings.len())?;

        for ring_index in 0..self.boundary.rings.len() {
            let vertex_range = child_range(
                &self.boundary.rings,
                self.boundary.vertices.len(),
                ring_index,
            )?;

            self.write_header(LINE_STRING_Z);
            self.write_count(vertex_range.len())?;
            for &vertex_index in &self.boundary.vertices[vertex_range] {
                self.write_coordinate(vertex_index)?;
            }
        }

        Ok(())
    }

    fn write_multi_polygon(&mut self) -> error::Result<()> {
        self.write_header(MULTI_POLYGON_Z);
        self.write_count(self.boundary.surfaces.len())?;

        for surface_index in 0..self.boundary.surfaces.len() {
            self.write_polygon(surface_index)?;
        }

        Ok(())
    }

    fn write_polygon(&mut self, surface_index: usize) -> error::Result<()> {
        let ring_range = child_range(
            &self.boundary.surfaces,
            self.boundary.rings.len(),
            surface_index,
        )?;

        self.write_header(POLYGON_Z);
        self.write_count(ring_range.len())?;

        for ring_index in ring_range {
            let vertex_range = child_range(
                &self.boundary.rings,
                self.boundary.vertices.len(),
                ring_index,
            )?;
            self.write_count(vertex_range.len())?;
            for &vertex_index in &self.boundary.vertices[vertex_range] {
                self.write_coordinate(vertex_index)?;
            }
        }

        Ok(())
    }

    fn write_coordinate(&mut self, vertex_index: VertexIndex<VR>) -> error::Result<()> {
        let index = vertex_index.try_to_usize()?;
        let coordinate =
            self.vertices
                .as_slice()
                .get(index)
                .ok_or_else(|| error::Error::InvalidReference {
                    element_type: "vertex".to_owned(),
                    index,
                    max_index: self.vertices.len().saturating_sub(1),
                })?;

        self.write_f64(coordinate.x());
        self.write_f64(coordinate.y());
        self.write_f64(coordinate.z());
        Ok(())
    }

    fn write_header(&mut self, geometry_type: u32) {
        self.bytes.push(LITTLE_ENDIAN);
        self.bytes.extend_from_slice(&geometry_type.to_le_bytes());
    }

    fn write_count(&mut self, count: usize) -> error::Result<()> {
        self.bytes
            .extend_from_slice(&count_to_u32(count)?.to_le_bytes());
        Ok(())
    }

    fn write_f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn child_range<VR: VertexRef>(
    offsets: &[VertexIndex<VR>],
    child_len: usize,
    index: usize,
) -> error::Result<Range<usize>> {
    let start = offsets[index].try_to_usize()?;
    let end = offsets
        .get(index + 1)
        .map(VertexIndex::try_to_usize)
        .transpose()?
        .unwrap_or(child_len);
    Ok(start..end)
}

fn count_to_u32(count: usize) -> error::Result<u32> {
    u32::try_from(count).map_err(|_| error::Error::IndexConversion {
        source_type: "usize".to_owned(),
        target_type: "u32".to_owned(),
        value: count.to_string(),
    })
}

fn estimated_wkb_size<VR: VertexRef>(boundary: &Boundary<VR>) -> usize {
    match boundary.check_type() {
        BoundaryType::MultiPoint => {
            WKB_HEADER_BYTES
                + WKB_COUNT_BYTES
                + boundary.vertices.len() * (WKB_HEADER_BYTES + COORDINATE_Z_BYTES)
        }
        BoundaryType::MultiLineString => {
            WKB_HEADER_BYTES
                + WKB_COUNT_BYTES
                + boundary.rings.len() * (WKB_HEADER_BYTES + WKB_COUNT_BYTES)
                + boundary.vertices.len() * COORDINATE_Z_BYTES
        }
        BoundaryType::MultiOrCompositeSurface
        | BoundaryType::Solid
        | BoundaryType::MultiOrCompositeSolid => {
            WKB_HEADER_BYTES
                + WKB_COUNT_BYTES
                + boundary.surfaces.len() * (WKB_HEADER_BYTES + WKB_COUNT_BYTES)
                + boundary.rings.len() * WKB_COUNT_BYTES
                + boundary.vertices.len() * COORDINATE_Z_BYTES
        }
        BoundaryType::None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cityjson::core::boundary::nested::{
        BoundaryNestedMultiLineString32, BoundaryNestedMultiOrCompositeSolid32,
        BoundaryNestedMultiOrCompositeSurface32, BoundaryNestedMultiPoint32, BoundaryNestedSolid32,
    };

    fn vertices() -> Vertices<u32, RealWorldCoordinate> {
        Vertices::from(vec![
            RealWorldCoordinate::new(0.0, 0.0, 0.0),
            RealWorldCoordinate::new(1.0, 0.0, 0.0),
            RealWorldCoordinate::new(1.0, 1.0, 0.0),
            RealWorldCoordinate::new(0.0, 1.0, 0.0),
            RealWorldCoordinate::new(0.25, 0.25, 1.0),
            RealWorldCoordinate::new(0.75, 0.25, 1.0),
            RealWorldCoordinate::new(0.75, 0.75, 1.0),
            RealWorldCoordinate::new(0.25, 0.75, 1.0),
        ])
    }

    fn read_u8(bytes: &[u8], offset: &mut usize) -> u8 {
        let value = bytes[*offset];
        *offset += 1;
        value
    }

    fn read_u32(bytes: &[u8], offset: &mut usize) -> u32 {
        let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        value
    }

    fn read_f64(bytes: &[u8], offset: &mut usize) -> f64 {
        let value = f64::from_le_bytes(bytes[*offset..*offset + 8].try_into().unwrap());
        *offset += 8;
        value
    }

    fn assert_header(bytes: &[u8], offset: &mut usize, geometry_type: u32) {
        assert_eq!(read_u8(bytes, offset), LITTLE_ENDIAN);
        assert_eq!(read_u32(bytes, offset), geometry_type);
    }

    fn assert_coordinate(bytes: &[u8], offset: &mut usize, expected: [f64; 3]) {
        assert_eq!(read_f64(bytes, offset).to_bits(), expected[0].to_bits());
        assert_eq!(read_f64(bytes, offset).to_bits(), expected[1].to_bits());
        assert_eq!(read_f64(bytes, offset).to_bits(), expected[2].to_bits());
    }

    #[test]
    fn writes_multi_point_z() {
        let nested: BoundaryNestedMultiPoint32 = vec![0, 2];
        let boundary: Boundary<u32> = nested.into();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_POINT_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);
        assert_header(&wkb, &mut offset, POINT_Z);
        assert_coordinate(&wkb, &mut offset, [0.0, 0.0, 0.0]);
        assert_header(&wkb, &mut offset, POINT_Z);
        assert_coordinate(&wkb, &mut offset, [1.0, 1.0, 0.0]);
        assert_eq!(offset, wkb.len());
    }

    #[test]
    fn writes_multi_line_string_z() {
        let nested: BoundaryNestedMultiLineString32 = vec![vec![0, 1], vec![2, 3, 0]];
        let boundary: Boundary<u32> = nested.try_into().unwrap();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_LINE_STRING_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);
        assert_header(&wkb, &mut offset, LINE_STRING_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);
        assert_coordinate(&wkb, &mut offset, [0.0, 0.0, 0.0]);
        assert_coordinate(&wkb, &mut offset, [1.0, 0.0, 0.0]);
        assert_header(&wkb, &mut offset, LINE_STRING_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 3);
        assert_coordinate(&wkb, &mut offset, [1.0, 1.0, 0.0]);
        assert_coordinate(&wkb, &mut offset, [0.0, 1.0, 0.0]);
        assert_coordinate(&wkb, &mut offset, [0.0, 0.0, 0.0]);
        assert_eq!(offset, wkb.len());
    }

    #[test]
    fn writes_multi_polygon_z_with_hole() {
        let nested: BoundaryNestedMultiOrCompositeSurface32 =
            vec![vec![vec![0, 1, 2, 3, 0], vec![4, 5, 6, 7, 4]]];
        let boundary: Boundary<u32> = nested.try_into().unwrap();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 1);
        assert_header(&wkb, &mut offset, POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);
        assert_eq!(read_u32(&wkb, &mut offset), 5);
        assert_coordinate(&wkb, &mut offset, [0.0, 0.0, 0.0]);
        offset += 4 * COORDINATE_Z_BYTES;
        assert_eq!(read_u32(&wkb, &mut offset), 5);
        assert_coordinate(&wkb, &mut offset, [0.25, 0.25, 1.0]);
    }

    #[test]
    fn flattens_solids_to_multi_polygon_z() {
        let solid: BoundaryNestedSolid32 =
            vec![vec![vec![vec![0, 1, 2, 3, 0]], vec![vec![4, 5, 6, 7, 4]]]];
        let boundary: Boundary<u32> = solid.try_into().unwrap();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);
    }

    #[test]
    fn flattens_multi_solids_to_multi_polygon_z() {
        let multi_solid: BoundaryNestedMultiOrCompositeSolid32 = vec![
            vec![vec![vec![vec![0, 1, 2, 3, 0]]]],
            vec![vec![vec![vec![4, 5, 6, 7, 4]]]],
        ];
        let boundary: Boundary<u32> = multi_solid.try_into().unwrap();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);
    }

    #[test]
    fn rejects_empty_boundary() {
        let boundary = Boundary::<u32>::new();
        let error = boundary.to_wkb(&vertices()).unwrap_err();

        assert!(matches!(error, error::Error::InvalidGeometry(_)));
    }

    #[test]
    fn rejects_inconsistent_offsets() {
        let mut boundary = Boundary::<u32>::new();
        boundary.vertices = vec![VertexIndex::new(0), VertexIndex::new(1)];
        boundary.rings = vec![VertexIndex::new(3)];
        let error = boundary.to_wkb(&vertices()).unwrap_err();

        assert!(matches!(error, error::Error::InvalidGeometry(_)));
    }

    #[test]
    fn rejects_missing_vertex_reference() {
        let nested: BoundaryNestedMultiPoint32 = vec![8];
        let boundary: Boundary<u32> = nested.into();
        let error = boundary.to_wkb(&vertices()).unwrap_err();

        assert!(matches!(
            error,
            error::Error::InvalidReference { index: 8, .. }
        ));
    }
}
