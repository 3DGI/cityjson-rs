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
//! - `Solid`: standard WKB has no solid geometry type, so shells are expanded through
//!   `shells -> surfaces` and the reached surfaces are emitted as one `MultiPolygonZ`.
//! - `MultiSolid` and `CompositeSolid`: solids and shells are expanded through
//!   `solids -> shells -> surfaces`, and the reached surfaces are emitted as one
//!   `MultiPolygonZ`.
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
            let vertex_start = self.boundary.rings[ring_index].try_to_usize()?;
            let vertex_end = self
                .boundary
                .rings
                .get(ring_index + 1)
                .map(VertexIndex::try_to_usize)
                .transpose()?
                .unwrap_or(self.boundary.vertices.len());

            self.write_header(LINE_STRING_Z);
            self.write_count(vertex_end - vertex_start)?;
            for vertex_index in &self.boundary.vertices[vertex_start..vertex_end] {
                self.write_coordinate(*vertex_index)?;
            }
        }

        Ok(())
    }

    fn write_multi_polygon(&mut self) -> error::Result<()> {
        let mut surface_indices = Vec::with_capacity(self.boundary.surfaces.len());

        match self.boundary.check_type() {
            BoundaryType::MultiOrCompositeSurface => {
                surface_indices.extend(0..self.boundary.surfaces.len());
            }
            BoundaryType::Solid => {
                for shell_index in 0..self.boundary.shells.len() {
                    let surface_start = self.boundary.shells[shell_index].try_to_usize()?;
                    let surface_end = self
                        .boundary
                        .shells
                        .get(shell_index + 1)
                        .map(VertexIndex::try_to_usize)
                        .transpose()?
                        .unwrap_or(self.boundary.surfaces.len());
                    surface_indices.extend(surface_start..surface_end);
                }
            }
            BoundaryType::MultiOrCompositeSolid => {
                for solid_index in 0..self.boundary.solids.len() {
                    let shell_start = self.boundary.solids[solid_index].try_to_usize()?;
                    let shell_end = self
                        .boundary
                        .solids
                        .get(solid_index + 1)
                        .map(VertexIndex::try_to_usize)
                        .transpose()?
                        .unwrap_or(self.boundary.shells.len());

                    for shell_index in shell_start..shell_end {
                        let surface_start = self.boundary.shells[shell_index].try_to_usize()?;
                        let surface_end = self
                            .boundary
                            .shells
                            .get(shell_index + 1)
                            .map(VertexIndex::try_to_usize)
                            .transpose()?
                            .unwrap_or(self.boundary.surfaces.len());
                        surface_indices.extend(surface_start..surface_end);
                    }
                }
            }
            BoundaryType::MultiPoint | BoundaryType::MultiLineString | BoundaryType::None => {
                return Err(error::Error::InvalidGeometry(
                    "cannot write non-surface boundary as MultiPolygonZ".to_owned(),
                ));
            }
        }

        if surface_indices.is_empty() {
            return Err(error::Error::InvalidGeometry(
                "cannot write a surface-backed boundary with no polygons as WKB".to_owned(),
            ));
        }

        self.write_header(MULTI_POLYGON_Z);
        self.write_count(surface_indices.len())?;

        for surface_index in surface_indices {
            self.write_polygon(surface_index)?;
        }

        Ok(())
    }

    fn write_polygon(&mut self, surface_index: usize) -> error::Result<()> {
        let ring_start = self.boundary.surfaces[surface_index].try_to_usize()?;
        let ring_end = self
            .boundary
            .surfaces
            .get(surface_index + 1)
            .map(VertexIndex::try_to_usize)
            .transpose()?
            .unwrap_or(self.boundary.rings.len());
        if ring_start == ring_end {
            return Err(error::Error::InvalidGeometry(
                "cannot write a WKB polygon with no rings".to_owned(),
            ));
        }

        self.write_header(POLYGON_Z);
        self.write_count(ring_end - ring_start)?;

        for ring_index in ring_start..ring_end {
            let vertex_start = self.boundary.rings[ring_index].try_to_usize()?;
            let vertex_end = self
                .boundary
                .rings
                .get(ring_index + 1)
                .map(VertexIndex::try_to_usize)
                .transpose()?
                .unwrap_or(self.boundary.vertices.len());
            let vertex_range = vertex_start..vertex_end;
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
            let ring_count = self.read_count()?;
            if ring_count == 0 {
                return Err(invalid_wkb("PolygonZ must contain at least one ring"));
            }

            push_offset(&mut boundary.surfaces, boundary.rings.len())?;
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
#[allow(clippy::too_many_lines)]
mod tests {
    use super::*;
    use crate::backend::default::boundary::test_cases;
    use crate::cityjson::core::vertex::VertexIndex;

    fn read_u8(bytes: &[u8], offset: &mut usize) -> u8 {
        let value = bytes[*offset];
        *offset += 1;
        value
    }

    fn read_u32(bytes: &[u8], offset: &mut usize) -> u32 {
        let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += WKB_COUNT_BYTES;
        value
    }

    fn read_f64(bytes: &[u8], offset: &mut usize) -> f64 {
        let value = f64::from_le_bytes(bytes[*offset..*offset + 8].try_into().unwrap());
        *offset += std::mem::size_of::<f64>();
        value
    }

    fn read_coordinate(bytes: &[u8], offset: &mut usize) -> [f64; 3] {
        [
            read_f64(bytes, offset),
            read_f64(bytes, offset),
            read_f64(bytes, offset),
        ]
    }

    fn read_header(bytes: &[u8], offset: &mut usize) -> u32 {
        assert_eq!(read_u8(bytes, offset), LITTLE_ENDIAN);
        read_u32(bytes, offset)
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

    fn assert_coordinate_eq(actual: [f64; 3], expected: [f64; 3]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }

    fn raw_values(indices: &[VertexIndex<u32>]) -> Vec<u32> {
        indices.iter().map(VertexIndex::value).collect()
    }

    fn top_level_geometry_type(boundary: &Boundary<u32>) -> u32 {
        let bytes = boundary.to_wkb(&test_cases::vertices()).unwrap();
        let mut offset = 0;
        read_header(&bytes, &mut offset)
    }

    fn polygon_rings(bytes: &[u8]) -> Vec<Vec<Vec<[f64; 3]>>> {
        let mut offset = 0;
        assert_eq!(read_header(bytes, &mut offset), MULTI_POLYGON_Z);
        let polygon_count = read_u32(bytes, &mut offset);
        let mut polygons = Vec::new();

        for _ in 0..polygon_count {
            assert_eq!(read_header(bytes, &mut offset), POLYGON_Z);
            let ring_count = read_u32(bytes, &mut offset);
            let mut rings = Vec::new();

            for _ in 0..ring_count {
                let coordinate_count = read_u32(bytes, &mut offset);
                let mut coordinates = Vec::new();
                for _ in 0..coordinate_count {
                    coordinates.push(read_coordinate(bytes, &mut offset));
                }
                rings.push(coordinates);
            }

            polygons.push(rings);
        }

        assert_eq!(offset, bytes.len());
        polygons
    }

    fn assert_boundary_wkb_byte_stable(boundary: &Boundary<u32>) {
        let vertices = test_cases::vertices();
        let wkb = boundary.to_wkb(&vertices).unwrap();
        let (parsed_boundary, parsed_vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();
        let encoded_again = parsed_boundary.to_wkb(&parsed_vertices).unwrap();

        assert_eq!(encoded_again, wkb);
    }

    fn multi_point_wkb(coordinates: &[[f64; 3]]) -> Vec<u8> {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POINT_Z);
        push_count(&mut wkb, coordinates.len().try_into().unwrap());
        for coordinate in coordinates {
            push_header(&mut wkb, POINT_Z);
            push_coordinate(&mut wkb, *coordinate);
        }
        wkb
    }

    fn multi_line_string_wkb(lines: &[&[[f64; 3]]]) -> Vec<u8> {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_LINE_STRING_Z);
        push_count(&mut wkb, lines.len().try_into().unwrap());
        for line in lines {
            push_header(&mut wkb, LINE_STRING_Z);
            push_count(&mut wkb, line.len().try_into().unwrap());
            for coordinate in *line {
                push_coordinate(&mut wkb, *coordinate);
            }
        }
        wkb
    }

    fn multi_polygon_wkb(polygons: &[&[&[[f64; 3]]]]) -> Vec<u8> {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POLYGON_Z);
        push_count(&mut wkb, polygons.len().try_into().unwrap());
        for polygon in polygons {
            push_header(&mut wkb, POLYGON_Z);
            push_count(&mut wkb, polygon.len().try_into().unwrap());
            for ring in *polygon {
                push_count(&mut wkb, ring.len().try_into().unwrap());
                for coordinate in *ring {
                    push_coordinate(&mut wkb, *coordinate);
                }
            }
        }
        wkb
    }

    fn closed_square_wkb() -> Vec<u8> {
        multi_polygon_wkb(&[&[&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ]]])
    }

    fn polygon_with_zero_rings_wkb() -> Vec<u8> {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POLYGON_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POLYGON_Z);
        push_count(&mut wkb, 0);
        wkb
    }

    fn assert_invalid_geometry(bytes: &[u8]) {
        let error = Boundary::<u32>::from_wkb(bytes).unwrap_err();
        assert!(matches!(error, error::Error::InvalidGeometry(_)));
    }

    fn assert_invalid_ring(bytes: &[u8]) {
        let error = Boundary::<u32>::from_wkb(bytes).unwrap_err();
        assert!(matches!(error, error::Error::InvalidRing { .. }));
    }

    #[test]
    fn writes_supported_boundary_types_to_expected_wkb_types() {
        let cases = [
            (test_cases::multipoint_repeated_refs(), MULTI_POINT_Z),
            (
                test_cases::multilinestring_two_segments(),
                MULTI_LINE_STRING_Z,
            ),
            (test_cases::surface_with_hole(), MULTI_POLYGON_Z),
            (test_cases::solid_two_shells(), MULTI_POLYGON_Z),
            (test_cases::multi_solid_ordered(), MULTI_POLYGON_Z),
        ];

        for (boundary, expected_type) in cases {
            assert_eq!(top_level_geometry_type(&boundary), expected_type);
        }
    }

    #[test]
    fn boundary_to_wkb_to_wkb_is_byte_stable_for_preservable_shapes() {
        for boundary in [
            test_cases::multipoint_repeated_refs(),
            test_cases::multilinestring_two_segments(),
            test_cases::surface_open_triangle(),
            test_cases::surface_with_hole(),
            test_cases::multi_surface_two_polygons(),
        ] {
            assert_boundary_wkb_byte_stable(&boundary);
        }
    }

    #[test]
    fn wkb_to_boundary_to_wkb_is_byte_stable_for_supported_inputs() {
        let point_wkb = multi_point_wkb(&[[0.0, 1.0, -2.0], [3.5, 4.0, 5.25]]);
        let line0 = [[0.0, 0.0, 0.0], [1.0, 0.0, 2.0]];
        let line1 = [[1.0, 1.0, 0.0], [0.0, 1.0, -1.0], [0.0, 0.0, 0.0]];
        let line_wkb = multi_line_string_wkb(&[&line0, &line1]);
        let polygon_wkb = closed_square_wkb();

        for wkb in [point_wkb, line_wkb, polygon_wkb] {
            let (boundary, vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();
            assert_eq!(boundary.to_wkb(&vertices).unwrap(), wkb);
        }
    }

    #[test]
    fn parses_multi_point_z_into_boundary_and_vertices() {
        let wkb = multi_point_wkb(&[[0.0, 1.0, -2.0], [3.5, 4.0, 5.25]]);
        let (boundary, parsed_vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();

        assert_eq!(boundary.check_type(), BoundaryType::MultiPoint);
        assert_eq!(raw_values(&boundary.vertices), vec![0, 1]);
        assert_eq!(parsed_vertices.len(), 2);
        assert_eq!(
            parsed_vertices.as_slice()[0],
            RealWorldCoordinate::new(0.0, 1.0, -2.0)
        );
        assert_eq!(
            parsed_vertices.as_slice()[1],
            RealWorldCoordinate::new(3.5, 4.0, 5.25)
        );
    }

    #[test]
    fn parses_multi_line_string_z_into_boundary_and_vertices() {
        let line0 = [[0.0, 0.0, 0.0], [1.0, 0.0, 2.0]];
        let line1 = [[1.0, 1.0, 0.0], [0.0, 1.0, -1.0], [0.0, 0.0, 0.0]];
        let wkb = multi_line_string_wkb(&[&line0, &line1]);
        let (boundary, parsed_vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();

        assert_eq!(boundary.check_type(), BoundaryType::MultiLineString);
        assert_eq!(raw_values(&boundary.rings), vec![0, 2]);
        assert_eq!(raw_values(&boundary.vertices), vec![0, 1, 2, 3, 4]);
        assert_eq!(parsed_vertices.len(), 5);
        assert_eq!(
            parsed_vertices.as_slice()[2],
            RealWorldCoordinate::new(1.0, 1.0, 0.0)
        );
        assert_eq!(
            parsed_vertices.as_slice()[4],
            RealWorldCoordinate::new(0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn parses_multi_polygon_z_into_open_boundary_rings_and_vertices() {
        let outer = &[
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let inner = &[
            [0.5, 0.5, 1.0],
            [1.5, 0.5, 1.0],
            [1.5, 1.5, 1.0],
            [0.5, 0.5, 1.0],
        ];
        let wkb = multi_polygon_wkb(&[&[outer, inner]]);
        let (boundary, parsed_vertices) = Boundary::<u32>::from_wkb(&wkb).unwrap();

        assert_eq!(boundary.check_type(), BoundaryType::MultiOrCompositeSurface);
        assert_eq!(raw_values(&boundary.surfaces), vec![0]);
        assert_eq!(raw_values(&boundary.rings), vec![0, 4]);
        assert_eq!(raw_values(&boundary.vertices), vec![0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(parsed_vertices.len(), 7);
        assert_eq!(
            parsed_vertices.as_slice()[0],
            RealWorldCoordinate::new(0.0, 0.0, 0.0)
        );
        assert_eq!(
            parsed_vertices.as_slice()[6],
            RealWorldCoordinate::new(1.5, 1.5, 1.0)
        );
    }

    #[test]
    fn repeated_point_refs_emit_repeated_point_children() {
        let bytes = test_cases::multipoint_repeated_refs()
            .to_wkb(&test_cases::vertices())
            .unwrap();
        let mut offset = 0;

        assert_eq!(read_header(&bytes, &mut offset), MULTI_POINT_Z);
        assert_eq!(read_u32(&bytes, &mut offset), 4);

        for expected in [
            [0.0, 0.0, 0.0],
            [1.5, 1.25, 3.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.25, 0.5],
        ] {
            assert_eq!(read_header(&bytes, &mut offset), POINT_Z);
            assert_coordinate_eq(read_coordinate(&bytes, &mut offset), expected);
        }
        assert_eq!(offset, bytes.len());
    }

    #[test]
    fn coordinate_components_preserve_f64_bits() {
        let bytes = test_cases::multipoint_repeated_refs()
            .to_wkb(&test_cases::vertices())
            .unwrap();
        let mut offset = WKB_HEADER_BYTES + WKB_COUNT_BYTES + WKB_HEADER_BYTES;

        assert_coordinate_eq(read_coordinate(&bytes, &mut offset), [0.0, 0.0, 0.0]);
        offset += WKB_HEADER_BYTES;
        assert_coordinate_eq(read_coordinate(&bytes, &mut offset), [1.5, 1.25, 3.0]);
        offset += WKB_HEADER_BYTES;
        assert_coordinate_eq(read_coordinate(&bytes, &mut offset), [0.0, 0.0, 0.0]);
        offset += WKB_HEADER_BYTES;
        assert_coordinate_eq(read_coordinate(&bytes, &mut offset), [0.0, 1.25, 0.5]);
    }

    #[test]
    fn open_polygon_rings_are_closed_on_write() {
        let bytes = test_cases::surface_open_triangle()
            .to_wkb(&test_cases::vertices())
            .unwrap();
        let polygons = polygon_rings(&bytes);
        let ring = &polygons[0][0];

        assert_eq!(ring.len(), 4);
        assert_coordinate_eq(ring[0], [0.0, 0.0, 0.0]);
        assert_coordinate_eq(*ring.last().unwrap(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn already_closed_legacy_rings_are_not_double_closed() {
        let bytes = test_cases::legacy_closed_surface()
            .to_wkb(&test_cases::vertices())
            .unwrap();
        let polygons = polygon_rings(&bytes);

        assert_eq!(polygons[0][0].len(), 5);
    }

    #[test]
    fn holes_stay_attached_to_the_same_polygon() {
        let bytes = test_cases::surface_with_hole()
            .to_wkb(&test_cases::vertices())
            .unwrap();
        let polygons = polygon_rings(&bytes);

        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].len(), 2);
        assert_coordinate_eq(polygons[0][0][0], [0.0, 0.0, 0.0]);
        assert_coordinate_eq(polygons[0][1][0], [0.25, 0.25, 1.0]);
    }

    #[test]
    fn solid_shells_flatten_in_shell_surface_order() {
        let bytes = test_cases::solid_two_shells()
            .to_wkb(&test_cases::vertices())
            .unwrap();
        let polygons = polygon_rings(&bytes);

        assert_eq!(polygons.len(), 2);
        assert_coordinate_eq(polygons[0][0][0], [0.25, 0.25, 1.0]);
        assert_coordinate_eq(polygons[1][0][0], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn multi_solids_flatten_in_solid_shell_surface_order() {
        let bytes = test_cases::multi_solid_ordered()
            .to_wkb(&test_cases::vertices())
            .unwrap();
        let polygons = polygon_rings(&bytes);

        assert_eq!(polygons.len(), 3);
        assert_coordinate_eq(polygons[0][0][0], [0.25, 0.25, 1.0]);
        assert_coordinate_eq(polygons[1][0][0], [0.0, 0.0, 0.0]);
        assert_coordinate_eq(polygons[2][0][0], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn rejects_empty_boundary() {
        let error = Boundary::<u32>::new()
            .to_wkb(&test_cases::vertices())
            .unwrap_err();

        assert!(matches!(error, error::Error::InvalidGeometry(_)));
    }

    #[test]
    fn rejects_inconsistent_offsets() {
        let mut boundary = Boundary::<u32>::new();
        boundary.vertices = vec![VertexIndex::new(0), VertexIndex::new(1)];
        boundary.rings = vec![VertexIndex::new(3)];
        let error = boundary.to_wkb(&test_cases::vertices()).unwrap_err();

        assert!(matches!(error, error::Error::InvalidGeometry(_)));
    }

    #[test]
    fn rejects_missing_vertex_reference() {
        let boundary: Boundary<u32> = vec![12].into();
        let error = boundary.to_wkb(&test_cases::vertices()).unwrap_err();

        assert!(matches!(
            error,
            error::Error::InvalidReference { index: 12, .. }
        ));
    }

    #[test]
    fn rejects_surface_backed_boundary_with_no_reachable_polygons() {
        let boundary = Boundary::from_parts(
            vec![
                VertexIndex::new(0),
                VertexIndex::new(1),
                VertexIndex::new(2),
            ],
            vec![VertexIndex::new(0)],
            vec![VertexIndex::new(0)],
            Vec::new(),
            vec![VertexIndex::new(0)],
        )
        .unwrap();
        let error = boundary.to_wkb(&test_cases::vertices()).unwrap_err();

        assert!(matches!(error, error::Error::InvalidGeometry(_)));
    }

    #[test]
    fn rejects_polygon_with_no_rings() {
        let boundary = Boundary::from_parts(
            Vec::new(),
            Vec::new(),
            vec![VertexIndex::new(0)],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let error = boundary.to_wkb(&test_cases::vertices()).unwrap_err();

        assert!(matches!(error, error::Error::InvalidGeometry(_)));
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
        let error = boundary.to_wkb(&test_cases::vertices()).unwrap_err();

        assert!(matches!(error, error::Error::InvalidRing { .. }));
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
    fn rejects_unsupported_and_non_z_iso_type_codes() {
        for geometry_type in [4, 2_004, 3_004, 9_999] {
            let mut wkb = Vec::new();
            push_header(&mut wkb, geometry_type);

            assert_invalid_geometry(&wkb);
        }
    }

    #[test]
    fn rejects_top_level_singular_geometry() {
        for geometry_type in [POINT_Z, LINE_STRING_Z, POLYGON_Z] {
            let mut wkb = Vec::new();
            push_header(&mut wkb, geometry_type);

            assert_invalid_geometry(&wkb);
        }
    }

    #[test]
    fn rejects_wrong_child_geometry_type() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_LINE_STRING_Z);
        push_count(&mut wkb, 1);
        push_header(&mut wkb, POINT_Z);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_empty_top_level_multi_geometry() {
        let mut wkb = Vec::new();
        push_header(&mut wkb, MULTI_POINT_Z);
        push_count(&mut wkb, 0);

        assert_invalid_geometry(&wkb);
    }

    #[test]
    fn rejects_polygon_with_zero_rings() {
        assert_invalid_geometry(&polygon_with_zero_rings_wkb());
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
        let wkb = multi_polygon_wkb(&[&[&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]]]]);

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
        let mut wkb = multi_point_wkb(&[[0.0, 0.0, 0.0]]);
        wkb.push(0);

        assert_invalid_geometry(&wkb);
    }
}
