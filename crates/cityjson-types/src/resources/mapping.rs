pub mod materials;
pub mod semantics;
pub mod textures;

use crate::resources::id::ResourceId;
use crate::v2_0::boundary::BoundaryType;
use crate::v2_0::vertex::VertexRef;

pub use materials::MaterialMap;
pub use semantics::SemanticMap;
pub use textures::TextureMap;

#[repr(C)]
#[derive(Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct SemanticOrMaterialMap<VR: VertexRef, RR: ResourceId> {
    pub(crate) points: Vec<Option<RR>>,
    pub(crate) linestrings: Vec<Option<RR>>,
    pub(crate) surfaces: Vec<Option<RR>>,
    // The boundary is authoritative for shell and solid topology.
    _phantom: std::marker::PhantomData<VR>,
}

impl<VR: VertexRef, RR: ResourceId> SemanticOrMaterialMap<VR, RR> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.points.is_empty() && self.linestrings.is_empty() && self.surfaces.is_empty()
    }

    pub(crate) fn add_point(&mut self, resource: Option<RR>) {
        self.points.push(resource);
    }

    pub(crate) fn add_linestring(&mut self, resource: Option<RR>) {
        self.linestrings.push(resource);
    }

    pub(crate) fn add_surface(&mut self, resource: Option<RR>) {
        self.surfaces.push(resource);
    }

    pub(crate) fn points(&self) -> &[Option<RR>] {
        &self.points
    }

    pub(crate) fn linestrings(&self) -> &[Option<RR>] {
        &self.linestrings
    }

    pub(crate) fn surfaces(&self) -> &[Option<RR>] {
        &self.surfaces
    }

    pub(crate) fn check_type(&self) -> BoundaryType {
        if !self.surfaces.is_empty() {
            BoundaryType::MultiOrCompositeSurface
        } else if !self.linestrings.is_empty() {
            BoundaryType::MultiLineString
        } else if !self.points.is_empty() {
            BoundaryType::MultiPoint
        } else {
            BoundaryType::None
        }
    }
}

macro_rules! define_typed_resource_map {
    ($name:ident, $handle:ty) => {
        #[derive(Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
        pub struct $name<VR: crate::v2_0::vertex::VertexRef> {
            inner: crate::resources::mapping::SemanticOrMaterialMap<
                VR,
                crate::resources::id::ResourceId32,
            >,
        }

        impl<VR: crate::v2_0::vertex::VertexRef> $name<VR> {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn is_empty(&self) -> bool {
                self.inner.is_empty()
            }

            pub fn add_point(&mut self, resource: Option<$handle>) {
                self.inner.add_point(resource.map(|r| r.to_raw()));
            }

            pub fn add_linestring(&mut self, resource: Option<$handle>) {
                self.inner.add_linestring(resource.map(|r| r.to_raw()));
            }

            pub fn add_surface(&mut self, resource: Option<$handle>) {
                self.inner.add_surface(resource.map(|r| r.to_raw()));
            }

            pub fn points(&self) -> &[Option<$handle>] {
                crate::resources::handles::cast_option_handle_slice::<$handle>(self.inner.points())
            }

            pub fn linestrings(&self) -> &[Option<$handle>] {
                crate::resources::handles::cast_option_handle_slice::<$handle>(
                    self.inner.linestrings(),
                )
            }

            pub fn surfaces(&self) -> &[Option<$handle>] {
                crate::resources::handles::cast_option_handle_slice::<$handle>(
                    self.inner.surfaces(),
                )
            }

            pub fn check_type(&self) -> crate::v2_0::boundary::BoundaryType {
                self.inner.check_type()
            }

            #[allow(dead_code)]
            pub(crate) fn from_raw(
                inner: crate::resources::mapping::SemanticOrMaterialMap<
                    VR,
                    crate::resources::id::ResourceId32,
                >,
            ) -> Self {
                Self { inner }
            }

            #[allow(dead_code)]
            pub(crate) fn into_raw(
                self,
            ) -> crate::resources::mapping::SemanticOrMaterialMap<
                VR,
                crate::resources::id::ResourceId32,
            > {
                self.inner
            }

            #[allow(dead_code)]
            pub(crate) fn to_raw(
                &self,
            ) -> &crate::resources::mapping::SemanticOrMaterialMap<
                VR,
                crate::resources::id::ResourceId32,
            > {
                &self.inner
            }
        }
    };
}

pub(crate) use define_typed_resource_map;

// ---------------------------------------------------------------------------
// Unit tests for SemanticOrMaterialMap
// Family 5: semantic/material map shape
// Family 6: map check_type() correctness
// ---------------------------------------------------------------------------

#[cfg(test)]
mod semantic_material_map {
    use super::*;
    use crate::resources::id::ResourceId32;

    type Map = SemanticOrMaterialMap<u32, ResourceId32>;

    fn make_surface_map() -> Map {
        let mut map = Map::new();
        map.add_surface(Some(ResourceId32::new(0, 0)));
        map.add_surface(Some(ResourceId32::new(1, 0)));
        map.add_surface(None);
        map
    }

    /// Inputs: point, linestring, and surface semantic/material maps with one
    /// populated bucket. Assertions: only the selected bucket is populated,
    /// bucket length is dense, and `None` assignments are preserved. Purpose:
    /// compact positive coverage for map bucket population.
    #[test]
    fn semantic_material_map_populates_only_selected_bucket() {
        let mut point_map = Map::new();
        point_map.add_point(Some(ResourceId32::new(0, 0)));
        point_map.add_point(None);
        assert_eq!(point_map.points().len(), 2);
        assert!(point_map.linestrings().is_empty());
        assert!(point_map.surfaces().is_empty());
        assert!(point_map.points()[1].is_none());

        let mut line_map = Map::new();
        line_map.add_linestring(None);
        line_map.add_linestring(Some(ResourceId32::new(7, 0)));
        assert!(line_map.points().is_empty());
        assert_eq!(line_map.linestrings().len(), 2);
        assert!(line_map.surfaces().is_empty());
        assert!(line_map.linestrings()[0].is_none());

        let surface_map = make_surface_map();
        assert!(surface_map.points().is_empty());
        assert!(surface_map.linestrings().is_empty());
        assert_eq!(surface_map.surfaces().len(), 3);
        assert!(surface_map.surfaces()[2].is_none());
    }

    /// Inputs: empty, point-only, linestring-only, and surface-only maps.
    /// Assertions: `check_type()` follows the highest non-empty bucket. Purpose:
    /// define map type detection independently from geometry validation.
    #[test]
    fn semantic_material_map_check_type_follows_highest_non_empty_bucket() {
        assert_eq!(Map::new().check_type(), BoundaryType::None);

        let mut point_map = Map::new();
        point_map.add_point(None);
        assert_eq!(point_map.check_type(), BoundaryType::MultiPoint);

        let mut line_map = Map::new();
        line_map.add_linestring(None);
        assert_eq!(line_map.check_type(), BoundaryType::MultiLineString);

        let surface_map = make_surface_map();
        assert_eq!(
            surface_map.check_type(),
            BoundaryType::MultiOrCompositeSurface
        );
    }
}
