//! The root `CityJSON` object.
//!
//! [`CityModel`] holds everything in a `CityJSON` document: city objects, vertices, geometries,
//! and the shared resource pools for semantics, materials, and textures.
//!
//! ## City objects and geometry
//!
//! City objects are stored in [`CityObjects`] and reference geometries by [`GeometryHandle`].
//! Add a city object with [`CityModel::cityobjects_mut`], then attach geometries built with
//! [`GeometryDraft`](super::geometry_draft::GeometryDraft).
//!
//! ```rust
//! use cityjson_types::CityModelType;
//! use cityjson_types::v2_0::{
//!     CityObject, CityObjectIdentifier, CityObjectType, GeometryDraft, OwnedCityModel, PointDraft,
//!     RealWorldCoordinate,
//! };
//!
//! let mut model = OwnedCityModel::new(CityModelType::CityJSON);
//!
//! // Add a city object.
//! let mut tree = CityObject::new(
//!     CityObjectIdentifier::new("tree-1".to_string()),
//!     CityObjectType::SolitaryVegetationObject,
//! );
//!
//! // Build and attach a geometry.
//! let geom = GeometryDraft::multi_point(
//!     None,
//!     [PointDraft::new(RealWorldCoordinate::new(84710.0, 446900.0, 5.0))],
//! )
//! .insert_into(&mut model)
//! .unwrap();
//!
//! tree.add_geometry(geom);
//! model.cityobjects_mut().add(tree).unwrap();
//! ```
//!
//! ## Resource pools
//!
//! Semantics, materials, and textures are stored once in the model and referenced by handle.
//! Use [`CityModel::add_semantic`], [`CityModel::add_material`], and [`CityModel::add_texture`]
//! to register resources; use [`CityModel::get_or_insert_semantic`] etc. to deduplicate.
//!
//! ## Generics
//!
//! `CityModel<VR, SS>` is generic over the vertex index type (`VR: VertexRef`, e.g. `u32`) and
//! string storage (`SS: StringStorage`). `OwnedCityModel` and `BorrowedCityModel` are the
//! common aliases.
use crate::backend::default::citymodel::{CityModelCore, CityModelCoreCapacities};
use crate::backend::default::geometry_validation::{
    BoundaryVertexSource, GeometryValidationContext, validate_stored_geometry,
    validate_stored_geometry_for_boundary_source,
};
use crate::cityjson::core::appearance::ThemeName;
use crate::error::Error;
use crate::error::Result;
use crate::raw::{RawAccess, RawPoolView, RawSliceView};
use crate::resources::handles::{
    CityObjectHandle, GeometryHandle, GeometryTemplateHandle, MaterialHandle, SemanticHandle,
    TextureHandle,
};
use crate::resources::id::ResourceId32;
use crate::resources::storage::{BorrowedStringStorage, OwnedStringStorage, StringStorage};
use crate::v2_0::appearance::material::Material;
use crate::v2_0::appearance::texture::Texture;
use crate::v2_0::attributes::AttributeValue;
use crate::v2_0::coordinate::{RealWorldCoordinate, UVCoordinate};
use crate::v2_0::extension::Extensions;
use crate::v2_0::geometry::GeometryView;
use crate::v2_0::geometry::semantic::Semantic;
use crate::v2_0::geometry::{Geometry, GeometryType};
use crate::v2_0::metadata::{BBox, Metadata};
use crate::v2_0::transform::Transform;
use crate::v2_0::vertex::{VertexIndex, VertexRef};
use crate::v2_0::vertices::Vertices;
use crate::v2_0::{CityObject, CityObjects};
use crate::{CityJSONVersion, format_option};
use std::collections::HashSet;
use std::fmt;

fn invalid_reference(element_type: &'static str, index: usize, len: usize) -> Error {
    Error::InvalidReference {
        element_type: element_type.to_string(),
        index,
        max_index: len.saturating_sub(1),
    }
}

fn union_bbox(lhs: BBox, rhs: BBox) -> BBox {
    BBox::new(
        lhs.min_x().min(rhs.min_x()),
        lhs.min_y().min(rhs.min_y()),
        lhs.min_z().min(rhs.min_z()),
        lhs.max_x().max(rhs.max_x()),
        lhs.max_y().max(rhs.max_y()),
        lhs.max_z().max(rhs.max_z()),
    )
}

fn include_point(extent: &mut Option<BBox>, x: f64, y: f64, z: f64) {
    let point = BBox::new(x, y, z, x, y, z);
    *extent = Some(match *extent {
        Some(existing) => union_bbox(existing, point),
        None => point,
    });
}

/// `CityModel` with owned string storage and 32-bit vertex indices.
pub type OwnedCityModel = CityModel<u32, OwnedStringStorage>;

/// `CityModel` with borrowed string storage and 32-bit vertex indices.
pub type BorrowedCityModel<'a> = CityModel<u32, BorrowedStringStorage<'a>>;

/// Pre-allocation hints for [`CityModel::with_capacities`].
///
/// All fields are optional — set only what you know in advance. Unused fields default to zero.
#[derive(Debug, Clone, Copy, Default)]
pub struct CityModelCapacities {
    pub cityobjects: usize,
    pub vertices: usize,
    pub semantics: usize,
    pub materials: usize,
    pub textures: usize,
    pub geometries: usize,
    pub template_vertices: usize,
    pub template_geometries: usize,
    pub uv_coordinates: usize,
}

impl From<CityModelCapacities> for CityModelCoreCapacities {
    fn from(value: CityModelCapacities) -> Self {
        Self {
            cityobjects: value.cityobjects,
            vertices: value.vertices,
            semantics: value.semantics,
            materials: value.materials,
            textures: value.textures,
            geometries: value.geometries,
            template_vertices: value.template_vertices,
            template_geometries: value.template_geometries,
            uv_coordinates: value.uv_coordinates,
        }
    }
}

/// The root `CityJSON` object.
///
/// Holds all vertices, city objects, and shared resource pools. See the [module docs](self)
/// for usage examples.
#[derive(Debug, Clone)]
pub struct CityModel<VR: VertexRef = u32, SS: StringStorage = OwnedStringStorage> {
    #[allow(clippy::type_complexity)]
    inner: CityModelCore<
        VR,
        ResourceId32,
        SS,
        Semantic<SS>,
        Material<SS>,
        Texture<SS>,
        Geometry<VR, SS>,
        Metadata<SS>,
        Transform,
        Extensions<SS>,
        CityObjects<SS>,
    >,
}

impl<VR: VertexRef, SS: StringStorage> CityModel<VR, SS> {
    #[must_use]
    pub fn new(type_citymodel: crate::CityModelType) -> Self {
        Self {
            inner: CityModelCore::new(type_citymodel, Some(CityJSONVersion::V2_0)),
        }
    }

    #[must_use]
    pub fn with_capacities(
        type_citymodel: crate::CityModelType,
        capacities: CityModelCapacities,
    ) -> Self {
        Self {
            inner: CityModelCore::with_capacities(
                type_citymodel,
                Some(CityJSONVersion::V2_0),
                capacities.into(),
                CityObjects::with_capacity,
            ),
        }
    }

    pub fn get_semantic(&self, id: SemanticHandle) -> Option<&Semantic<SS>> {
        self.inner.get_semantic(id.to_raw())
    }

    pub fn get_semantic_mut(&mut self, id: SemanticHandle) -> Option<&mut Semantic<SS>> {
        self.inner.get_semantic_mut(id.to_raw())
    }

    /// Add a semantic and return its handle.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the semantic pool cannot store
    /// additional entries for `ResourceId32`.
    pub fn add_semantic(&mut self, semantic: Semantic<SS>) -> Result<SemanticHandle> {
        self.inner
            .add_semantic(semantic)
            .map(SemanticHandle::from_raw)
    }

    pub fn semantic_count(&self) -> usize {
        self.inner.semantic_count()
    }

    pub fn has_semantics(&self) -> bool {
        self.inner.has_semantics()
    }

    pub fn iter_semantics(&self) -> impl Iterator<Item = (SemanticHandle, &Semantic<SS>)> + '_ {
        self.inner
            .iter_semantics()
            .map(|(id, v)| (SemanticHandle::from_raw(id), v))
    }

    pub fn iter_semantics_mut(
        &mut self,
    ) -> impl Iterator<Item = (SemanticHandle, &mut Semantic<SS>)> + '_ {
        self.inner
            .iter_semantics_mut()
            .map(|(id, v)| (SemanticHandle::from_raw(id), v))
    }

    pub fn find_semantic(&self, semantic: &Semantic<SS>) -> Option<SemanticHandle>
    where
        Semantic<SS>: PartialEq,
    {
        self.inner
            .find_semantic(semantic)
            .map(SemanticHandle::from_raw)
    }

    #[cfg(test)]
    pub(crate) fn remove_semantic(&mut self, id: SemanticHandle) -> Option<Semantic<SS>> {
        self.inner.remove_semantic(id.to_raw())
    }

    /// Return an existing semantic handle or insert a new semantic.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when inserting a new semantic exceeds
    /// the semantic pool capacity.
    pub fn get_or_insert_semantic(&mut self, semantic: Semantic<SS>) -> Result<SemanticHandle>
    where
        Semantic<SS>: PartialEq,
    {
        self.inner
            .get_or_insert_semantic(semantic)
            .map(SemanticHandle::from_raw)
    }

    pub fn get_material(&self, id: MaterialHandle) -> Option<&Material<SS>> {
        self.inner.get_material(id.to_raw())
    }

    pub fn get_material_mut(&mut self, id: MaterialHandle) -> Option<&mut Material<SS>> {
        self.inner.get_material_mut(id.to_raw())
    }

    /// Add a material and return its handle.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the material pool cannot store
    /// additional entries for `ResourceId32`.
    pub fn add_material(&mut self, material: Material<SS>) -> Result<MaterialHandle> {
        self.inner
            .add_material(material)
            .map(MaterialHandle::from_raw)
    }

    pub fn material_count(&self) -> usize {
        self.inner.material_count()
    }

    pub fn iter_materials(&self) -> impl Iterator<Item = (MaterialHandle, &Material<SS>)> + '_ {
        self.inner
            .iter_materials()
            .map(|(id, v)| (MaterialHandle::from_raw(id), v))
    }

    pub fn iter_materials_mut(
        &mut self,
    ) -> impl Iterator<Item = (MaterialHandle, &mut Material<SS>)> + '_ {
        self.inner
            .iter_materials_mut()
            .map(|(id, v)| (MaterialHandle::from_raw(id), v))
    }

    pub fn find_material(&self, material: &Material<SS>) -> Option<MaterialHandle>
    where
        Material<SS>: PartialEq,
    {
        self.inner
            .find_material(material)
            .map(MaterialHandle::from_raw)
    }

    #[cfg(test)]
    pub(crate) fn remove_material(&mut self, id: MaterialHandle) -> Option<Material<SS>> {
        self.inner.remove_material(id.to_raw())
    }

    /// Return an existing material handle or insert a new material.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when inserting a new material exceeds
    /// the material pool capacity.
    pub fn get_or_insert_material(&mut self, material: Material<SS>) -> Result<MaterialHandle>
    where
        Material<SS>: PartialEq,
    {
        self.inner
            .get_or_insert_material(material)
            .map(MaterialHandle::from_raw)
    }

    pub fn get_texture(&self, id: TextureHandle) -> Option<&Texture<SS>> {
        self.inner.get_texture(id.to_raw())
    }

    pub fn get_texture_mut(&mut self, id: TextureHandle) -> Option<&mut Texture<SS>> {
        self.inner.get_texture_mut(id.to_raw())
    }

    /// Add a texture and return its handle.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the texture pool cannot store
    /// additional entries for `ResourceId32`.
    pub fn add_texture(&mut self, texture: Texture<SS>) -> Result<TextureHandle> {
        self.inner.add_texture(texture).map(TextureHandle::from_raw)
    }

    pub fn texture_count(&self) -> usize {
        self.inner.texture_count()
    }

    pub fn iter_textures(&self) -> impl Iterator<Item = (TextureHandle, &Texture<SS>)> + '_ {
        self.inner
            .iter_textures()
            .map(|(id, v)| (TextureHandle::from_raw(id), v))
    }

    pub fn iter_textures_mut(
        &mut self,
    ) -> impl Iterator<Item = (TextureHandle, &mut Texture<SS>)> + '_ {
        self.inner
            .iter_textures_mut()
            .map(|(id, v)| (TextureHandle::from_raw(id), v))
    }

    pub fn find_texture(&self, texture: &Texture<SS>) -> Option<TextureHandle>
    where
        Texture<SS>: PartialEq,
    {
        self.inner
            .find_texture(texture)
            .map(TextureHandle::from_raw)
    }

    #[cfg(test)]
    pub(crate) fn remove_texture(&mut self, id: TextureHandle) -> Option<Texture<SS>> {
        self.inner.remove_texture(id.to_raw())
    }

    /// Return an existing texture handle or insert a new texture.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when inserting a new texture exceeds
    /// the texture pool capacity.
    pub fn get_or_insert_texture(&mut self, texture: Texture<SS>) -> Result<TextureHandle>
    where
        Texture<SS>: PartialEq,
    {
        self.inner
            .get_or_insert_texture(texture)
            .map(TextureHandle::from_raw)
    }

    pub fn get_geometry(&self, id: GeometryHandle) -> Option<&Geometry<VR, SS>> {
        self.inner.get_geometry(id.to_raw())
    }

    /// Resolve a geometry handle to the effective geometry view.
    ///
    /// # Errors
    ///
    /// Returns an error when the geometry handle is invalid or when a
    /// `GeometryInstance` references a missing geometry template.
    pub fn resolve_geometry(&self, id: GeometryHandle) -> Result<GeometryView<'_, VR, SS>> {
        let geometry =
            self.get_geometry(id)
                .ok_or_else(|| crate::error::Error::InvalidReference {
                    element_type: "geometry".to_string(),
                    index: id.to_raw().index() as usize,
                    max_index: self.geometry_count().saturating_sub(1),
                })?;

        if let Some(instance) = geometry.instance() {
            let template = self
                .get_geometry_template(instance.template())
                .ok_or_else(|| crate::error::Error::InvalidReference {
                    element_type: "template geometry".to_string(),
                    index: instance.template().to_raw().index() as usize,
                    max_index: self.geometry_template_count().saturating_sub(1),
                })?;
            Ok(GeometryView::from_geometry(template, Some(instance)))
        } else {
            Ok(GeometryView::from_geometry(geometry, None))
        }
    }

    /// Calculate the geographical extent from all city objects with directly attached geometry.
    ///
    /// Stored `geographicalExtent` values on metadata or city objects are ignored. The returned
    /// extent is not stored back into the model.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::InvalidReference`] when an attached geometry, boundary
    /// vertex, geometry template, or instance reference point is missing.
    pub fn calculate_geographical_extent(&self) -> Result<Option<BBox>> {
        let mut extent = None;

        for (_, cityobject) in self.cityobjects().iter() {
            if let Some(cityobject_extent) = self.calculate_cityobject_extent(cityobject)? {
                extent = Some(match extent {
                    Some(existing) => union_bbox(existing, cityobject_extent),
                    None => cityobject_extent,
                });
            }
        }

        Ok(extent)
    }

    /// Calculate the geographical extent for one city object from its directly attached geometry.
    ///
    /// Stored `geographicalExtent` values on the city object are ignored. The returned extent is
    /// not stored back into the city object.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::InvalidReference`] when `handle` is missing or when an
    /// attached geometry, boundary vertex, geometry template, or instance reference point is
    /// missing.
    pub fn calculate_cityobject_geographical_extent(
        &self,
        handle: CityObjectHandle,
    ) -> Result<Option<BBox>> {
        let cityobject = self.cityobjects().get(handle).ok_or_else(|| {
            let (index, _) = handle.raw_parts();
            invalid_reference("city object", index as usize, self.cityobjects().len())
        })?;

        self.calculate_cityobject_extent(cityobject)
    }

    fn calculate_cityobject_extent(&self, cityobject: &CityObject<SS>) -> Result<Option<BBox>> {
        let mut extent = None;

        for handle in cityobject.geometry().unwrap_or(&[]) {
            let geometry = self.get_geometry(*handle).ok_or_else(|| {
                let (index, _) = handle.raw_parts();
                invalid_reference("geometry", index as usize, self.geometry_count())
            })?;

            if let Some(geometry_extent) = self.calculate_geometry_extent(geometry)? {
                extent = Some(match extent {
                    Some(existing) => union_bbox(existing, geometry_extent),
                    None => geometry_extent,
                });
            }
        }

        Ok(extent)
    }

    fn calculate_geometry_extent(&self, geometry: &Geometry<VR, SS>) -> Result<Option<BBox>> {
        if let Some(instance) = geometry.instance() {
            return self.calculate_geometry_instance_extent(instance);
        }

        Self::calculate_regular_geometry_extent(geometry, self.vertices(), "vertex")
    }

    fn calculate_geometry_instance_extent(
        &self,
        instance: crate::v2_0::geometry::GeometryInstanceView<'_, VR>,
    ) -> Result<Option<BBox>> {
        let template = self
            .get_geometry_template(instance.template())
            .ok_or_else(|| {
                let (index, _) = instance.template().raw_parts();
                invalid_reference(
                    "template geometry",
                    index as usize,
                    self.geometry_template_count(),
                )
            })?;

        let reference_point = self.get_vertex(instance.reference_point()).ok_or_else(|| {
            invalid_reference(
                "vertex",
                instance
                    .reference_point()
                    .try_to_usize()
                    .unwrap_or(usize::MAX),
                self.vertices().len(),
            )
        })?;

        let Some(boundary) = template.boundaries() else {
            return Ok(None);
        };

        let matrix = instance.transformation();
        let matrix = matrix.as_array();
        let mut extent = None;

        for vertex_index in boundary.vertices() {
            let coordinate = self.template_vertices().get(*vertex_index).ok_or_else(|| {
                invalid_reference(
                    "template vertex",
                    vertex_index.try_to_usize().unwrap_or(usize::MAX),
                    self.template_vertices().len(),
                )
            })?;

            let x = matrix[0] * coordinate.x()
                + matrix[1] * coordinate.y()
                + matrix[2] * coordinate.z()
                + matrix[3]
                + reference_point.x();
            let y = matrix[4] * coordinate.x()
                + matrix[5] * coordinate.y()
                + matrix[6] * coordinate.z()
                + matrix[7]
                + reference_point.y();
            let z = matrix[8] * coordinate.x()
                + matrix[9] * coordinate.y()
                + matrix[10] * coordinate.z()
                + matrix[11]
                + reference_point.z();

            include_point(&mut extent, x, y, z);
        }

        Ok(extent)
    }

    fn calculate_regular_geometry_extent(
        geometry: &Geometry<VR, SS>,
        vertices: &Vertices<VR, RealWorldCoordinate>,
        vertex_element_type: &'static str,
    ) -> Result<Option<BBox>> {
        let Some(boundary) = geometry.boundaries() else {
            return Ok(None);
        };

        let mut extent = None;

        for vertex_index in boundary.vertices() {
            let coordinate = vertices.get(*vertex_index).ok_or_else(|| {
                invalid_reference(
                    vertex_element_type,
                    vertex_index.try_to_usize().unwrap_or(usize::MAX),
                    vertices.len(),
                )
            })?;

            include_point(&mut extent, coordinate.x(), coordinate.y(), coordinate.z());
        }

        Ok(extent)
    }

    /// Add a geometry and return its handle.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::InvalidGeometry`] when `geometry` violates the stored
    /// geometry invariants, or [`crate::error::Error::ResourcePoolFull`] when the geometry pool
    /// cannot store additional entries for `ResourceId32`.
    pub fn add_geometry(&mut self, geometry: Geometry<VR, SS>) -> Result<GeometryHandle> {
        validate_stored_geometry(geometry.raw(), self)?;
        self.add_geometry_unchecked(geometry)
    }

    /// Add a geometry without validating stored-geometry invariants.
    ///
    /// Callers must ensure that `geometry` satisfies the same invariants currently enforced by
    /// [`CityModel::add_geometry`]. In particular, boundary vertex indices must reference the
    /// correct vertex pool and the geometry must be valid for regular geometry insertion.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the geometry pool cannot store
    /// additional entries for `ResourceId32`.
    pub fn add_geometry_unchecked(&mut self, geometry: Geometry<VR, SS>) -> Result<GeometryHandle> {
        self.inner
            .add_geometry(geometry)
            .map(GeometryHandle::from_raw)
    }

    pub fn geometry_count(&self) -> usize {
        self.inner.geometry_count()
    }

    pub fn iter_geometries(
        &self,
    ) -> impl Iterator<Item = (GeometryHandle, &Geometry<VR, SS>)> + '_ {
        self.inner
            .iter_geometries()
            .map(|(id, v)| (GeometryHandle::from_raw(id), v))
    }

    pub fn vertices(&self) -> &Vertices<VR, RealWorldCoordinate> {
        self.inner.vertices()
    }

    pub fn vertices_mut(&mut self) -> &mut Vertices<VR, RealWorldCoordinate> {
        self.inner.vertices_mut()
    }

    /// Add a vertex and return its index.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the vertex
    /// container cannot represent more vertices for `VR`.
    pub fn add_vertex(
        &mut self,
        coordinate: RealWorldCoordinate,
    ) -> crate::error::Result<VertexIndex<VR>> {
        self.inner.add_vertex(coordinate)
    }

    /// Add many vertices and return the contiguous index range assigned to them.
    ///
    /// The returned range is half-open: `start` is the first inserted vertex index and `end` is
    /// one past the final inserted vertex.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the vertex
    /// container cannot represent more vertices for `VR`.
    pub fn add_vertices(
        &mut self,
        coordinates: &[RealWorldCoordinate],
    ) -> crate::error::Result<std::ops::Range<VertexIndex<VR>>> {
        self.inner.add_vertices(coordinates)
    }

    pub fn get_vertex(&self, index: VertexIndex<VR>) -> Option<&RealWorldCoordinate> {
        self.inner.get_vertex(index)
    }

    pub fn metadata(&self) -> Option<&Metadata<SS>> {
        self.inner.metadata()
    }

    pub fn metadata_mut(&mut self) -> &mut Metadata<SS> {
        self.inner.metadata_mut()
    }

    /// Typed feature root id.
    ///
    /// This is semantically meaningful for `CityJSONFeature`, where it identifies the
    /// main feature `CityObject`. For plain `CityJSON`, root `id` remains an ordinary
    /// extra property and this accessor returns `None`.
    pub fn id(&self) -> Option<CityObjectHandle> {
        self.inner.id().map(CityObjectHandle::from_raw)
    }

    pub fn set_id(&mut self, id: Option<CityObjectHandle>) {
        self.inner.set_id(id.map(CityObjectHandle::to_raw));
    }

    pub fn extra(&self) -> Option<&crate::v2_0::attributes::Attributes<SS>> {
        self.inner.extra()
    }

    pub fn extra_mut(&mut self) -> &mut crate::v2_0::attributes::Attributes<SS> {
        self.inner.extra_mut()
    }

    pub fn transform(&self) -> Option<&Transform> {
        self.inner.transform()
    }

    pub fn transform_mut(&mut self) -> &mut Transform {
        self.inner.transform_mut()
    }

    pub fn clear_transform(&mut self) {
        self.inner.clear_transform();
    }

    pub fn extensions(&self) -> Option<&Extensions<SS>> {
        self.inner.extensions()
    }

    pub fn extensions_mut(&mut self) -> &mut Extensions<SS> {
        self.inner.extensions_mut()
    }

    pub fn cityobjects(&self) -> &CityObjects<SS> {
        self.inner.cityobjects()
    }

    /// Returns a raw accessor for zero-copy reads of internal model pools.
    #[inline]
    pub fn raw(&self) -> crate::raw::v2_0::CityModelRawAccessor<'_, VR, SS> {
        crate::raw::v2_0::CityModelRawAccessor::new(self)
    }

    /// Returns mutable access to the city object collection.
    ///
    /// This remains public because object authoring still happens through live
    /// mutation. It can still create dangling geometry or object references; the
    /// stricter guarantees in this module apply to stored geometry insertion,
    /// not to arbitrary city object graph edits.
    pub fn cityobjects_mut(&mut self) -> &mut CityObjects<SS> {
        self.inner.cityobjects_mut()
    }

    pub fn clear_cityobjects(&mut self) {
        self.inner.cityobjects_mut().clear();
    }

    /// Add a UV coordinate and return its vertex index.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the UV-coordinate container
    /// cannot represent more vertices for `VR`.
    pub fn add_uv_coordinate(
        &mut self,
        uvcoordinate: UVCoordinate,
    ) -> crate::error::Result<VertexIndex<VR>> {
        self.inner.add_uv_coordinate(uvcoordinate)
    }

    pub fn get_uv_coordinate(&self, index: VertexIndex<VR>) -> Option<&UVCoordinate> {
        self.inner.get_uv_coordinate(index)
    }

    pub fn vertices_texture(&self) -> &Vertices<VR, UVCoordinate> {
        self.inner.vertices_texture()
    }

    /// Add a template vertex and return its index.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the template-vertex
    /// container cannot represent more vertices for `VR`.
    pub fn add_template_vertex(
        &mut self,
        coordinate: RealWorldCoordinate,
    ) -> crate::error::Result<VertexIndex<VR>> {
        self.inner.add_template_vertex(coordinate)
    }

    pub fn get_template_vertex(&self, index: VertexIndex<VR>) -> Option<&RealWorldCoordinate> {
        self.inner.get_template_vertex(index)
    }

    pub fn template_vertices(&self) -> &Vertices<VR, RealWorldCoordinate> {
        self.inner.template_vertices()
    }

    pub fn template_vertices_mut(&mut self) -> &mut Vertices<VR, RealWorldCoordinate> {
        self.inner.template_vertices_mut()
    }

    pub fn get_geometry_template(&self, id: GeometryTemplateHandle) -> Option<&Geometry<VR, SS>> {
        self.inner.get_template_geometry(id.to_raw())
    }

    /// Add a template geometry and return its handle.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::InvalidGeometry`] when `geometry` violates the stored
    /// template-geometry invariants, or [`crate::error::Error::ResourcePoolFull`] when the
    /// template-geometry pool cannot store additional entries for `ResourceId32`.
    pub fn add_geometry_template(
        &mut self,
        geometry: Geometry<VR, SS>,
    ) -> Result<GeometryTemplateHandle> {
        if *geometry.type_geometry() == GeometryType::GeometryInstance {
            return Err(crate::error::Error::InvalidGeometry(
                "GeometryInstance cannot be inserted into the template geometry pool".to_string(),
            ));
        }

        validate_stored_geometry_for_boundary_source(
            geometry.raw(),
            self,
            BoundaryVertexSource::Template,
        )?;
        self.add_geometry_template_unchecked(geometry)
    }

    /// Add a template geometry without validating stored-geometry invariants.
    ///
    /// Callers must ensure that `geometry` satisfies the same invariants currently enforced by
    /// [`CityModel::add_geometry_template`], and that it is not a `GeometryInstance`.
    /// Boundary vertex indices must reference the template vertex pool.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the template-geometry pool cannot
    /// store additional entries for `ResourceId32`.
    pub fn add_geometry_template_unchecked(
        &mut self,
        geometry: Geometry<VR, SS>,
    ) -> Result<GeometryTemplateHandle> {
        self.inner
            .add_template_geometry(geometry)
            .map(GeometryTemplateHandle::from_raw)
    }

    pub fn geometry_template_count(&self) -> usize {
        self.inner.template_geometry_count()
    }

    pub fn iter_geometry_templates(
        &self,
    ) -> impl Iterator<Item = (GeometryTemplateHandle, &Geometry<VR, SS>)> + '_ {
        self.inner
            .iter_template_geometries()
            .map(|(id, v)| (GeometryTemplateHandle::from_raw(id), v))
    }

    pub fn type_citymodel(&self) -> crate::CityModelType {
        self.inner.type_citymodel()
    }

    pub fn version(&self) -> Option<crate::CityJSONVersion> {
        self.inner.version()
    }

    /// Reserve capacity for bulk import workloads.
    ///
    /// This reserves all pools that can be expanded after construction. `cityobjects` uses the
    /// same runtime reserve path as the other pools, while the remaining fields reserve vertices,
    /// geometries, templates, and appearance resources.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when any pool would exceed the
    /// representable size for `ResourceId32`.
    pub fn reserve_import(&mut self, capacities: CityModelCapacities) -> Result<()> {
        self.cityobjects_mut().reserve(capacities.cityobjects)?;
        self.inner.reserve_vertex_capacity(capacities.vertices)?;
        self.inner
            .reserve_geometry_capacity(capacities.geometries)?;
        self.inner
            .reserve_template_vertex_capacity(capacities.template_vertices)?;
        self.inner
            .reserve_template_geometry_capacity(capacities.template_geometries)?;
        self.inner.reserve_semantic_capacity(capacities.semantics)?;
        self.inner.reserve_material_capacity(capacities.materials)?;
        self.inner.reserve_texture_capacity(capacities.textures)?;
        self.inner.reserve_uv_capacity(capacities.uv_coordinates)
    }

    pub(crate) fn reserve_draft_insert(
        &mut self,
        mode: super::geometry_draft::DraftInsertMode,
        new_vertices: usize,
        new_uvs: usize,
    ) -> Result<()> {
        let capacities = match mode {
            super::geometry_draft::DraftInsertMode::Regular => CityModelCapacities {
                geometries: 1,
                vertices: new_vertices,
                uv_coordinates: new_uvs,
                ..CityModelCapacities::default()
            },
            super::geometry_draft::DraftInsertMode::Template => CityModelCapacities {
                template_geometries: 1,
                template_vertices: new_vertices,
                uv_coordinates: new_uvs,
                ..CityModelCapacities::default()
            },
        };
        self.reserve_import(capacities)
    }

    pub fn default_material_theme(&self) -> Option<&ThemeName<SS>> {
        self.inner.default_material_theme()
    }

    pub fn set_default_material_theme(&mut self, theme: Option<ThemeName<SS>>) {
        self.inner.set_default_material_theme(theme);
    }

    pub fn default_texture_theme(&self) -> Option<&ThemeName<SS>> {
        self.inner.default_texture_theme()
    }

    pub fn set_default_texture_theme(&mut self, theme: Option<ThemeName<SS>>) {
        self.inner.set_default_texture_theme(theme);
    }

    pub fn has_material_theme(&self, theme: &str) -> bool {
        self.iter_geometries()
            .any(|(_, geometry)| Self::geometry_has_material_theme(geometry, theme))
            || self
                .iter_geometry_templates()
                .any(|(_, geometry)| Self::geometry_has_material_theme(geometry, theme))
    }

    pub fn has_texture_theme(&self, theme: &str) -> bool {
        self.iter_geometries()
            .any(|(_, geometry)| Self::geometry_has_texture_theme(geometry, theme))
            || self
                .iter_geometry_templates()
                .any(|(_, geometry)| Self::geometry_has_texture_theme(geometry, theme))
    }

    /// Validate that configured default appearance themes exist in the model.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::InvalidThemeName`] when a configured default theme name
    /// does not appear in any geometry material or texture theme map.
    pub fn validate_default_themes(&self) -> Result<()> {
        if let Some(theme) = self.default_material_theme()
            && !self.has_material_theme(theme.as_ref())
        {
            return Err(Error::InvalidThemeName {
                theme_type: "material".to_string(),
                theme: theme.as_ref().to_string(),
            });
        }

        if let Some(theme) = self.default_texture_theme()
            && !self.has_texture_theme(theme.as_ref())
        {
            return Err(Error::InvalidThemeName {
                theme_type: "texture".to_string(),
                theme: theme.as_ref().to_string(),
            });
        }

        Ok(())
    }

    /// Extracts a float attribute column from all `CityObjects`.
    ///
    /// Returns `(object_refs, values)` where each index in both vectors corresponds.
    pub fn extract_float_column(&self, key: &str) -> (Vec<CityObjectHandle>, Vec<f64>) {
        self.extract_attribute_column(key, |value| match value {
            AttributeValue::Float(value) => Some(*value),
            _ => None,
        })
    }

    /// Extracts an integer attribute column from all `CityObjects`.
    ///
    /// Returns `(object_refs, values)` where each index in both vectors corresponds.
    pub fn extract_integer_column(&self, key: &str) -> (Vec<CityObjectHandle>, Vec<i64>) {
        self.extract_attribute_column(key, |value| match value {
            AttributeValue::Integer(value) => Some(*value),
            _ => None,
        })
    }

    /// Extracts a string attribute column from all `CityObjects`.
    ///
    /// Returns `(object_refs, values)` where each index in both vectors corresponds.
    pub fn extract_string_column<'a>(
        &'a self,
        key: &str,
    ) -> (Vec<CityObjectHandle>, Vec<&'a SS::String>) {
        self.extract_attribute_column(key, |value| match value {
            AttributeValue::String(value) => Some(value),
            _ => None,
        })
    }

    fn extract_attribute_column<'a, T, F>(
        &'a self,
        key: &str,
        mut select: F,
    ) -> (Vec<CityObjectHandle>, Vec<T>)
    where
        F: FnMut(&'a AttributeValue<SS>) -> Option<T>,
    {
        let mut object_refs = Vec::new();
        let mut values = Vec::new();

        for (id, cityobject) in self.cityobjects().iter() {
            if let Some(attributes) = cityobject.attributes()
                && let Some(value) = attributes.get(key).and_then(&mut select)
            {
                object_refs.push(id);
                values.push(value);
            }
        }

        (object_refs, values)
    }

    fn geometry_has_material_theme(geometry: &Geometry<VR, SS>, theme: &str) -> bool {
        geometry
            .materials()
            .is_some_and(|themes| themes.iter().any(|(name, _)| name.as_ref() == theme))
    }

    fn geometry_has_texture_theme(geometry: &Geometry<VR, SS>, theme: &str) -> bool {
        geometry
            .textures()
            .is_some_and(|themes| themes.iter().any(|(name, _)| name.as_ref() == theme))
    }

    /// Returns all unique attribute keys from all `CityObjects`.
    pub fn attribute_keys(&self) -> HashSet<&str> {
        let mut keys = HashSet::new();

        for (_, cityobject) in self.cityobjects().iter() {
            if let Some(attributes) = cityobject.attributes() {
                for key in attributes.keys() {
                    keys.insert(key.as_ref());
                }
            }
        }

        keys
    }
}

impl<VR: VertexRef, SS: StringStorage> RawAccess for CityModel<VR, SS> {
    type Vertex = RealWorldCoordinate;
    type Geometry = Geometry<VR, SS>;
    type Semantic = Semantic<SS>;
    type Material = Material<SS>;
    type Texture = Texture<SS>;

    fn vertices_raw(&self) -> RawSliceView<'_, Self::Vertex> {
        RawSliceView::new(self.vertices().as_slice())
    }

    fn geometries_raw(&self) -> RawPoolView<'_, Self::Geometry> {
        self.inner.geometries_raw()
    }

    fn semantics_raw(&self) -> RawPoolView<'_, Self::Semantic> {
        self.inner.semantics_raw()
    }

    fn materials_raw(&self) -> RawPoolView<'_, Self::Material> {
        self.inner.materials_raw()
    }

    fn textures_raw(&self) -> RawPoolView<'_, Self::Texture> {
        self.inner.textures_raw()
    }
}

impl<VR: VertexRef, SS: StringStorage> fmt::Display for CityModel<VR, SS> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "CityModel {{")?;
        writeln!(f, "\ttype: {}", self.type_citymodel())?;
        writeln!(f, "\tversion: {}", format_option(self.version().as_ref()))?;
        writeln!(
            f,
            "\textensions: {{ {} }}",
            format_option(self.extensions())
        )?;
        writeln!(f, "\tid: {}", format_option(self.id().as_ref()))?;
        writeln!(f, "\ttransform: {{ {} }}", format_option(self.transform()))?;
        writeln!(f, "\tmetadata: {}", format_option(self.metadata()))?;
        writeln!(
            f,
            "\tCityObjects: {{ nr. cityobjects: {}, nr. geometries: {} }}",
            self.cityobjects().len(),
            self.geometry_count()
        )?;
        writeln!(
            f,
            "\tappearance: {{ nr. materials: {}, nr. textures: {}, nr. vertices-texture: {}, default-theme-texture: {}, default-theme-material: {} }}",
            self.material_count(),
            self.texture_count(),
            self.vertices_texture().len(),
            format_option(self.default_texture_theme()),
            format_option(self.default_material_theme())
        )?;
        writeln!(f, "\tgeometry-templates: not implemented")?;
        writeln!(
            f,
            "\tvertices: {{ nr. vertices: {}, quantized coordinates: not implemented }}",
            self.vertices().len()
        )?;
        writeln!(f, "\textra: {}", format_option(self.extra()))?;
        writeln!(f, "}}")
    }
}

impl<VR: VertexRef, SS: StringStorage> GeometryValidationContext<VR, ResourceId32>
    for CityModel<VR, SS>
{
    fn semantic_exists(&self, id: ResourceId32) -> bool {
        self.inner.get_semantic(id).is_some()
    }

    fn material_exists(&self, id: ResourceId32) -> bool {
        self.inner.get_material(id).is_some()
    }

    fn texture_exists(&self, id: ResourceId32) -> bool {
        self.inner.get_texture(id).is_some()
    }

    fn uv_exists(&self, id: VertexIndex<VR>) -> bool {
        self.inner.get_uv_coordinate(id).is_some()
    }

    fn regular_vertex_exists(&self, id: VertexIndex<VR>) -> bool {
        self.inner.get_vertex(id).is_some()
    }

    fn template_vertex_exists(&self, id: VertexIndex<VR>) -> bool {
        self.inner.get_template_vertex(id).is_some()
    }

    fn template_geometry_exists(&self, id: ResourceId32) -> bool {
        self.inner.get_template_geometry(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CityModelType;
    use crate::backend::default::geometry::GeometryInstanceData;
    use crate::resources::id::ResourceId32;
    use crate::v2_0::appearance::ImageType;
    use crate::v2_0::appearance::material::Material;
    use crate::v2_0::appearance::texture::Texture;
    use crate::v2_0::boundary::nested::BoundaryNestedMultiPoint32;
    use crate::v2_0::geometry::{
        AffineTransform3D, LoD, StoredGeometryInstance, StoredGeometryParts,
    };
    use crate::v2_0::{
        CityObject, CityObjectIdentifier, CityObjectType, GeometryDraft, PointDraft, RingDraft,
        SurfaceDraft,
    };

    fn multi_point_geometry(
        vertices: BoundaryNestedMultiPoint32,
    ) -> Geometry<u32, OwnedStringStorage> {
        Geometry::from_raw_parts(
            GeometryType::MultiPoint,
            Some(LoD::LoD1),
            Some(vertices.into()),
            None,
            None,
            None,
            None,
        )
    }

    fn stored_multi_point_geometry(
        vertices: BoundaryNestedMultiPoint32,
    ) -> Geometry<u32, OwnedStringStorage> {
        Geometry::from_stored_parts(StoredGeometryParts {
            type_geometry: GeometryType::MultiPoint,
            lod: Some(LoD::LoD1),
            boundaries: Some(vertices.into()),
            semantics: None,
            materials: None,
            textures: None,
            instance: None,
        })
    }

    fn building(id: &str) -> CityObject<OwnedStringStorage> {
        CityObject::new(
            CityObjectIdentifier::new(id.to_string()),
            CityObjectType::Building,
        )
    }

    fn add_stored_multi_point_geometry(
        model: &mut OwnedCityModel,
        coordinates: &[[f64; 3]],
    ) -> GeometryHandle {
        let mut boundary_vertices = Vec::with_capacity(coordinates.len());
        for [x, y, z] in coordinates {
            let index = model
                .add_vertex(RealWorldCoordinate::new(*x, *y, *z))
                .unwrap();
            boundary_vertices.push(index.value());
        }

        model
            .add_geometry(stored_multi_point_geometry(boundary_vertices))
            .unwrap()
    }

    fn add_cityobject_with_geometries(
        model: &mut OwnedCityModel,
        id: &str,
        geometries: &[GeometryHandle],
    ) -> CityObjectHandle {
        let mut cityobject = building(id);
        for geometry in geometries {
            cityobject.add_geometry(*geometry);
        }
        model.cityobjects_mut().add(cityobject).unwrap()
    }

    fn assert_invalid_reference(err: &Error, expected_element_type: &str) {
        assert!(
            matches!(
                err,
                Error::InvalidReference { element_type, .. } if element_type == expected_element_type
            ),
            "expected InvalidReference for {expected_element_type}, got {err}"
        );
    }

    #[test]
    fn calculate_cityobject_geographical_extent_from_multipoint_geometry() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let geometry = add_stored_multi_point_geometry(
            &mut model,
            &[[1.0, 2.0, 3.0], [-4.0, 5.0, 6.0], [7.0, -8.0, -9.0]],
        );
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[geometry]);

        let extent = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap();

        assert_eq!(extent, Some(BBox::new(-4.0, -8.0, -9.0, 7.0, 5.0, 6.0)));
    }

    #[test]
    fn calculate_cityobject_geographical_extent_unions_multiple_geometries() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let first =
            add_stored_multi_point_geometry(&mut model, &[[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]);
        let second = add_stored_multi_point_geometry(
            &mut model,
            &[[-10.0, -20.0, -30.0], [-4.0, -5.0, -6.0]],
        );
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[first, second]);

        let extent = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap();

        assert_eq!(extent, Some(BBox::new(-10.0, -20.0, -30.0, 3.0, 4.0, 5.0)));
    }

    #[test]
    fn calculate_geographical_extent_unions_all_cityobjects() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let first =
            add_stored_multi_point_geometry(&mut model, &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]]);
        let second =
            add_stored_multi_point_geometry(&mut model, &[[-5.0, 10.0, -2.0], [6.0, 11.0, 4.0]]);
        add_cityobject_with_geometries(&mut model, "building-1", &[first]);
        add_cityobject_with_geometries(&mut model, "building-2", &[second]);

        let extent = model.calculate_geographical_extent().unwrap();

        assert_eq!(extent, Some(BBox::new(-5.0, 0.0, -2.0, 6.0, 11.0, 4.0)));
    }

    #[test]
    fn calculate_geographical_extent_ignores_orphan_vertices_and_geometries() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let attached =
            add_stored_multi_point_geometry(&mut model, &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]]);
        add_cityobject_with_geometries(&mut model, "building-1", &[attached]);

        model
            .add_vertex(RealWorldCoordinate::new(-1000.0, -1000.0, -1000.0))
            .unwrap();
        model
            .add_vertex(RealWorldCoordinate::new(1000.0, 1000.0, 1000.0))
            .unwrap();
        add_stored_multi_point_geometry(
            &mut model,
            &[[-500.0, -500.0, -500.0], [500.0, 500.0, 500.0]],
        );

        let extent = model.calculate_geographical_extent().unwrap();

        assert_eq!(extent, Some(BBox::new(0.0, 0.0, 0.0, 1.0, 2.0, 3.0)));
    }

    #[test]
    fn calculate_cityobject_geographical_extent_returns_none_without_geometry() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let handle = model.cityobjects_mut().add(building("building-1")).unwrap();

        let extent = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap();

        assert_eq!(extent, None);
    }

    #[test]
    fn calculate_geographical_extent_returns_none_without_cityobject_geometry() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);

        assert_eq!(model.calculate_geographical_extent().unwrap(), None);

        model.cityobjects_mut().add(building("building-1")).unwrap();

        assert_eq!(model.calculate_geographical_extent().unwrap(), None);
    }

    #[test]
    fn calculate_cityobject_geographical_extent_resolves_geometry_instance_identity() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let template = GeometryDraft::multi_point(
            None,
            [
                PointDraft::new(RealWorldCoordinate::new(-1.0, 2.0, 3.0)),
                PointDraft::new(RealWorldCoordinate::new(4.0, -5.0, 6.0)),
            ],
        )
        .insert_template_into(&mut model)
        .unwrap();
        let instance = GeometryDraft::instance(
            template,
            RealWorldCoordinate::new(10.0, 20.0, 30.0),
            AffineTransform3D::identity(),
        )
        .insert_into(&mut model)
        .unwrap();
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[instance]);

        let extent = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap();

        assert_eq!(extent, Some(BBox::new(9.0, 15.0, 33.0, 14.0, 22.0, 36.0)));
    }

    #[test]
    fn calculate_cityobject_geographical_extent_resolves_geometry_instance_scaling() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let template = GeometryDraft::multi_point(
            None,
            [
                PointDraft::new(RealWorldCoordinate::new(-1.0, 2.0, -3.0)),
                PointDraft::new(RealWorldCoordinate::new(4.0, -5.0, 6.0)),
            ],
        )
        .insert_template_into(&mut model)
        .unwrap();
        let instance = GeometryDraft::instance(
            template,
            RealWorldCoordinate::new(10.0, 20.0, 30.0),
            AffineTransform3D::new([
                2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]),
        )
        .insert_into(&mut model)
        .unwrap();
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[instance]);

        let extent = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap();

        assert_eq!(extent, Some(BBox::new(8.0, 5.0, 18.0, 18.0, 26.0, 54.0)));
    }

    #[test]
    fn calculate_cityobject_geographical_extent_errors_for_missing_cityobject_handle() {
        let model = OwnedCityModel::new(CityModelType::CityJSON);
        let handle = unsafe { CityObjectHandle::from_raw_parts_unchecked(42, 0) };

        let err = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap_err();

        assert_invalid_reference(&err, "city object");
    }

    #[test]
    fn calculate_cityobject_geographical_extent_errors_for_missing_geometry_handle() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let geometry = unsafe { GeometryHandle::from_raw_parts_unchecked(42, 0) };
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[geometry]);

        let err = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap_err();

        assert_invalid_reference(&err, "geometry");
    }

    #[test]
    fn calculate_cityobject_geographical_extent_errors_for_missing_vertex_reference() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let geometry = model
            .add_geometry_unchecked(stored_multi_point_geometry(vec![0u32]))
            .unwrap();
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[geometry]);

        let err = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap_err();

        assert_invalid_reference(&err, "vertex");
    }

    #[test]
    fn calculate_cityobject_geographical_extent_errors_for_missing_template_reference() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let reference_point = model
            .add_vertex(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap();
        let missing_template = unsafe { GeometryTemplateHandle::from_raw_parts_unchecked(42, 0) };
        let geometry = Geometry::from_stored_parts(StoredGeometryParts {
            type_geometry: GeometryType::GeometryInstance,
            lod: None,
            boundaries: None,
            semantics: None,
            materials: None,
            textures: None,
            instance: Some(StoredGeometryInstance {
                template: missing_template,
                reference_point,
                transformation: AffineTransform3D::identity(),
            }),
        });
        let geometry = model.add_geometry_unchecked(geometry).unwrap();
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[geometry]);

        let err = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap_err();

        assert_invalid_reference(&err, "template geometry");
    }

    #[test]
    fn calculate_methods_do_not_use_or_mutate_stored_geographical_extent() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let geometry =
            add_stored_multi_point_geometry(&mut model, &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]]);
        let handle = add_cityobject_with_geometries(&mut model, "building-1", &[geometry]);
        let stored_cityobject_extent = BBox::new(-100.0, -100.0, -100.0, -90.0, -90.0, -90.0);
        let stored_metadata_extent = BBox::new(90.0, 90.0, 90.0, 100.0, 100.0, 100.0);
        model
            .cityobjects_mut()
            .get_mut(handle)
            .unwrap()
            .set_geographical_extent(Some(stored_cityobject_extent));
        model
            .metadata_mut()
            .set_geographical_extent(stored_metadata_extent);

        let cityobject_extent = model
            .calculate_cityobject_geographical_extent(handle)
            .unwrap();
        let model_extent = model.calculate_geographical_extent().unwrap();

        assert_eq!(
            cityobject_extent,
            Some(BBox::new(0.0, 0.0, 0.0, 1.0, 2.0, 3.0))
        );
        assert_eq!(model_extent, Some(BBox::new(0.0, 0.0, 0.0, 1.0, 2.0, 3.0)));
        assert_eq!(
            model
                .cityobjects()
                .get(handle)
                .unwrap()
                .geographical_extent()
                .copied(),
            Some(stored_cityobject_extent)
        );
        assert_eq!(
            model.metadata().unwrap().geographical_extent().copied(),
            Some(stored_metadata_extent)
        );
    }

    #[test]
    fn add_geometry_rejects_missing_regular_boundary_vertex_even_when_template_vertex_exists() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model
            .add_template_vertex(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap();

        let err = model
            .add_geometry(multi_point_geometry(vec![0u32]))
            .unwrap_err();

        assert!(format!("{err}").contains("missing regular vertex"));
    }

    #[test]
    fn add_geometry_template_rejects_missing_template_boundary_vertex_even_when_regular_vertex_exists()
     {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model
            .add_vertex(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap();

        let err = model
            .add_geometry_template(multi_point_geometry(vec![0u32]))
            .unwrap_err();

        assert!(format!("{err}").contains("missing template vertex"));
    }

    #[test]
    fn add_geometry_unchecked_accepts_valid_stored_geometry() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model
            .add_vertex(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap();

        let geometry = stored_multi_point_geometry(vec![0u32]);
        let handle = model.add_geometry_unchecked(geometry).unwrap();

        assert_eq!(model.geometry_count(), 1);
        assert_eq!(
            model.get_geometry(handle).unwrap().type_geometry(),
            &GeometryType::MultiPoint
        );
    }

    #[test]
    fn add_geometry_template_unchecked_accepts_valid_stored_geometry() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model
            .add_template_vertex(RealWorldCoordinate::new(0.0, 0.0, 0.0))
            .unwrap();

        let geometry = stored_multi_point_geometry(vec![0u32]);
        let handle = model.add_geometry_template_unchecked(geometry).unwrap();

        assert_eq!(model.geometry_template_count(), 1);
        assert_eq!(
            model.get_geometry_template(handle).unwrap().type_geometry(),
            &GeometryType::MultiPoint
        );
    }

    #[test]
    fn add_geometry_template_rejects_geometry_instance() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let geometry = Geometry::from_raw_parts(
            GeometryType::GeometryInstance,
            None,
            None,
            None,
            None,
            None,
            Some(GeometryInstanceData::new(
                ResourceId32::new(0, 0),
                VertexIndex::new(0),
                AffineTransform3D::identity(),
            )),
        );

        let err = model.add_geometry_template(geometry).unwrap_err();

        assert_eq!(
            format!("{err}"),
            "GeometryInstance cannot be inserted into the template geometry pool"
        );
    }

    #[test]
    fn set_default_material_theme_stores_theme_name_without_validation() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model.set_default_material_theme(Some(ThemeName::new("missing-theme".to_string())));

        assert_eq!(
            model.default_material_theme().map(AsRef::as_ref),
            Some("missing-theme")
        );
    }

    #[test]
    fn validate_default_themes_rejects_missing_material_theme() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model.set_default_material_theme(Some(ThemeName::new("missing-theme".to_string())));

        let err = model.validate_default_themes().unwrap_err();

        assert!(matches!(
            err,
            Error::InvalidThemeName { ref theme_type, ref theme }
                if theme_type == "material" && theme == "missing-theme"
        ));
    }

    #[test]
    fn validate_default_themes_rejects_missing_texture_theme() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model.set_default_texture_theme(Some(ThemeName::new("missing-theme".to_string())));

        let err = model.validate_default_themes().unwrap_err();

        assert!(matches!(
            err,
            Error::InvalidThemeName { ref theme_type, ref theme }
                if theme_type == "texture" && theme == "missing-theme"
        ));
    }

    #[test]
    fn validate_default_themes_accepts_present_geometry_themes() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        let material = model
            .add_material(Material::new("mat-a".to_string()))
            .unwrap();
        let texture = model
            .add_texture(Texture::new("tex-a.png".to_string(), ImageType::Png))
            .unwrap();
        let theme = ThemeName::new("theme-a".to_string());

        GeometryDraft::multi_surface(
            None,
            [SurfaceDraft::new(
                RingDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]).with_texture(
                    theme.clone(),
                    texture,
                    [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                ),
                [],
            )
            .with_material(theme.clone(), material)],
        )
        .insert_into(&mut model)
        .unwrap();

        model.set_default_material_theme(Some(theme.clone()));
        model.set_default_texture_theme(Some(theme.clone()));

        assert!(model.has_material_theme("theme-a"));
        assert!(model.has_texture_theme("theme-a"));
        assert!(model.validate_default_themes().is_ok());
    }

    #[test]
    fn cityjsonfeature_root_id_is_stored_as_typed_model_state() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSONFeature);
        let handle = model
            .cityobjects_mut()
            .add(CityObject::new(
                CityObjectIdentifier::new("feature-1".to_string()),
                CityObjectType::Building,
            ))
            .unwrap();

        model.set_id(Some(handle));

        assert_eq!(model.id(), Some(handle));
        assert!(model.extra().is_none());
    }

    #[test]
    fn cityjson_root_extra_id_remains_independent_from_typed_feature_id() {
        let mut model = OwnedCityModel::new(CityModelType::CityJSON);
        model.extra_mut().insert(
            "id".to_string(),
            AttributeValue::String("document-root-id".to_string()),
        );

        assert_eq!(model.id(), None);
        assert_eq!(
            model.extra().and_then(|extra| extra.get("id")),
            Some(&AttributeValue::String("document-root-id".to_string()))
        );
    }
}
