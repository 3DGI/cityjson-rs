//! Indexed coordinate storage for `CityJSON` vertex pools.

use crate::backend::default::coordinate::{RealWorldCoordinate, UVCoordinate};
use crate::cityjson::core::coordinate::Coordinate;
use crate::cityjson::core::vertex::{VertexIndex, VertexRef};
use crate::error::{Error, Result};
use std::fmt;
use std::marker::PhantomData;

/// Coordinate container with capacity limited by the vertex index type `VR`.
///
/// # Type Parameters
///
/// * `VR` — vertex index type (`u16`, `u32`, or `u64`), determines max capacity
/// * `V` — coordinate type implementing [`Coordinate`]
///
/// # Examples
///
/// ```
/// use cityjson_types::v2_0::{GeometryVertices16, RealWorldCoordinate};
///
/// let mut vertices = GeometryVertices16::new();
/// let index = vertices.push(RealWorldCoordinate::new(0.0, 0.0, 0.0)).unwrap();
/// let coord = vertices.get(index).unwrap();
/// assert_eq!(coord.x(), 0.0);
/// ```
#[repr(C)]
#[derive(Clone)]
pub struct Vertices<VR: VertexRef, V: Coordinate> {
    coordinates: Vec<V>,
    _phantom: PhantomData<VR>,
}

impl<VR: VertexRef, V: Coordinate> Vertices<VR, V> {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            coordinates: Vec::new(),
            _phantom: PhantomData,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            coordinates: Vec::with_capacity(capacity),
            _phantom: PhantomData,
        }
    }

    /// Reserves capacity for at least `additional_capacity` more elements to be inserted in the
    /// `Vertices`.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` _bytes_.
    ///
    /// # Errors
    ///
    /// Returns [`Error::VerticesContainerFull`] if the current or new capacity
    /// would exceed the maximum number of vertices representable by `VR`.
    #[inline]
    pub fn reserve(&mut self, additional_capacity: usize) -> Result<()> {
        let max = VR::MAX.try_into().unwrap_or(usize::MAX);
        if self.coordinates.len() >= max || self.coordinates.len() + additional_capacity > max {
            return Err(Error::VerticesContainerFull {
                attempted: self.coordinates.len() + 1,
                maximum: max,
            });
        }
        self.coordinates.reserve(additional_capacity);
        Ok(())
    }

    /// Returns the number of vertices in the collection.
    #[must_use]
    pub fn len(&self) -> usize {
        self.coordinates.len()
    }

    /// Adds a new coordinate to the collection.
    ///
    /// # Errors
    ///
    /// Returns [`Error::VerticesContainerFull`] if adding the coordinate would
    /// exceed the maximum number of vertices representable by `VR`.
    pub fn push(&mut self, coordinate: V) -> Result<VertexIndex<VR>> {
        if self.coordinates.len() >= VR::MAX.try_into().unwrap_or(usize::MAX) {
            return Err(Error::VerticesContainerFull {
                attempted: self.coordinates.len() + 1,
                maximum: VR::MAX.try_into().unwrap_or(usize::MAX),
            });
        }
        let index = VertexIndex::<VR>::try_from(self.coordinates.len())?;
        self.coordinates.push(coordinate);
        Ok(index)
    }

    /// Adds many coordinates at once and returns the contiguous index range assigned to them.
    ///
    /// The returned range is half-open: `start` is the index of the first inserted coordinate and
    /// `end` is one past the final inserted coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`Error::VerticesContainerFull`] if appending `coordinates` would exceed the
    /// maximum number of vertices representable by `VR`.
    pub fn extend_from_slice(
        &mut self,
        coordinates: &[V],
    ) -> Result<std::ops::Range<VertexIndex<VR>>> {
        let start = VertexIndex::<VR>::try_from(self.coordinates.len())?;
        let maximum = VR::MAX.try_into().unwrap_or(usize::MAX);
        let Some(new_len) = self.coordinates.len().checked_add(coordinates.len()) else {
            return Err(Error::VerticesContainerFull {
                attempted: usize::MAX,
                maximum,
            });
        };

        if new_len > maximum {
            return Err(Error::VerticesContainerFull {
                attempted: new_len,
                maximum,
            });
        }

        self.coordinates.reserve(coordinates.len());
        self.coordinates.extend_from_slice(coordinates);

        let end = VertexIndex::<VR>::try_from(self.coordinates.len())?;
        Ok(start..end)
    }

    /// Returns a reference to the coordinate at the specified index.
    #[inline]
    pub fn get(&self, index: VertexIndex<VR>) -> Option<&V> {
        self.coordinates.get(index.to_usize())
    }

    /// Returns true if the collection is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.coordinates.is_empty()
    }

    /// Returns a slice of all coordinates.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[V] {
        &self.coordinates
    }

    /// Returns a mutable slice of all coordinates.
    #[inline]
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [V] {
        &mut self.coordinates
    }

    /// Clears the collection, removing all vertices.
    #[inline]
    pub fn clear(&mut self) {
        self.coordinates.clear();
    }
}

impl<VR: VertexRef, V: Coordinate> fmt::Debug for Vertices<VR, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vertices")
            .field("len", &self.coordinates.len())
            .field("capacity", &self.coordinates.capacity())
            .field("max_vertices", &VR::MAX.try_into().unwrap_or(usize::MAX))
            .finish()
    }
}

impl<VR: VertexRef, V: Coordinate> fmt::Display for Vertices<VR, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Vertices[{}/{}]",
            self.coordinates.len(),
            VR::MAX.try_into().unwrap_or(usize::MAX)
        )
    }
}

impl<VR: VertexRef, V: Coordinate> Default for Vertices<VR, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VR: VertexRef, V: Coordinate> From<Vec<V>> for Vertices<VR, V> {
    fn from(value: Vec<V>) -> Self {
        Self {
            coordinates: value,
            _phantom: PhantomData,
        }
    }
}

impl<VR: VertexRef, V: Coordinate> From<&[V]> for Vertices<VR, V> {
    fn from(value: &[V]) -> Self {
        Self {
            coordinates: Vec::from(value),
            _phantom: PhantomData,
        }
    }
}

/// A collection of real-world coordinates with u16 indexing (up to 65,535 vertices)
pub type GeometryVertices16 = Vertices<u16, RealWorldCoordinate>;
/// A collection of real-world coordinates with u32 indexing (up to 4,294,967,295 vertices)
pub type GeometryVertices32 = Vertices<u32, RealWorldCoordinate>;
/// A collection of real-world coordinates with u64 indexing (virtually unlimited vertices)
pub type GeometryVertices64 = Vertices<u64, RealWorldCoordinate>;

/// A collection of UV texture coordinates with u16 indexing (up to 65,535 vertices)
pub type UVVertices16 = Vertices<u16, UVCoordinate>;
/// A collection of UV texture coordinates with u32 indexing (up to 4,294,967,295 vertices)
pub type UVVertices32 = Vertices<u32, UVCoordinate>;
/// A collection of UV texture coordinates with u64 indexing (virtually unlimited vertices)
pub type UVVertices64 = Vertices<u64, UVCoordinate>;
