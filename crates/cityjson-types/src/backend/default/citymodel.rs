//! # `CityModel` Core
//!
//! Core implementation of `CityModel` that is shared across different `CityJSON` versions.
//!
//! This module provides the `CityModelCore` type which contains the common data structures
//! used by all `CityJSON` versions. Version-specific implementations wrap this core type
//! and provide version-specific behavior through macros.

use crate::cityjson::core::appearance::ThemeName;
use crate::cityjson::core::attributes::Attributes;
use crate::cityjson::core::coordinate::UVCoordinate;
use crate::cityjson::core::vertex::{VertexIndex, VertexRef};
use crate::cityjson::core::vertices::Vertices;
use crate::error::Result;
use crate::raw::RawPoolView;
use crate::resources::id::ResourceId;
use crate::resources::pool::{DefaultResourcePool, ResourcePool};
use crate::resources::storage::StringStorage;
use crate::v2_0::coordinate::RealWorldCoordinate;
use crate::{CityJSONVersion, CityModelType};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CityModelCoreCapacities {
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

/// Core `CityModel` structure that is shared across all `CityJSON` versions.
///
/// This type is generic over:
/// - `VR`: The vertex reference type
/// - `RR`: The resource reference type
/// - `SS`: The string storage type
/// - `Semantic`: The semantic type for this version
/// - `Material`: The material type for this version
/// - `Texture`: The texture type for this version
/// - `Geometry`: The geometry type for this version
/// - `Metadata`: The metadata type for this version
/// - `Transform`: The transform type for this version
/// - `Extensions`: The extensions type for this version
/// - `CityObjects`: The city objects collection type for this version
#[derive(Debug, Clone)]
pub(crate) struct CityModelCore<
    VR: VertexRef,
    RR: ResourceId,
    SS: StringStorage,
    Semantic,
    Material,
    Texture,
    Geometry,
    Metadata,
    Transform,
    Extensions,
    CityObjects,
> {
    /// `CityModel` type
    type_citymodel: CityModelType,
    /// `CityJSON` version
    version: Option<CityJSONVersion>,
    /// `CityJSON` Extension declarations
    extensions: Option<Extensions>,
    /// Typed root `id` for `CityJSONFeature` documents.
    id: Option<RR>,
    /// Extra root properties for the `CityModel`
    extra: Option<Attributes<SS>>,
    /// `CityModel` metadata
    metadata: Option<Metadata>,
    /// Collection of `CityObjects`
    cityobjects: CityObjects,
    /// The transform object
    transform: Option<Transform>,
    /// Pool of vertex coordinates
    vertices: Vertices<VR, RealWorldCoordinate>,
    /// Pool of geometries
    geometries: DefaultResourcePool<Geometry, RR>,
    /// Pool of vertex coordinates used by the geometry templates in `template_geometries`
    template_vertices: Vertices<VR, RealWorldCoordinate>,
    /// Pool of geometry templates
    template_geometries: DefaultResourcePool<Geometry, RR>,
    /// Pool of semantic objects
    semantics: DefaultResourcePool<Semantic, RR>,
    /// Pool of material objects
    materials: DefaultResourcePool<Material, RR>,
    /// Pool of texture objects
    textures: DefaultResourcePool<Texture, RR>,
    /// Pool of vertex textures (UV coordinates)
    vertices_texture: Vertices<VR, UVCoordinate>,
    /// Default material theme name
    default_material_theme: Option<ThemeName<SS>>,
    /// Default texture theme name
    default_texture_theme: Option<ThemeName<SS>>,
}

impl<
    VR: VertexRef,
    RR: ResourceId,
    SS: StringStorage,
    Semantic,
    Material,
    Texture,
    Geometry,
    Metadata,
    Transform,
    Extensions,
    CityObjects,
>
    CityModelCore<
        VR,
        RR,
        SS,
        Semantic,
        Material,
        Texture,
        Geometry,
        Metadata,
        Transform,
        Extensions,
        CityObjects,
    >
where
    CityObjects: Default,
{
    /// Create a new `CityModelCore` with the given type and version
    #[must_use]
    pub fn new(type_citymodel: CityModelType, version: Option<CityJSONVersion>) -> Self {
        Self {
            type_citymodel,
            version,
            extensions: None,
            id: None,
            extra: None,
            metadata: None,
            cityobjects: CityObjects::default(),
            transform: None,
            vertices: Vertices::new(),
            geometries: DefaultResourcePool::new_pool(),
            template_vertices: Vertices::new(),
            template_geometries: DefaultResourcePool::new_pool(),
            semantics: DefaultResourcePool::new_pool(),
            materials: DefaultResourcePool::new_pool(),
            textures: DefaultResourcePool::new_pool(),
            vertices_texture: Vertices::new(),
            default_material_theme: None,
            default_texture_theme: None,
        }
    }

    /// Create a new `CityModelCore` with specified capacities.
    pub fn with_capacities(
        type_citymodel: CityModelType,
        version: Option<CityJSONVersion>,
        capacities: CityModelCoreCapacities,
        create_cityobjects: impl FnOnce(usize) -> CityObjects,
    ) -> Self {
        Self {
            type_citymodel,
            version,
            extensions: None,
            id: None,
            extra: None,
            metadata: None,
            cityobjects: create_cityobjects(capacities.cityobjects),
            transform: None,
            vertices: Vertices::with_capacity(capacities.vertices),
            geometries: DefaultResourcePool::with_capacity(capacities.geometries),
            template_vertices: Vertices::with_capacity(capacities.template_vertices),
            template_geometries: DefaultResourcePool::with_capacity(capacities.template_geometries),
            semantics: DefaultResourcePool::with_capacity(capacities.semantics),
            materials: DefaultResourcePool::with_capacity(capacities.materials),
            textures: DefaultResourcePool::with_capacity(capacities.textures),
            vertices_texture: Vertices::with_capacity(capacities.uv_coordinates),
            default_material_theme: None,
            default_texture_theme: None,
        }
    }

    // ==================== SEMANTICS ====================

    /// Get a semantic by its resource reference
    pub fn get_semantic(&self, id: RR) -> Option<&Semantic> {
        self.semantics.get(id)
    }

    /// Get a mutable reference to a semantic
    pub fn get_semantic_mut(&mut self, id: RR) -> Option<&mut Semantic> {
        self.semantics.get_mut(id)
    }

    /// Add a semantic and return its resource reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the semantic pool cannot store
    /// additional entries for the configured `RR` reference type.
    pub fn add_semantic(&mut self, semantic: Semantic) -> Result<RR> {
        self.semantics.add(semantic)
    }

    /// Get the number of semantics in the model
    pub fn semantic_count(&self) -> usize {
        self.semantics.len()
    }

    /// Check if there are any semantics
    pub fn has_semantics(&self) -> bool {
        !self.semantics.is_empty()
    }

    /// Iterate over all semantics
    pub fn iter_semantics(&self) -> impl Iterator<Item = (RR, &Semantic)> + '_ {
        self.semantics.iter()
    }

    /// Returns a zero-copy raw view of the semantic resource pool.
    pub fn semantics_raw(&self) -> RawPoolView<'_, Semantic> {
        self.semantics.raw_view()
    }

    /// Iterate over all semantics with mutable references
    pub fn iter_semantics_mut(&mut self) -> impl Iterator<Item = (RR, &mut Semantic)> + '_ {
        self.semantics.iter_mut()
    }

    /// Find a semantic by value (if it implements `PartialEq`)
    pub fn find_semantic(&self, semantic: &Semantic) -> Option<RR>
    where
        Semantic: PartialEq,
    {
        self.semantics.find(semantic)
    }

    /// Remove a semantic by its resource reference
    #[cfg(test)]
    pub(crate) fn remove_semantic(&mut self, id: RR) -> Option<Semantic> {
        self.semantics.remove(id)
    }

    /// Get or insert a semantic, returning the resource reference
    ///
    /// Note: Deduplication works when semantics are reused (same instance cloned).
    /// Creating new semantics with different `AttributeId32` values won't deduplicate
    /// even if attribute values are logically identical.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when inserting a new semantic would
    /// exceed the semantic pool capacity for `RR`.
    pub fn get_or_insert_semantic(&mut self, semantic: Semantic) -> Result<RR>
    where
        Semantic: PartialEq,
    {
        if let Some(existing_id) = self.semantics.find(&semantic) {
            return Ok(existing_id);
        }
        self.semantics.add(semantic)
    }

    // ==================== MATERIALS ====================

    /// Get a material by its resource reference
    pub fn get_material(&self, id: RR) -> Option<&Material> {
        self.materials.get(id)
    }

    /// Get a mutable reference to a material
    pub fn get_material_mut(&mut self, id: RR) -> Option<&mut Material> {
        self.materials.get_mut(id)
    }

    /// Add a material and return its resource reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the material pool cannot store
    /// additional entries for the configured `RR` reference type.
    pub fn add_material(&mut self, material: Material) -> Result<RR> {
        self.materials.add(material)
    }

    /// Get the number of materials in the model
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Iterate over all materials
    pub fn iter_materials(&self) -> impl Iterator<Item = (RR, &Material)> + '_ {
        self.materials.iter()
    }

    /// Returns a zero-copy raw view of the material resource pool.
    pub fn materials_raw(&self) -> RawPoolView<'_, Material> {
        self.materials.raw_view()
    }

    /// Iterate over all materials with mutable references
    pub fn iter_materials_mut(&mut self) -> impl Iterator<Item = (RR, &mut Material)> + '_ {
        self.materials.iter_mut()
    }

    /// Find a material by value (if it implements `PartialEq`)
    pub fn find_material(&self, material: &Material) -> Option<RR>
    where
        Material: PartialEq,
    {
        self.materials.find(material)
    }

    /// Remove a material by its resource reference
    #[cfg(test)]
    pub(crate) fn remove_material(&mut self, id: RR) -> Option<Material> {
        self.materials.remove(id)
    }

    /// Get or insert a material, returning the resource reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when inserting a new material would
    /// exceed the material pool capacity for `RR`.
    pub fn get_or_insert_material(&mut self, material: Material) -> Result<RR>
    where
        Material: PartialEq,
    {
        if let Some(existing_id) = self.materials.find(&material) {
            return Ok(existing_id);
        }
        self.materials.add(material)
    }

    // ==================== TEXTURES ====================

    /// Get a texture by its resource reference
    pub fn get_texture(&self, id: RR) -> Option<&Texture> {
        self.textures.get(id)
    }

    /// Get a mutable reference to a texture
    pub fn get_texture_mut(&mut self, id: RR) -> Option<&mut Texture> {
        self.textures.get_mut(id)
    }

    /// Add a texture and return its resource reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the texture pool cannot store
    /// additional entries for the configured `RR` reference type.
    pub fn add_texture(&mut self, texture: Texture) -> Result<RR> {
        self.textures.add(texture)
    }

    /// Get the number of textures in the model
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Iterate over all textures
    pub fn iter_textures(&self) -> impl Iterator<Item = (RR, &Texture)> + '_ {
        self.textures.iter()
    }

    /// Returns a zero-copy raw view of the texture resource pool.
    pub fn textures_raw(&self) -> RawPoolView<'_, Texture> {
        self.textures.raw_view()
    }

    /// Iterate over all textures with mutable references
    pub fn iter_textures_mut(&mut self) -> impl Iterator<Item = (RR, &mut Texture)> + '_ {
        self.textures.iter_mut()
    }

    /// Find a texture by value (if it implements `PartialEq`)
    pub fn find_texture(&self, texture: &Texture) -> Option<RR>
    where
        Texture: PartialEq,
    {
        self.textures.find(texture)
    }

    /// Remove a texture by its resource reference
    #[cfg(test)]
    pub(crate) fn remove_texture(&mut self, id: RR) -> Option<Texture> {
        self.textures.remove(id)
    }

    /// Get or insert a texture, returning the resource reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when inserting a new texture would
    /// exceed the texture pool capacity for `RR`.
    pub fn get_or_insert_texture(&mut self, texture: Texture) -> Result<RR>
    where
        Texture: PartialEq,
    {
        if let Some(existing_id) = self.textures.find(&texture) {
            return Ok(existing_id);
        }
        self.textures.add(texture)
    }

    // ==================== GEOMETRIES ====================

    /// Get a geometry by its resource reference
    pub fn get_geometry(&self, id: RR) -> Option<&Geometry> {
        self.geometries.get(id)
    }

    /// Add a geometry and return its resource reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the geometry pool cannot store
    /// additional entries for the configured `RR` reference type.
    pub fn add_geometry(&mut self, geometry: Geometry) -> Result<RR> {
        self.geometries.add(geometry)
    }

    /// Get the number of geometries in the model
    pub fn geometry_count(&self) -> usize {
        self.geometries.len()
    }

    /// Iterate over all geometries
    pub fn iter_geometries(&self) -> impl Iterator<Item = (RR, &Geometry)> + '_ {
        self.geometries.iter()
    }

    /// Returns a zero-copy raw view of the geometry resource pool.
    pub fn geometries_raw(&self) -> RawPoolView<'_, Geometry> {
        self.geometries.raw_view()
    }

    pub(crate) fn reserve_geometry_capacity(&mut self, additional: usize) -> Result<()> {
        self.geometries.reserve(additional)
    }

    pub(crate) fn reserve_semantic_capacity(&mut self, additional: usize) -> Result<()> {
        self.semantics.reserve(additional)
    }

    pub(crate) fn reserve_material_capacity(&mut self, additional: usize) -> Result<()> {
        self.materials.reserve(additional)
    }

    pub(crate) fn reserve_texture_capacity(&mut self, additional: usize) -> Result<()> {
        self.textures.reserve(additional)
    }

    // Vertex methods
    pub fn vertices(&self) -> &Vertices<VR, RealWorldCoordinate> {
        &self.vertices
    }

    pub fn vertices_mut(&mut self) -> &mut Vertices<VR, RealWorldCoordinate> {
        &mut self.vertices
    }

    pub(crate) fn reserve_vertex_capacity(&mut self, additional: usize) -> Result<()> {
        self.vertices.reserve(additional)
    }

    /// Add a vertex and return its index.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the vertex container cannot
    /// represent more vertices for `VR`.
    pub fn add_vertex(&mut self, coordinate: RealWorldCoordinate) -> Result<VertexIndex<VR>> {
        self.vertices.push(coordinate)
    }

    /// Add many vertices and return the contiguous index range assigned to them.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the vertex container cannot
    /// represent more vertices for `VR`.
    pub fn add_vertices(
        &mut self,
        coordinates: &[RealWorldCoordinate],
    ) -> Result<std::ops::Range<VertexIndex<VR>>> {
        self.vertices.extend_from_slice(coordinates)
    }

    pub fn get_vertex(&self, index: VertexIndex<VR>) -> Option<&RealWorldCoordinate> {
        self.vertices.get(index)
    }

    // Metadata methods
    pub fn metadata(&self) -> Option<&Metadata> {
        self.metadata.as_ref()
    }

    pub fn metadata_mut(&mut self) -> &mut Metadata
    where
        Metadata: Default,
    {
        self.metadata.get_or_insert_with(Metadata::default)
    }

    // Extra methods
    pub fn id(&self) -> Option<RR> {
        self.id
    }

    pub fn set_id(&mut self, id: Option<RR>) {
        self.id = id;
    }

    pub fn extra(&self) -> Option<&Attributes<SS>> {
        self.extra.as_ref()
    }

    pub fn extra_mut(&mut self) -> &mut Attributes<SS> {
        self.extra.get_or_insert_with(Attributes::new)
    }

    // Transform methods
    pub fn transform(&self) -> Option<&Transform> {
        self.transform.as_ref()
    }

    pub fn transform_mut(&mut self) -> &mut Transform
    where
        Transform: Default,
    {
        self.transform.get_or_insert_with(Transform::default)
    }

    pub fn clear_transform(&mut self) {
        self.transform = None;
    }

    // Extensions methods
    pub fn extensions(&self) -> Option<&Extensions> {
        self.extensions.as_ref()
    }

    pub fn extensions_mut(&mut self) -> &mut Extensions
    where
        Extensions: Default,
    {
        self.extensions.get_or_insert_with(Extensions::default)
    }

    // CityObjects methods
    pub fn cityobjects(&self) -> &CityObjects {
        &self.cityobjects
    }

    pub fn cityobjects_mut(&mut self) -> &mut CityObjects {
        &mut self.cityobjects
    }

    // UV coordinate methods
    /// Add a UV coordinate and return its index.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the UV-coordinate container
    /// cannot represent more vertices for `VR`.
    pub fn add_uv_coordinate(&mut self, uvcoordinate: UVCoordinate) -> Result<VertexIndex<VR>> {
        self.vertices_texture.push(uvcoordinate)
    }

    pub fn get_uv_coordinate(&self, index: VertexIndex<VR>) -> Option<&UVCoordinate> {
        self.vertices_texture.get(index)
    }

    pub fn vertices_texture(&self) -> &Vertices<VR, UVCoordinate> {
        &self.vertices_texture
    }

    pub(crate) fn reserve_uv_capacity(&mut self, additional: usize) -> Result<()> {
        self.vertices_texture.reserve(additional)
    }

    // Template vertex methods
    /// Add a template vertex and return its index.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VerticesContainerFull`] when the template-vertex container
    /// cannot represent more vertices for `VR`.
    pub fn add_template_vertex(
        &mut self,
        coordinate: RealWorldCoordinate,
    ) -> Result<VertexIndex<VR>> {
        self.template_vertices.push(coordinate)
    }

    pub fn get_template_vertex(&self, index: VertexIndex<VR>) -> Option<&RealWorldCoordinate> {
        self.template_vertices.get(index)
    }

    pub fn template_vertices(&self) -> &Vertices<VR, RealWorldCoordinate> {
        &self.template_vertices
    }

    pub fn template_vertices_mut(&mut self) -> &mut Vertices<VR, RealWorldCoordinate> {
        &mut self.template_vertices
    }

    pub(crate) fn reserve_template_vertex_capacity(&mut self, additional: usize) -> Result<()> {
        self.template_vertices.reserve(additional)
    }

    // ==================== TEMPLATE GEOMETRIES ====================

    /// Get a template geometry by its resource reference
    pub fn get_template_geometry(&self, id: RR) -> Option<&Geometry> {
        self.template_geometries.get(id)
    }

    /// Add a template geometry and return its resource reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ResourcePoolFull`] when the template-geometry pool cannot
    /// store additional entries for the configured `RR` reference type.
    pub fn add_template_geometry(&mut self, geometry: Geometry) -> Result<RR> {
        self.template_geometries.add(geometry)
    }

    /// Get the number of template geometries in the model
    pub fn template_geometry_count(&self) -> usize {
        self.template_geometries.len()
    }

    /// Iterate over all template geometries
    pub fn iter_template_geometries(&self) -> impl Iterator<Item = (RR, &Geometry)> + '_ {
        self.template_geometries.iter()
    }

    pub(crate) fn reserve_template_geometry_capacity(&mut self, additional: usize) -> Result<()> {
        self.template_geometries.reserve(additional)
    }

    // ==================== ATTRIBUTES ====================

    // Type and version methods
    pub fn type_citymodel(&self) -> CityModelType {
        self.type_citymodel
    }

    pub fn version(&self) -> Option<CityJSONVersion> {
        self.version
    }

    // Appearance theme methods
    pub fn default_material_theme(&self) -> Option<&ThemeName<SS>> {
        self.default_material_theme.as_ref()
    }

    pub fn set_default_material_theme(&mut self, theme: Option<ThemeName<SS>>) {
        self.default_material_theme = theme;
    }

    pub fn default_texture_theme(&self) -> Option<&ThemeName<SS>> {
        self.default_texture_theme.as_ref()
    }

    pub fn set_default_texture_theme(&mut self, theme: Option<ThemeName<SS>>) {
        self.default_texture_theme = theme;
    }
}
