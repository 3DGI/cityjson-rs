//! # Boundary Representation for `CityJSON` Geometries
//!
//! ## Nested vs. Flattened Representations
//!
//! ### Nested Representation
//!
//! `CityJSON` defines geometry boundaries using nested arrays in JSON. For example, a `MultiSurface`
//! is represented as an array of surfaces, where each surface is an array of rings, and each ring
//! is an array of vertex indices. This nested structure is intuitive and directly maps to the
//! JSON representation, but can be inefficient for processing and memory usage due to the overhead
//! of nested vectors.
//!
//! The `nested` module provides types that match this structure:
//! - `BoundaryNestedMultiPoint<T>`
//! - `BoundaryNestedMultiLineString<T>`
//! - `BoundaryNestedMultiOrCompositeSurface<T>`
//! - `BoundaryNestedSolid<T>`
//! - `BoundaryNestedMultiOrCompositeSolid<T>`
//!
//! ### Flattened Representation
//!
//! The `Boundary<VR>` struct provides a memory-efficient "flattened" representation that uses a
//! series of offset indices to navigate through a single, contiguous array. This approach:
//! - Reduces memory overhead from nested vectors
//! - Improves cache locality for better performance
//! - Simplifies operations on the geometry
//!
//! For example, a `MultiSurface` is represented using:
//! - A single vector of vertex indices
//! - A vector of ring indices (pointing into the vertex vector)
//! - A vector of surface indices (pointing into the ring vector)
//!
//! ## Usage
//!
//! The nested representation is primarily used for serialization/deserialization to/from `CityJSON`,
//! while the flattened representation is used for internal processing within cityjson-rs. The module
//! provides methods to convert between the two representations.
//!
//! ```rust
//! use cityjson_types::v2_0::{Boundary, BoundaryType};
//! use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiPoint32;
//!
//! // Create a nested representation of a MultiPoint
//! let multi_point: BoundaryNestedMultiPoint32 = vec![0, 1, 2, 3];
//!
//! // Convert to flattened representation
//! let boundary: Boundary<u32> = multi_point.into();
//!
//! // Check boundary type
//! assert_eq!(boundary.check_type(), BoundaryType::MultiPoint);
//!
//! // Convert back to nested representation
//! let multi_point_again = boundary.to_nested_multi_point().unwrap();
//! assert_eq!(multi_point_again, vec![0, 1, 2, 3]);
//! ```

pub mod nested;
#[cfg(test)]
mod test_cases;
mod wkb;

use super::vertices::Vertices;
use crate::cityjson::core::boundary::nested::{
    BoundaryNestedMultiLineString, BoundaryNestedMultiOrCompositeSolid,
    BoundaryNestedMultiOrCompositeSurface, BoundaryNestedMultiPoint, BoundaryNestedSolid,
};
use crate::cityjson::core::coordinate::Coordinate;
use crate::cityjson::core::vertex::VertexRef;
use crate::cityjson::core::vertex::{RawVertexView, VertexIndex};
use crate::error;

// Type aliases for convenience
/// A boundary using 16-bit vertex indices (suitable for up to 65,535 vertices)
pub type Boundary16 = Boundary<u16>;
/// A boundary using 32-bit vertex indices (suitable for up to ~4.3 billion vertices)
pub type Boundary32 = Boundary<u32>;
/// A boundary using 64-bit vertex indices (suitable for virtually unlimited vertices)
pub type Boundary64 = Boundary<u64>;

/// Flattened boundary for any `CityJSON` geometry type (`MultiPoint` through `MultiSolid`).
///
/// See the module-level documentation for the nested vs. flattened representation.
///
/// # Type Parameters
///
/// * `VR` — vertex reference type (`u16`, `u32`, or `u64`), determines index limits
///
/// # Example
///
/// ```
/// use cityjson_types::v2_0::{Boundary, BoundaryType};
/// use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiLineString32;
///
/// // Create a nested representation of a multi-linestring
/// let multi_linestring: BoundaryNestedMultiLineString32 = vec![
///     vec![0, 1, 2],       // First linestring
///     vec![3, 4, 5, 6],    // Second linestring
/// ];
///
/// // Convert to flattened representation
/// let boundary: Boundary<u32> = multi_linestring.try_into().unwrap();
///
/// // Check the boundary type
/// assert_eq!(boundary.check_type(), BoundaryType::MultiLineString);
///
/// // Convert back to nested representation
/// let multi_linestring_again = boundary.to_nested_multi_linestring().unwrap();
/// assert_eq!(multi_linestring_again, vec![vec![0, 1, 2], vec![3, 4, 5, 6]]);
/// ```
#[repr(C)]
#[derive(Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Boundary<VR: VertexRef> {
    /// Vertex indices that point to the global Vertices buffer.
    pub(crate) vertices: Vec<VertexIndex<VR>>,
    /// Vertex offsets that mark the start of each ring. The values point to this Boundary's vertices.
    pub(crate) rings: Vec<VertexIndex<VR>>,
    /// Ring offsets that mark the start of each surface. The values point to this Boundary's rings.
    pub(crate) surfaces: Vec<VertexIndex<VR>>,
    /// Surface offsets that mark the start of each shell. The values point to this Boundary's surfaces.
    pub(crate) shells: Vec<VertexIndex<VR>>,
    /// Shell offsets that mark the start of each solid. The values point to this Boundary's shells.
    pub(crate) solids: Vec<VertexIndex<VR>>,
}

/// Columnar representation of a [`Boundary`].
///
/// Each field is a flat buffer with offsets to the next level.
#[derive(Debug, Clone, Copy)]
pub struct BoundaryColumnar<'a, VR: VertexRef> {
    pub vertices: &'a [VertexIndex<VR>],
    pub ring_offsets: &'a [VertexIndex<VR>],
    pub surface_offsets: &'a [VertexIndex<VR>],
    pub shell_offsets: &'a [VertexIndex<VR>],
    pub solid_offsets: &'a [VertexIndex<VR>],
}

/// Iterator over the coordinates referenced by a [`Boundary`].
///
/// Coordinates are yielded in boundary order. Repeated vertex references are preserved.
pub struct BoundaryCoordinates<'a, VR: VertexRef, V: Coordinate> {
    indices: std::slice::Iter<'a, VertexIndex<VR>>,
    vertices: &'a Vertices<VR, V>,
}

impl<'a, VR: VertexRef, V: Coordinate> Iterator for BoundaryCoordinates<'a, VR, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let index = *self.indices.next()?;
            if let Some(coordinate) = self.vertices.get(index) {
                return Some(coordinate);
            }
        }
    }
}

/// Iterator over the unique coordinates referenced by a [`Boundary`].
///
/// Coordinates are yielded in vertex-index order after deduplicating the boundary's vertex
/// references into the provided scratch buffer.
pub struct BoundaryUniqueCoordinates<'a, VR: VertexRef, V: Coordinate> {
    indices: std::slice::Iter<'a, VertexIndex<VR>>,
    vertices: &'a Vertices<VR, V>,
}

impl<'a, VR: VertexRef, V: Coordinate> Iterator for BoundaryUniqueCoordinates<'a, VR, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let index = *self.indices.next()?;
            if let Some(coordinate) = self.vertices.get(index) {
                return Some(coordinate);
            }
        }
    }
}

impl<VR: VertexRef> Boundary<VR> {
    #[inline]
    fn offsets_are_consistent(offsets: &[VertexIndex<VR>], child_len: usize) -> bool {
        let Some(first) = offsets.first() else {
            return true;
        };

        if !first.is_zero() {
            return false;
        }

        if offsets
            .last()
            .is_some_and(|last| last.to_usize() > child_len)
        {
            return false;
        }

        offsets.windows(2).all(|window| {
            let start = window[0].to_usize();
            let end = window[1].to_usize();
            start <= end && end <= child_len
        })
    }

    fn ensure_convertible_as(&self, expected: BoundaryType) -> error::Result<()> {
        let boundary_type = self.check_type();
        if boundary_type != expected {
            return Err(error::Error::IncompatibleBoundary(
                boundary_type.to_string(),
                expected.to_string(),
            ));
        }

        if !self.is_consistent() {
            return Err(error::Error::InvalidGeometry(format!(
                "inconsistent {expected} boundary offsets"
            )));
        }

        Ok(())
    }

    /// Creates a new empty boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::Boundary;
    ///
    /// let boundary: Boundary<u32> = Boundary::new();
    /// assert!(boundary.is_consistent());
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new boundary with the specified capacity for each vector.
    ///
    /// # Parameters
    ///
    /// * `vertices` - Capacity for the vertices vector
    /// * `rings` - Capacity for the rings vector
    /// * `surfaces` - Capacity for the surfaces vector
    /// * `shells` - Capacity for the shells vector
    /// * `solids` - Capacity for the solids vector
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::Boundary;
    ///
    /// // Create a boundary with pre-allocated capacity
    /// let boundary: Boundary<u32> = Boundary::with_capacity(
    ///     100, // space for 100 vertices
    ///     20,  // space for 20 rings
    ///     10,  // space for 10 surfaces
    ///     2,   // space for 2 shells
    ///     1,   // space for 1 solid
    /// );
    /// assert!(boundary.is_consistent());
    /// ```
    #[inline]
    #[must_use]
    pub fn with_capacity(
        vertices: usize,
        rings: usize,
        surfaces: usize,
        shells: usize,
        solids: usize,
    ) -> Self {
        Self {
            vertices: Vec::with_capacity(vertices),
            rings: Vec::with_capacity(rings),
            surfaces: Vec::with_capacity(surfaces),
            shells: Vec::with_capacity(shells),
            solids: Vec::with_capacity(solids),
        }
    }

    /// Creates a boundary from owned flat parts.
    ///
    /// # Errors
    ///
    /// Returns an error when the offset buffers are inconsistent with the child buffers.
    pub fn from_parts(
        vertices: Vec<VertexIndex<VR>>,
        rings: Vec<VertexIndex<VR>>,
        surfaces: Vec<VertexIndex<VR>>,
        shells: Vec<VertexIndex<VR>>,
        solids: Vec<VertexIndex<VR>>,
    ) -> error::Result<Self> {
        let boundary = Self {
            vertices,
            rings,
            surfaces,
            shells,
            solids,
        };

        if !boundary.is_consistent() {
            return Err(error::Error::InvalidGeometry(
                "inconsistent boundary offsets".to_owned(),
            ));
        }

        Ok(boundary)
    }

    /// Creates a boundary from owned flat parts without validating topology offsets.
    ///
    /// This is intended for trusted callers that have already validated the stored boundary
    /// layout and want to avoid re-running the offset checks in the hot path.
    ///
    /// # Safety
    ///
    /// Callers must ensure that every non-empty offset buffer starts at zero, is monotonic, and
    /// never points past the end of its child buffer:
    ///
    /// - `rings` into `vertices`
    /// - `surfaces` into `rings`
    /// - `shells` into `surfaces`
    /// - `solids` into `shells`
    ///
    /// Passing malformed buffers is not memory-unsafe, but it produces a `Boundary` whose stored
    /// topology invariants are broken and which may later fail checked conversions.
    #[must_use]
    pub unsafe fn from_parts_unchecked(
        vertices: Vec<VertexIndex<VR>>,
        rings: Vec<VertexIndex<VR>>,
        surfaces: Vec<VertexIndex<VR>>,
        shells: Vec<VertexIndex<VR>>,
        solids: Vec<VertexIndex<VR>>,
    ) -> Self {
        Self {
            vertices,
            rings,
            surfaces,
            shells,
            solids,
        }
    }

    #[must_use]
    pub fn vertices_raw(&self) -> RawVertexView<'_, VR> {
        RawVertexView(&self.vertices)
    }

    #[must_use]
    pub fn rings_raw(&self) -> RawVertexView<'_, VR> {
        RawVertexView(&self.rings)
    }

    #[must_use]
    pub fn surfaces_raw(&self) -> RawVertexView<'_, VR> {
        RawVertexView(&self.surfaces)
    }

    #[must_use]
    pub fn shells_raw(&self) -> RawVertexView<'_, VR> {
        RawVertexView(&self.shells)
    }

    #[must_use]
    pub fn solids_raw(&self) -> RawVertexView<'_, VR> {
        RawVertexView(&self.solids)
    }

    /// Exports this boundary into a columnar view suitable for serializers.
    #[inline]
    #[must_use]
    pub fn to_columnar(&self) -> BoundaryColumnar<'_, VR> {
        BoundaryColumnar {
            vertices: &self.vertices,
            ring_offsets: &self.rings,
            surface_offsets: &self.surfaces,
            shell_offsets: &self.shells,
            solid_offsets: &self.solids,
        }
    }

    #[inline]
    #[must_use]
    pub fn vertices(&self) -> &[VertexIndex<VR>] {
        &self.vertices
    }

    /// Iterates the coordinates referenced by this boundary in boundary order.
    ///
    /// This is the fast topology-to-coordinate join primitive: it uses the boundary's vertex
    /// references directly against the provided vertex pool and performs no allocation.
    #[inline]
    #[must_use]
    pub fn coordinates<'a, V: Coordinate>(
        &'a self,
        vertices: &'a Vertices<VR, V>,
    ) -> BoundaryCoordinates<'a, VR, V> {
        BoundaryCoordinates {
            indices: self.vertices.iter(),
            vertices,
        }
    }

    /// Deduplicates this boundary's vertex references into the provided scratch buffer.
    ///
    /// The returned slice references `scratch`, sorted by vertex index.
    #[inline]
    pub fn unique_vertex_indices<'a>(
        &self,
        scratch: &'a mut Vec<VertexIndex<VR>>,
    ) -> &'a [VertexIndex<VR>] {
        scratch.clear();
        scratch.extend_from_slice(&self.vertices);
        scratch.sort_unstable();
        scratch.dedup();
        scratch.as_slice()
    }

    /// Iterates the unique coordinates referenced by this boundary.
    ///
    /// The caller provides a scratch buffer used to deduplicate the boundary's vertex references.
    #[inline]
    #[must_use]
    pub fn unique_coordinates<'a, V: Coordinate>(
        &'a self,
        vertices: &'a Vertices<VR, V>,
        scratch: &'a mut Vec<VertexIndex<VR>>,
    ) -> BoundaryUniqueCoordinates<'a, VR, V> {
        let indices = self.unique_vertex_indices(scratch);
        BoundaryUniqueCoordinates {
            indices: indices.iter(),
            vertices,
        }
    }

    /// Replaces items of the container with elements from the given iterator
    pub fn set_vertices_from_iter<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = VertexIndex<VR>>,
    {
        self.vertices = iter.into_iter().collect();
    }

    #[inline]
    #[must_use]
    pub fn rings(&self) -> &[VertexIndex<VR>] {
        &self.rings
    }

    /// Replaces items of the container with elements from the given iterator
    pub fn set_rings_from_iter<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = VertexIndex<VR>>,
    {
        self.rings = iter.into_iter().collect();
    }

    #[inline]
    #[must_use]
    pub fn surfaces(&self) -> &[VertexIndex<VR>] {
        &self.surfaces
    }

    /// Replaces items of the container with elements from the given iterator
    pub fn set_surfaces_from_iter<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = VertexIndex<VR>>,
    {
        self.surfaces = iter.into_iter().collect();
    }

    #[inline]
    #[must_use]
    pub fn shells(&self) -> &[VertexIndex<VR>] {
        &self.shells
    }

    /// Replaces items of the container with elements from the given iterator
    pub fn set_shells_from_iter<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = VertexIndex<VR>>,
    {
        self.shells = iter.into_iter().collect();
    }

    #[inline]
    #[must_use]
    pub fn solids(&self) -> &[VertexIndex<VR>] {
        &self.solids
    }

    /// Replaces items of the container with elements from the given iterator
    pub fn set_solids_from_iter<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = VertexIndex<VR>>,
    {
        self.solids = iter.into_iter().collect();
    }

    /// Converts to a nested `MultiPoint` boundary representation.
    ///
    /// This method converts the flattened boundary to a nested `MultiPoint` representation
    /// if the boundary can be interpreted as a `MultiPoint`.
    ///
    /// # Errors
    ///
    /// Returns [`error::Error::IncompatibleBoundary`] when this boundary is not a
    /// `MultiPoint`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::{Boundary, BoundaryType};
    /// use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiPoint32;
    ///
    /// // Create a boundary from a MultiPoint
    /// let multi_point: BoundaryNestedMultiPoint32 = vec![0, 1, 2, 3];
    /// let boundary: Boundary<u32> = multi_point.into();
    ///
    /// // Convert back to MultiPoint
    /// let nested = boundary.to_nested_multi_point().unwrap();
    /// assert_eq!(nested, vec![0, 1, 2, 3]);
    ///
    /// // Check type
    /// assert_eq!(boundary.check_type(), BoundaryType::MultiPoint);
    /// ```
    pub fn to_nested_multi_point(&self) -> error::Result<BoundaryNestedMultiPoint<VR>> {
        self.ensure_convertible_as(BoundaryType::MultiPoint)?;
        Ok(self
            .vertices
            .iter()
            .map(super::vertex::VertexIndex::value)
            .collect())
    }

    /// Converts to a nested `MultiLineString` boundary representation.
    ///
    /// This method converts the flattened boundary to a nested `MultiLineString` representation
    /// if the boundary can be interpreted as a `MultiLineString`.
    ///
    /// # Errors
    ///
    /// Returns [`error::Error::IncompatibleBoundary`] when this boundary is not a
    /// `MultiLineString`.
    /// Returns index-conversion errors when nested index offsets cannot be represented by `VR`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::Boundary;
    /// use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiLineString32;
    ///
    /// // Create a boundary from a MultiLineString
    /// let multi_linestring: BoundaryNestedMultiLineString32 = vec![
    ///     vec![0, 1, 2],
    ///     vec![3, 4, 5]
    /// ];
    /// let boundary: Boundary<u32> = multi_linestring.try_into().unwrap();
    ///
    /// // Convert back to MultiLineString
    /// let nested = boundary.to_nested_multi_linestring().unwrap();
    /// assert_eq!(nested, vec![vec![0, 1, 2], vec![3, 4, 5]]);
    /// ```
    pub fn to_nested_multi_linestring(&self) -> error::Result<BoundaryNestedMultiLineString<VR>> {
        self.ensure_convertible_as(BoundaryType::MultiLineString)?;
        let mut counter = BoundaryCounter::<VR>::default();
        let mut ml = BoundaryNestedMultiLineString::with_capacity(self.rings.len());
        self.push_rings_to_surface(self.rings.as_slice(), &mut ml, &mut counter)?;
        Ok(ml)
    }

    /// Converts to a nested Multi- or `CompositeSurface` boundary representation.
    ///
    /// This method converts the flattened boundary to a nested Multi- or `CompositeSurface`
    /// representation if the boundary can be interpreted as a Multi- or `CompositeSurface`.
    ///
    /// # Errors
    ///
    /// Returns [`error::Error::IncompatibleBoundary`] when this boundary is not a
    /// Multi- or `CompositeSurface`.
    /// Returns index-conversion errors when nested index offsets cannot be represented by `VR`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::{Boundary, BoundaryType};
    /// use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiOrCompositeSurface32;
    ///
    /// // Create a boundary from a MultiSurface
    /// // A simple MultiSurface with two surfaces, each with one ring
    /// let multi_surface: BoundaryNestedMultiOrCompositeSurface32 = vec![
    ///     vec![vec![0, 1, 2]], // First surface (triangle)
    ///     vec![vec![3, 4, 5]]  // Second surface (triangle)
    /// ];
    /// let boundary: Boundary<u32> = multi_surface.clone().try_into().unwrap();
    ///
    /// // Convert back to MultiSurface
    /// let nested = boundary.to_nested_multi_or_composite_surface().unwrap();
    /// assert_eq!(nested, multi_surface);
    /// ```
    pub fn to_nested_multi_or_composite_surface(
        &self,
    ) -> error::Result<BoundaryNestedMultiOrCompositeSurface<VR>> {
        self.ensure_convertible_as(BoundaryType::MultiOrCompositeSurface)?;
        let mut counter = BoundaryCounter::<VR>::default();
        let mut mc_surface =
            BoundaryNestedMultiOrCompositeSurface::with_capacity(self.surfaces.len());
        self.push_surfaces_to_multi_surface(
            self.surfaces.as_slice(),
            &mut mc_surface,
            &mut counter,
        )?;
        Ok(mc_surface)
    }

    /// Converts to a nested Solid boundary representation.
    ///
    /// This method converts the flattened boundary to a nested Solid representation
    /// if the boundary can be interpreted as a Solid.
    ///
    /// # Errors
    ///
    /// Returns [`error::Error::IncompatibleBoundary`] when this boundary is not a `Solid`.
    /// Returns index-conversion errors when nested index offsets cannot be represented by `VR`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::{Boundary, BoundaryType};
    /// use cityjson_types::v2_0::boundary::nested::BoundaryNestedSolid32;
    ///
    /// // Create a simplified solid representation (just one shell with one face for brevity)
    /// let solid: BoundaryNestedSolid32 = vec![
    ///     vec![vec![vec![0, 1, 2, 3]]] // One shell with one surface with one ring
    /// ];
    /// let boundary: Boundary<u32> = solid.clone().try_into().unwrap();
    ///
    /// // Check type
    /// assert_eq!(boundary.check_type(), BoundaryType::Solid);
    ///
    /// // Convert back to Solid
    /// let nested = boundary.to_nested_solid().unwrap();
    /// assert_eq!(nested, solid);
    /// ```
    pub fn to_nested_solid(&self) -> error::Result<BoundaryNestedSolid<VR>> {
        self.ensure_convertible_as(BoundaryType::Solid)?;
        let mut counter = BoundaryCounter::<VR>::default();
        let mut solid = BoundaryNestedSolid::with_capacity(self.shells.len());
        self.push_shells_to_solid(self.shells.as_slice(), &mut solid, &mut counter)?;
        Ok(solid)
    }

    /// Converts to a nested Multi- or `CompositeSolid` boundary representation.
    ///
    /// This method converts the flattened boundary to a nested Multi- or `CompositeSolid`
    /// representation if the boundary can be interpreted as a Multi- or `CompositeSolid`.
    ///
    /// # Errors
    ///
    /// Returns [`error::Error::IncompatibleBoundary`] when this boundary is not a
    /// Multi- or `CompositeSolid`.
    /// Returns index-conversion errors when nested index offsets cannot be represented by `VR`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::{Boundary, BoundaryType};
    /// use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiOrCompositeSolid32;
    ///
    /// // Create a very simplified MultiSolid (just two solids with minimal structure for brevity)
    /// let multi_solid: BoundaryNestedMultiOrCompositeSolid32 = vec![
    ///     vec![vec![vec![vec![0, 1, 2, 3]]]],  // First solid
    ///     vec![vec![vec![vec![4, 5, 6, 7]]]]   // Second solid
    /// ];
    /// let boundary: Boundary<u32> = multi_solid.clone().try_into().unwrap();
    ///
    /// // Check type
    /// assert_eq!(boundary.check_type(), BoundaryType::MultiOrCompositeSolid);
    ///
    /// // Convert back to MultiSolid
    /// let nested = boundary.to_nested_multi_or_composite_solid().unwrap();
    /// assert_eq!(nested, multi_solid);
    /// ```
    pub fn to_nested_multi_or_composite_solid(
        &self,
    ) -> error::Result<BoundaryNestedMultiOrCompositeSolid<VR>> {
        self.ensure_convertible_as(BoundaryType::MultiOrCompositeSolid)?;
        let mut counter = BoundaryCounter::<VR>::default();
        let mut mc_solid = BoundaryNestedMultiOrCompositeSolid::with_capacity(self.solids.len());
        for &shells_start_i in &self.solids {
            let shells_len = VertexIndex::<VR>::try_from(self.shells.len())?;
            let shells_end_i = self
                .solids
                .get(counter.try_increment_solid_idx()?.to_usize())
                .copied()
                .unwrap_or(shells_len);

            if let Some(shells) = self
                .shells
                .get(shells_start_i.to_usize()..shells_end_i.to_usize())
            {
                let mut solid = BoundaryNestedSolid::with_capacity(shells.len());
                self.push_shells_to_solid(shells, &mut solid, &mut counter)?;
                mc_solid.push(solid);
            }
        }
        Ok(mc_solid)
    }

    /// Helper method to process shells for a solid
    fn push_shells_to_solid(
        &self,
        shells: &[VertexIndex<VR>],
        solid: &mut Vec<BoundaryNestedMultiOrCompositeSurface<VR>>,
        counter: &mut BoundaryCounter<VR>,
    ) -> error::Result<()> {
        for &surfaces_start_i in shells {
            let surfaces_len = VertexIndex::<VR>::try_from(self.surfaces.len())?;
            let surfaces_end_i = self
                .shells
                .get(counter.try_increment_shell_idx()?.to_usize())
                .copied()
                .unwrap_or(surfaces_len);

            if let Some(surfaces) = self
                .surfaces
                .get(surfaces_start_i.to_usize()..surfaces_end_i.to_usize())
            {
                let mut mc_surface =
                    BoundaryNestedMultiOrCompositeSurface::with_capacity(surfaces.len());
                self.push_surfaces_to_multi_surface(surfaces, &mut mc_surface, counter)?;
                solid.push(mc_surface);
            }
        }
        Ok(())
    }

    /// Helper method to process surfaces for a shell
    fn push_surfaces_to_multi_surface(
        &self,
        surfaces: &[VertexIndex<VR>],
        mc_surface: &mut BoundaryNestedMultiOrCompositeSurface<VR>,
        counter: &mut BoundaryCounter<VR>,
    ) -> error::Result<()> {
        for &ring_start_i in surfaces {
            let rings_len = VertexIndex::<VR>::try_from(self.rings.len())?;
            let ring_end_i = self
                .surfaces
                .get(counter.try_increment_surface_idx()?.to_usize())
                .copied()
                .unwrap_or(rings_len);

            if let Some(rings) = self
                .rings
                .get(ring_start_i.to_usize()..ring_end_i.to_usize())
            {
                let mut surface = BoundaryNestedMultiLineString::with_capacity(rings.len());
                self.push_rings_to_surface(rings, &mut surface, counter)?;
                mc_surface.push(surface);
            }
        }
        Ok(())
    }

    /// Helper method to process rings for a surface
    fn push_rings_to_surface(
        &self,
        rings: &[VertexIndex<VR>],
        surface: &mut BoundaryNestedMultiLineString<VR>,
        counter: &mut BoundaryCounter<VR>,
    ) -> error::Result<()> {
        for &vertices_start_i in rings {
            let vertices_len = VertexIndex::<VR>::try_from(self.vertices.len())?;
            let vertices_end_i = self
                .rings
                .get(counter.try_increment_ring_idx()?.to_usize())
                .copied()
                .unwrap_or(vertices_len);
            if let Some(vertices) = self
                .vertices
                .get(vertices_start_i.to_usize()..vertices_end_i.to_usize())
            {
                surface.push(
                    vertices
                        .iter()
                        .map(super::vertex::VertexIndex::value)
                        .collect(),
                );
            }
        }
        Ok(())
    }

    /// Determines the type of boundary stored in this instance.
    ///
    /// This method examines the structure of the boundary to determine its type.
    /// The detection follows a hierarchical approach, prioritizing the most complex
    /// structure present.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::{Boundary, BoundaryType};
    /// use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiLineString32;
    ///
    /// // Create a boundary from a MultiLineString
    /// let multi_linestring: BoundaryNestedMultiLineString32 = vec![vec![0, 1, 2]];
    /// let boundary: Boundary<u32> = multi_linestring.try_into().unwrap();
    ///
    /// // Check type
    /// assert_eq!(boundary.check_type(), BoundaryType::MultiLineString);
    /// ```
    #[must_use]
    pub fn check_type(&self) -> BoundaryType {
        if !self.solids.is_empty() {
            BoundaryType::MultiOrCompositeSolid
        } else if !self.shells.is_empty() {
            BoundaryType::Solid
        } else if !self.surfaces.is_empty() {
            BoundaryType::MultiOrCompositeSurface
        } else if !self.rings.is_empty() {
            BoundaryType::MultiLineString
        } else if !self.vertices.is_empty() {
            BoundaryType::MultiPoint
        } else {
            BoundaryType::None
        }
    }

    /// Verifies that the internal representation of the boundary is consistent.
    ///
    /// This method checks that all indices are valid and that there are no dangling
    /// references. It ensures that:
    /// - Non-empty offset arrays start at zero
    /// - Offset arrays are monotonic
    /// - Ring indices point to valid vertices
    /// - Surface indices point to valid rings
    /// - Shell indices point to valid surfaces
    /// - Solid indices point to valid shells
    ///
    /// Empty segments are allowed and are represented by adjacent equal offsets.
    ///
    /// # Examples
    ///
    /// ```
    /// use cityjson_types::v2_0::{Boundary, BoundaryType};
    ///
    /// let boundary: Boundary<u32> = Boundary::new();
    /// assert!(boundary.is_consistent());
    /// ```
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        Self::offsets_are_consistent(&self.rings, self.vertices.len())
            && Self::offsets_are_consistent(&self.surfaces, self.rings.len())
            && Self::offsets_are_consistent(&self.shells, self.surfaces.len())
            && Self::offsets_are_consistent(&self.solids, self.shells.len())
    }
}

/// The type of a `CityJSON` boundary.
///
/// This enum represents the different types of boundaries that can be represented
/// in `CityJSON`. The types follow a hierarchy of complexity, from the simplest
/// (`MultiPoint`) to the most complex (`MultiOrCompositeSolid`).
///
/// # Examples
///
/// ```
/// use cityjson_types::v2_0::{Boundary, BoundaryType};
/// use cityjson_types::v2_0::boundary::nested::BoundaryNestedMultiPoint32;
///
/// // Create a boundary from a MultiPoint
/// let multi_point: BoundaryNestedMultiPoint32 = vec![0, 1, 2, 3];
/// let boundary: Boundary<u32> = multi_point.into();
///
/// // Check type
/// assert_eq!(boundary.check_type(), BoundaryType::MultiPoint);
/// ```
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[non_exhaustive]
pub enum BoundaryType {
    /// A collection of solids, possibly connected.
    MultiOrCompositeSolid,
    /// A single solid.
    Solid,
    /// A collection of surfaces, possibly connected.
    MultiOrCompositeSurface,
    /// A collection of line strings.
    MultiLineString,
    /// A collection of points.
    MultiPoint,
    /// An empty boundary.
    #[default]
    None,
}

impl std::fmt::Display for BoundaryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BoundaryType::MultiOrCompositeSolid => "MultiOrCompositeSolid",
            BoundaryType::Solid => "Solid",
            BoundaryType::MultiOrCompositeSurface => "MultiOrCompositeSurface",
            BoundaryType::MultiLineString => "MultiLineString",
            BoundaryType::MultiPoint => "MultiPoint",
            BoundaryType::None => "None",
        };
        write!(f, "{s}")
    }
}

/// A counter for tracking positions within different levels of a boundary hierarchy.
///
/// This struct is used internally during conversions between flattened and nested
/// representations to keep track of the current position in each level of the hierarchy.
#[derive(Default)]
pub(crate) struct BoundaryCounter<VR: VertexRef> {
    #[cfg(test)]
    pub(crate) vertex: VertexIndex<VR>, // Current position in vertex list
    pub(crate) ring: VertexIndex<VR>, // Current position in ring list
    pub(crate) surface: VertexIndex<VR>, // Current position in surface list
    pub(crate) shell: VertexIndex<VR>, // Current position in shell list
    pub(crate) solid: VertexIndex<VR>, // Current position in solid list
}

impl<VR: VertexRef> BoundaryCounter<VR> {
    #[inline]
    fn increment_checked(offset: &mut VertexIndex<VR>) -> error::Result<VertexIndex<VR>> {
        *offset = offset.next().ok_or_else(|| error::Error::IndexOverflow {
            index_type: std::any::type_name::<VR>().to_string(),
            value: offset.value().to_string(),
        })?;
        Ok(*offset)
    }

    #[cfg(test)]
    pub(crate) fn increment_vertex_idx(&mut self) -> VertexIndex<VR> {
        self.vertex += VertexIndex::new(VR::one());
        self.vertex
    }

    #[cfg(test)]
    pub(crate) fn increment_ring_idx(&mut self) -> VertexIndex<VR> {
        self.ring += VertexIndex::new(VR::one());
        self.ring
    }

    #[cfg(test)]
    pub(crate) fn increment_surface_idx(&mut self) -> VertexIndex<VR> {
        self.surface += VertexIndex::new(VR::one());
        self.surface
    }

    #[cfg(test)]
    pub(crate) fn increment_shell_idx(&mut self) -> VertexIndex<VR> {
        self.shell += VertexIndex::new(VR::one());
        self.shell
    }

    #[cfg(test)]
    pub(crate) fn increment_solid_idx(&mut self) -> VertexIndex<VR> {
        self.solid += VertexIndex::new(VR::one());
        self.solid
    }

    pub(crate) fn try_increment_ring_idx(&mut self) -> error::Result<VertexIndex<VR>> {
        Self::increment_checked(&mut self.ring)
    }

    pub(crate) fn try_increment_surface_idx(&mut self) -> error::Result<VertexIndex<VR>> {
        Self::increment_checked(&mut self.surface)
    }

    pub(crate) fn try_increment_shell_idx(&mut self) -> error::Result<VertexIndex<VR>> {
        Self::increment_checked(&mut self.shell)
    }

    pub(crate) fn try_increment_solid_idx(&mut self) -> error::Result<VertexIndex<VR>> {
        Self::increment_checked(&mut self.solid)
    }

    #[cfg(test)]
    pub(crate) fn vertex_offset(&self) -> VertexIndex<VR> {
        self.vertex
    }

    #[cfg(test)]
    pub(crate) fn ring_offset(&self) -> VertexIndex<VR> {
        self.ring
    }

    #[cfg(test)]
    pub(crate) fn surface_offset(&self) -> VertexIndex<VR> {
        self.surface
    }

    #[cfg(test)]
    pub(crate) fn shell_offset(&self) -> VertexIndex<VR> {
        self.shell
    }

    #[cfg(test)]
    pub(crate) fn solid_offset(&self) -> VertexIndex<VR> {
        self.solid
    }
}

#[cfg(test)]
mod tests;
