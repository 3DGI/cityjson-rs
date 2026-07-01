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
//! references remain repeated in the WKB output. Ring order, shell order, and surface order are
//! preserved exactly. Polygon rings are written closed, as required by WKB, by repeating the first
//! coordinate at the end when the internal `CityJSON` ring is open. Output is little-endian ISO
//! SQL/MM WKB using the 3D type codes (`PointZ = 1001` through `MultiPolygonZ = 1006`).

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
    /// not present in `vertices`. Returns [`error::Error::InvalidRing`] when a polygon ring has too
    /// few coordinates for WKB. Returns [`error::Error::IndexConversion`] when a WKB count cannot fit
    /// in `u32`.
    pub fn to_wkb(&self, vertices: &Vertices<VR, RealWorldCoordinate>) -> error::Result<Vec<u8>> {
        WkbWriter::new(self, vertices).write()
    }

    /// Parses a little-endian ISO WKB multi-geometry into a boundary and vertex pool.
    ///
    /// Only `MultiPointZ`, `MultiLineStringZ`, and `MultiPolygonZ` are accepted. Polygon rings are
    /// required to be closed in WKB and are stored open in the returned `CityJSON` boundary by
    /// dropping the duplicated closing coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`error::Error::InvalidGeometry`] for unsupported WKB variants, big-endian input,
    /// EWKB flags, malformed or truncated input, wrong child geometry types, trailing bytes, and
    /// empty top-level multi-geometries. Returns [`error::Error::InvalidRing`] for unclosed or
    /// too-short polygon rings. Returns index/container errors when the parsed coordinate count
    /// cannot be represented by `VR`.
    pub fn from_wkb(bytes: &[u8]) -> error::Result<(Self, Vertices<VR, RealWorldCoordinate>)> {
        WkbReader::new(bytes).read()
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

        for vertex_index in &self.boundary.vertices {
            self.write_header(POINT_Z);
            self.write_coordinate(*vertex_index)?;
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
            for vertex_index in &self.boundary.vertices[vertex_range] {
                self.write_coordinate(*vertex_index)?;
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
            let coordinate_count = self.closed_ring_coordinate_count(vertex_range.clone())?;
            self.write_count(coordinate_count)?;
            for &vertex_index in &self.boundary.vertices[vertex_range.clone()] {
                self.write_coordinate(vertex_index)?;
            }
            if !self.ring_is_already_closed(vertex_range.clone()) {
                let first_vertex_index = self.boundary.vertices[vertex_range.start];
                self.write_coordinate(first_vertex_index)?;
            }
        }

        Ok(())
    }

    fn closed_ring_coordinate_count(&self, vertex_range: Range<usize>) -> error::Result<usize> {
        if vertex_range.len() < 3 {
            return Err(error::Error::InvalidRing {
                reason: "polygon ring must contain at least three vertices".to_owned(),
                vertex_count: vertex_range.len(),
            });
        }

        let coordinate_count = if self.ring_is_already_closed(vertex_range.clone()) {
            vertex_range.len()
        } else {
            vertex_range.len() + 1
        };

        if coordinate_count < 4 {
            return Err(error::Error::InvalidRing {
                reason: "closed WKB polygon ring must contain at least four coordinates".to_owned(),
                vertex_count: coordinate_count,
            });
        }

        Ok(coordinate_count)
    }

    fn ring_is_already_closed(&self, vertex_range: Range<usize>) -> bool {
        self.boundary.vertices[vertex_range.start] == self.boundary.vertices[vertex_range.end - 1]
    }

    fn write_coordinate(&mut self, vertex_index: VertexIndex<VR>) -> error::Result<()> {
        let coordinate =
            self.vertices
                .get(vertex_index)
                .ok_or_else(|| error::Error::InvalidReference {
                    element_type: "vertex".to_owned(),
                    index: vertex_index.to_usize(),
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

struct WkbReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> WkbReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read<VR: VertexRef>(
        mut self,
    ) -> error::Result<(Boundary<VR>, Vertices<VR, RealWorldCoordinate>)> {
        let geometry_type = self.read_header()?;
        let result = match geometry_type {
            MULTI_POINT_Z => self.read_multi_point(),
            MULTI_LINE_STRING_Z => self.read_multi_line_string(),
            MULTI_POLYGON_Z => self.read_multi_polygon(),
            POINT_Z | LINE_STRING_Z | POLYGON_Z => Err(invalid_wkb(
                "top-level singular geometries are not supported",
            )),
            _ => Err(unsupported_wkb_type(geometry_type)),
        }?;

        if self.offset != self.bytes.len() {
            return Err(invalid_wkb("trailing bytes after geometry"));
        }

        Ok(result)
    }

    fn read_multi_point<VR: VertexRef>(
        &mut self,
    ) -> error::Result<(Boundary<VR>, Vertices<VR, RealWorldCoordinate>)> {
        let point_count = self.read_non_empty_count("MultiPointZ")?;
        let mut boundary = Boundary::new();
        let mut vertices = Vertices::new();

        for _ in 0..point_count {
            self.expect_header(POINT_Z, "MultiPointZ child")?;
            let coordinate = self.read_coordinate()?;
            let vertex_index = vertices.push(coordinate)?;
            boundary.vertices.push(vertex_index);
        }

        Ok((boundary, vertices))
    }

    fn read_multi_line_string<VR: VertexRef>(
        &mut self,
    ) -> error::Result<(Boundary<VR>, Vertices<VR, RealWorldCoordinate>)> {
        let line_string_count = self.read_non_empty_count("MultiLineStringZ")?;
        let mut boundary = Boundary::new();
        let mut vertices = Vertices::new();

        for _ in 0..line_string_count {
            self.expect_header(LINE_STRING_Z, "MultiLineStringZ child")?;
            push_offset(&mut boundary.rings, boundary.vertices.len())?;

            let coordinate_count = self.read_count()?;
            self.ensure_remaining_coordinates(coordinate_count)?;
            for _ in 0..coordinate_count {
                let coordinate = self.read_coordinate()?;
                let vertex_index = vertices.push(coordinate)?;
                boundary.vertices.push(vertex_index);
            }
        }

        Ok((boundary, vertices))
    }

    fn read_multi_polygon<VR: VertexRef>(
        &mut self,
    ) -> error::Result<(Boundary<VR>, Vertices<VR, RealWorldCoordinate>)> {
        let polygon_count = self.read_non_empty_count("MultiPolygonZ")?;
        let mut boundary = Boundary::new();
        let mut vertices = Vertices::new();

        for _ in 0..polygon_count {
            self.expect_header(POLYGON_Z, "MultiPolygonZ child")?;
            push_offset(&mut boundary.surfaces, boundary.rings.len())?;

            let ring_count = self.read_count()?;
            for _ in 0..ring_count {
                push_offset(&mut boundary.rings, boundary.vertices.len())?;
                self.read_polygon_ring(&mut boundary, &mut vertices)?;
            }
        }

        Ok((boundary, vertices))
    }

    fn read_polygon_ring<VR: VertexRef>(
        &mut self,
        boundary: &mut Boundary<VR>,
        vertices: &mut Vertices<VR, RealWorldCoordinate>,
    ) -> error::Result<()> {
        let coordinate_count = self.read_count()?;
        if coordinate_count < 4 {
            return Err(error::Error::InvalidRing {
                reason: "WKB polygon ring must contain at least four coordinates".to_owned(),
                vertex_count: coordinate_count,
            });
        }

        self.ensure_remaining_coordinates(coordinate_count)?;
        let first_coordinate = self.read_coordinate()?;
        let first_vertex_index = vertices.push(first_coordinate)?;
        boundary.vertices.push(first_vertex_index);

        for _ in 1..coordinate_count - 1 {
            let coordinate = self.read_coordinate()?;
            let vertex_index = vertices.push(coordinate)?;
            boundary.vertices.push(vertex_index);
        }

        let closing_coordinate = self.read_coordinate()?;
        if closing_coordinate != first_coordinate {
            return Err(error::Error::InvalidRing {
                reason: "WKB polygon ring is not closed".to_owned(),
                vertex_count: coordinate_count,
            });
        }

        Ok(())
    }

    fn read_header(&mut self) -> error::Result<u32> {
        let byte_order = self.read_u8()?;
        if byte_order != LITTLE_ENDIAN {
            return Err(invalid_wkb("only little-endian byte order is supported"));
        }

        let geometry_type = self.read_u32()?;
        if has_ewkb_flags(geometry_type) {
            return Err(invalid_wkb("EWKB type flags are not supported by ISO WKB"));
        }

        Ok(geometry_type)
    }

    fn expect_header(&mut self, expected: u32, context: &str) -> error::Result<()> {
        let found = self.read_header()?;
        if found != expected {
            return Err(invalid_wkb(format!(
                "{context} has type {found}, expected {expected}"
            )));
        }
        Ok(())
    }

    fn read_non_empty_count(&mut self, geometry_name: &str) -> error::Result<usize> {
        let count = self.read_count()?;
        if count == 0 {
            return Err(invalid_wkb(format!(
                "{geometry_name} must contain at least one child geometry"
            )));
        }
        Ok(count)
    }

    fn read_count(&mut self) -> error::Result<usize> {
        u32_to_usize(self.read_u32()?)
    }

    fn read_coordinate(&mut self) -> error::Result<RealWorldCoordinate> {
        let x = self.read_f64()?;
        let y = self.read_f64()?;
        let z = self.read_f64()?;
        Ok(RealWorldCoordinate::new(x, y, z))
    }

    fn ensure_remaining_coordinates(&self, coordinate_count: usize) -> error::Result<()> {
        let Some(byte_count) = coordinate_count.checked_mul(COORDINATE_Z_BYTES) else {
            return Err(invalid_wkb("coordinate byte count overflows usize"));
        };

        if self.bytes.len().saturating_sub(self.offset) < byte_count {
            return Err(invalid_wkb("truncated coordinate sequence"));
        }

        Ok(())
    }

    fn read_u8(&mut self) -> error::Result<u8> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or_else(|| invalid_wkb("truncated byte-order marker"))?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_u32(&mut self) -> error::Result<u32> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_f64(&mut self) -> error::Result<f64> {
        Ok(f64::from_le_bytes(self.read_array()?))
    }

    fn read_array<const N: usize>(&mut self) -> error::Result<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or_else(|| invalid_wkb("byte offset overflows usize"))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid_wkb("truncated WKB payload"))?;

        let mut array = [0; N];
        array.copy_from_slice(slice);
        self.offset = end;
        Ok(array)
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

fn u32_to_usize(count: u32) -> error::Result<usize> {
    usize::try_from(count).map_err(|_| error::Error::IndexConversion {
        source_type: "u32".to_owned(),
        target_type: "usize".to_owned(),
        value: count.to_string(),
    })
}

fn push_offset<VR: VertexRef>(
    offsets: &mut Vec<VertexIndex<VR>>,
    offset: usize,
) -> error::Result<()> {
    offsets.push(VertexIndex::try_from(offset)?);
    Ok(())
}

fn has_ewkb_flags(geometry_type: u32) -> bool {
    geometry_type & 0xE000_0000 != 0
}

fn unsupported_wkb_type(geometry_type: u32) -> error::Error {
    invalid_wkb(format!("unsupported ISO WKB geometry type {geometry_type}"))
}

fn invalid_wkb(message: impl Into<String>) -> error::Error {
    error::Error::InvalidGeometry(format!("invalid WKB: {}", message.into()))
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
                + (boundary.vertices.len() + boundary.rings.len()) * COORDINATE_Z_BYTES
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

    fn push_header(bytes: &mut Vec<u8>, geometry_type: u32) {
        bytes.push(LITTLE_ENDIAN);
        bytes.extend_from_slice(&geometry_type.to_le_bytes());
    }

    fn push_count(bytes: &mut Vec<u8>, count: u32) {
        bytes.extend_from_slice(&count.to_le_bytes());
    }

    fn push_coordinate(bytes: &mut Vec<u8>, coordinate: [f64; 3]) {
        for value in coordinate {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }

    fn raw_values(indices: &[VertexIndex<u32>]) -> Vec<u32> {
        indices.iter().map(VertexIndex::value).collect()
    }

    fn assert_invalid_geometry(bytes: &[u8]) {
        let error = Boundary::<u32>::from_wkb(bytes).unwrap_err();
        assert!(matches!(error, error::Error::InvalidGeometry(_)));
    }

    fn assert_invalid_ring(bytes: &[u8]) {
        let error = Boundary::<u32>::from_wkb(bytes).unwrap_err();
        assert!(matches!(error, error::Error::InvalidRing { .. }));
    }

    fn closed_square_wkb() -> Vec<u8> {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POLYGON_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POLYGON_Z);
        push_count(&mut wkb, 1);
        push_count(&mut wkb, 5);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);
        push_coordinate(&mut wkb, [1.0, 0.0, 0.0]);
        push_coordinate(&mut wkb, [1.0, 1.0, 0.0]);
        push_coordinate(&mut wkb, [0.0, 1.0, 0.0]);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);
        wkb
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
    fn writes_multi_polygon_z_with_closed_wkb_rings() {
        let nested: BoundaryNestedMultiOrCompositeSurface32 =
            vec![vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]]];
        let boundary: Boundary<u32> = nested.try_into().unwrap();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 1);
        assert_header(&wkb, &mut offset, POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);

        assert_eq!(read_u32(&wkb, &mut offset), 5);
        assert_coordinate(&wkb, &mut offset, [0.0, 0.0, 0.0]);
        assert_coordinate(&wkb, &mut offset, [1.0, 0.0, 0.0]);
        assert_coordinate(&wkb, &mut offset, [1.0, 1.0, 0.0]);
        assert_coordinate(&wkb, &mut offset, [0.0, 1.0, 0.0]);
        assert_coordinate(&wkb, &mut offset, [0.0, 0.0, 0.0]);

        assert_eq!(read_u32(&wkb, &mut offset), 5);
        assert_coordinate(&wkb, &mut offset, [0.25, 0.25, 1.0]);
        assert_coordinate(&wkb, &mut offset, [0.75, 0.25, 1.0]);
        assert_coordinate(&wkb, &mut offset, [0.75, 0.75, 1.0]);
        assert_coordinate(&wkb, &mut offset, [0.25, 0.75, 1.0]);
        assert_coordinate(&wkb, &mut offset, [0.25, 0.25, 1.0]);
        assert_eq!(offset, wkb.len());
    }

    #[test]
    fn does_not_double_close_legacy_closed_internal_ring() {
        let nested: BoundaryNestedMultiOrCompositeSurface32 = vec![vec![vec![0, 1, 2, 3, 0]]];
        let boundary: Boundary<u32> = nested.try_into().unwrap();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 1);
        assert_header(&wkb, &mut offset, POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 1);
        assert_eq!(read_u32(&wkb, &mut offset), 5);
    }

    #[test]
    fn flattens_solids_to_multi_polygon_z() {
        let solid: BoundaryNestedSolid32 =
            vec![vec![vec![vec![0, 1, 2, 3]], vec![vec![4, 5, 6, 7]]]];
        let boundary: Boundary<u32> = solid.try_into().unwrap();
        let wkb = boundary.to_wkb(&vertices()).unwrap();
        let mut offset = 0;

        assert_header(&wkb, &mut offset, MULTI_POLYGON_Z);
        assert_eq!(read_u32(&wkb, &mut offset), 2);
        assert_header(&wkb, &mut offset, POLYGON_Z);
        offset += WKB_COUNT_BYTES;
        offset += WKB_COUNT_BYTES + 5 * COORDINATE_Z_BYTES;
        assert_header(&wkb, &mut offset, POLYGON_Z);
    }

    #[test]
    fn flattens_multi_solids_to_multi_polygon_z() {
        let multi_solid: BoundaryNestedMultiOrCompositeSolid32 = vec![
            vec![vec![vec![vec![0, 1, 2, 3]]]],
            vec![vec![vec![vec![4, 5, 6, 7]]]],
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

    #[test]
    fn rejects_polygon_ring_with_too_few_internal_vertices() {
        let boundary = Boundary::from_parts(
            vec![VertexIndex::new(0), VertexIndex::new(1)],
            vec![VertexIndex::new(0)],
            vec![VertexIndex::new(0)],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let error = boundary.to_wkb(&vertices()).unwrap_err();

        assert!(matches!(error, error::Error::InvalidRing { .. }));
    }

    #[test]
    fn parses_multi_point_z() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POINT_Z);
        push_count(&mut wkb, 2);
        push_header(&mut wkb, POINT_Z);
        push_coordinate(&mut wkb, [0.0, 1.0, 2.0]);
        push_header(&mut wkb, POINT_Z);
        push_coordinate(&mut wkb, [3.0, 4.0, 5.0]);

        let (boundary, parsed_vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();

        assert_eq!(boundary.check_type(), BoundaryType::MultiPoint);
        assert_eq!(raw_values(&boundary.vertices), vec![0, 1]);
        assert_eq!(
            parsed_vertices.as_slice()[0],
            RealWorldCoordinate::new(0.0, 1.0, 2.0)
        );
        assert_eq!(
            parsed_vertices.as_slice()[1],
            RealWorldCoordinate::new(3.0, 4.0, 5.0)
        );
    }

    #[test]
    fn parses_multi_line_string_z() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_LINE_STRING_Z);
        push_count(&mut wkb, 2);
        push_header(&mut wkb, LINE_STRING_Z);
        push_count(&mut wkb, 2);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);
        push_coordinate(&mut wkb, [1.0, 0.0, 0.0]);
        push_header(&mut wkb, LINE_STRING_Z);
        push_count(&mut wkb, 3);
        push_coordinate(&mut wkb, [1.0, 1.0, 0.0]);
        push_coordinate(&mut wkb, [0.0, 1.0, 0.0]);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);

        let (boundary, parsed_vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();

        assert_eq!(boundary.check_type(), BoundaryType::MultiLineString);
        assert_eq!(raw_values(&boundary.rings), vec![0, 2]);
        assert_eq!(raw_values(&boundary.vertices), vec![0, 1, 2, 3, 4]);
        assert_eq!(parsed_vertices.len(), 5);
    }

    #[test]
    fn parses_multi_polygon_z_and_stores_open_rings() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POLYGON_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POLYGON_Z);
        push_count(&mut wkb, 2);
        push_count(&mut wkb, 5);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);
        push_coordinate(&mut wkb, [1.0, 0.0, 0.0]);
        push_coordinate(&mut wkb, [1.0, 1.0, 0.0]);
        push_coordinate(&mut wkb, [0.0, 1.0, 0.0]);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);
        push_count(&mut wkb, 5);
        push_coordinate(&mut wkb, [0.25, 0.25, 1.0]);
        push_coordinate(&mut wkb, [0.75, 0.25, 1.0]);
        push_coordinate(&mut wkb, [0.75, 0.75, 1.0]);
        push_coordinate(&mut wkb, [0.25, 0.75, 1.0]);
        push_coordinate(&mut wkb, [0.25, 0.25, 1.0]);

        let (boundary, parsed_vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();

        assert_eq!(boundary.check_type(), BoundaryType::MultiOrCompositeSurface);
        assert_eq!(raw_values(&boundary.surfaces), vec![0]);
        assert_eq!(raw_values(&boundary.rings), vec![0, 4]);
        assert_eq!(raw_values(&boundary.vertices), vec![0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(parsed_vertices.len(), 8);
        assert_eq!(
            parsed_vertices.as_slice()[0],
            RealWorldCoordinate::new(0.0, 0.0, 0.0)
        );
        assert_eq!(
            parsed_vertices.as_slice()[3],
            RealWorldCoordinate::new(0.0, 1.0, 0.0)
        );
    }

    #[test]
    fn rejects_big_endian_wkb() {
        let mut wkb = Vec::new();
        wkb.push(0);
        wkb.extend_from_slice(&MULTI_POINT_Z.to_be_bytes());

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_ewkb_type_flags() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, 0x8000_0000 | MULTI_POINT_Z);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_top_level_singular_geometry() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, POINT_Z);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_non_z_iso_type_codes() {
        for geometry_type in [4, 2_004, 3_004] {
            let mut wkb = Vec::new();
            push_header(&mut wkb, geometry_type);

            assert_invalid_geometry(&wkb);
        }
    }

    #[test]
    fn rejects_wrong_child_type() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_LINE_STRING_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POINT_Z);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_unclosed_polygon_ring() {
        let mut wkb = closed_square_wkb();
        let len = wkb.len();
        wkb.truncate(len - COORDINATE_Z_BYTES);
        push_coordinate(&mut wkb, [2.0, 2.0, 0.0]);

        assert_invalid_ring(&wkb);
    }

    #[test]
    fn rejects_too_short_polygon_ring() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POLYGON_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POLYGON_Z);
        push_count(&mut wkb, 1);
        push_count(&mut wkb, 3);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);
        push_coordinate(&mut wkb, [1.0, 0.0, 0.0]);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);

        assert_invalid_ring(&wkb);
    }

    #[test]
    fn rejects_truncated_wkb() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POINT_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POINT_Z);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POINT_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POINT_Z);
        push_coordinate(&mut wkb, [0.0, 0.0, 0.0]);
        wkb.push(0);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_unsupported_type_code() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, 9_999);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_empty_top_level_multi_geometry() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POINT_Z);
        push_count(&mut wkb, 0);

        assert_invalid_geometry(&wkb);
    }
}
