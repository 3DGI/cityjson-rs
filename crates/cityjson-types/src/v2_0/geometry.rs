//! Geometry read API for `CityJSON` 2.0.
//!
//! [`Geometry`] covers the eight geometry types from spec §3 (boundary arrays) plus
//! `GeometryInstance` (spec §3.4, geometry templates). The table below maps each type to the
//! nesting depth of its boundary array and its typical `LoD` range:
//!
//! | Type | Boundary nesting | Typical `LoD` |
//! |------|-----------------|-------------|
//! | `MultiPoint` | `[v, …]` | any |
//! | `MultiLineString` | `[[v, …], …]` | any |
//! | `MultiSurface` | `[[[v, …], …], …]` | 0–2 |
//! | `CompositeSurface` | `[[[v, …], …], …]` | 2–3 |
//! | `Solid` | `[[[[v, …], …], …], …]` | 1–3 |
//! | `MultiSolid` | `[[[[[v, …], …], …], …], …]` | 1–3 |
//! | `CompositeSolid` | `[[[[[v, …], …], …], …], …]` | 3 |
//! | `GeometryInstance` | — (template reference) | any |
//!
//! Boundaries are stored flat internally. Use
//! [`Boundary::check_type`](super::boundary::Boundary::check_type) and the `to_nested_*`
//! methods to recover the nested JSON form.
//!
//! Semantics, materials, and textures are accessed through map views ([`SemanticMapView`],
//! [`MaterialMapView`], [`TextureMapView`]) keyed by theme name.
//!
//! For `GeometryInstance`, [`CityModel::resolve_geometry`](super::citymodel::CityModel::resolve_geometry) returns a [`GeometryView`] pointing
//! at the referenced template geometry.
//!
//! ```rust
//! use cityjson_types::CityModelType;
//! use cityjson_types::error::Result;
//! use cityjson_types::v2_0::{
//!     AffineTransform3D, CityModel, GeometryDraft, GeometryType, PointDraft,
//!     RealWorldCoordinate,
//! };
//!
//! fn read_instance_and_resolve() -> Result<()> {
//!     let mut model = CityModel::<u32>::new(CityModelType::CityJSON);
//!
//!     let template_handle = GeometryDraft::multi_point(
//!         None,
//!         [PointDraft::new(RealWorldCoordinate::new(0.0, 0.0, 0.0))],
//!     )
//!     .insert_template_into(&mut model)?;
//!
//!     let instance_handle = GeometryDraft::instance(
//!         template_handle,
//!         RealWorldCoordinate::new(10.0, 20.0, 0.0),
//!         AffineTransform3D::identity(),
//!     )
//!     .insert_into(&mut model)?;
//!
//!     let geometry = model.get_geometry(instance_handle).unwrap();
//!     assert_eq!(geometry.type_geometry(), &GeometryType::GeometryInstance);
//!     let instance = geometry.instance().unwrap();
//!     assert_eq!(instance.template(), template_handle);
//!
//!     let resolved = model.resolve_geometry(instance_handle)?;
//!     assert_eq!(resolved.type_geometry(), &GeometryType::MultiPoint);
//!     Ok(())
//! }
//! ```
use crate::Error;
use crate::Result;
use crate::backend::default::geometry::{
    GeometryCore, GeometryInstanceData, ThemedMaterials, ThemedTextures,
};
use crate::cityjson::core::appearance::ThemeName;
use crate::cityjson::core::coordinate::Coordinate;
use crate::resources::handles::{
    GeometryTemplateHandle, MaterialHandle, SemanticHandle, TextureHandle,
};
use crate::resources::id::ResourceId32;
use crate::resources::mapping::textures::TextureMapCore;
use crate::resources::mapping::{MaterialMap, SemanticMap, SemanticOrMaterialMap, TextureMap};
use crate::resources::storage::StringStorage;
use crate::v2_0::Vertices;
use crate::v2_0::boundary::Boundary;
use crate::v2_0::boundary::{BoundaryCoordinates, BoundaryUniqueCoordinates};
use crate::v2_0::citymodel::CityModel;
use crate::v2_0::vertex::{VertexIndex, VertexRef};
use std::marker::PhantomData;
use std::ops::{Deref, Index};

pub mod semantic;
pub use crate::backend::default::geometry::AffineTransform3D;
pub use crate::cityjson::core::geometry::{GeometryType, LoD};

/// A stored geometry.
///
/// Covers all eight `CityJSON` geometry types. Use [`Geometry::type_geometry`] to determine
/// the type, then access boundaries, semantics, materials, and textures through the
/// corresponding methods.
///
/// Boundaries are stored in flat offset-encoded form. Use `boundary.to_nested_*` to get
/// nested arrays compatible with the JSON representation.
#[derive(Clone, Debug)]
pub struct Geometry<VR: VertexRef, SS: StringStorage> {
    inner: GeometryCore<VR, ResourceId32, SS>,
}

/// Public flat parts for constructing a stored [`Geometry`].
///
/// This is the raw write-layer input: callers provide the final flat topology and resource
/// maps directly, and [`Geometry::from_stored_parts`] packages them into the internal storage
/// layout without additional validation.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredGeometryParts<VR: VertexRef, SS: StringStorage> {
    pub type_geometry: GeometryType,
    pub lod: Option<LoD>,
    pub boundaries: Option<Boundary<VR>>,
    pub semantics: Option<SemanticMap<VR>>,
    pub materials: Option<Vec<(ThemeName<SS>, MaterialMap<VR>)>>,
    pub textures: Option<Vec<(ThemeName<SS>, TextureMap<VR>)>>,
    pub instance: Option<StoredGeometryInstance<VR>>,
}

/// Public stored `GeometryInstance` payload used by [`StoredGeometryParts`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoredGeometryInstance<VR: VertexRef> {
    pub template: GeometryTemplateHandle,
    pub reference_point: VertexIndex<VR>,
    pub transformation: AffineTransform3D,
}

/// Read view over the `GeometryInstance` fields of a geometry.
///
/// A `GeometryInstance` references a template geometry and places it at a point
/// in the model's vertex pool using a 4×4 transformation matrix.
/// Use [`CityModel::resolve_geometry`](super::citymodel::CityModel::resolve_geometry)
/// to get a view of the effective (resolved) geometry type.
#[derive(Clone, Copy, Debug)]
pub struct GeometryInstanceView<'a, VR: VertexRef> {
    inner: &'a GeometryInstanceData<VR, ResourceId32>,
}

impl<VR: VertexRef> GeometryInstanceView<'_, VR> {
    #[must_use]
    pub fn template(&self) -> GeometryTemplateHandle {
        GeometryTemplateHandle::from_raw(*self.inner.template())
    }

    #[must_use]
    pub fn reference_point(&self) -> VertexIndex<VR> {
        *self.inner.reference_point()
    }

    #[must_use]
    pub fn transformation(&self) -> AffineTransform3D {
        *self.inner.transformation()
    }
}

/// A read view over a geometry, optionally resolved from a `GeometryInstance`.
///
/// When obtained from [`CityModel::resolve_geometry`](super::citymodel::CityModel::resolve_geometry),
/// this view points at the effective geometry type (e.g. the `MultiSurface` that a
/// `GeometryInstance` references), with the original instance data available through
/// [`GeometryView::instance`].
#[derive(Clone, Copy, Debug)]
pub struct GeometryView<'a, VR: VertexRef, SS: StringStorage> {
    geometry: &'a Geometry<VR, SS>,
    instance: Option<GeometryInstanceView<'a, VR>>,
}

#[derive(Clone, Copy, Debug)]
pub struct HandleOptionSlice<'a, H> {
    raw: &'a [Option<ResourceId32>],
    _marker: PhantomData<H>,
}

impl<'a, H> HandleOptionSlice<'a, H> {
    fn new(raw: &'a [Option<ResourceId32>]) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    #[inline]
    fn as_handle_slice(&self) -> &'a [Option<H>] {
        const {
            assert!(
                std::mem::size_of::<Option<H>>() == std::mem::size_of::<Option<ResourceId32>>()
            );
            assert!(
                std::mem::align_of::<Option<H>>() == std::mem::align_of::<Option<ResourceId32>>()
            );
        }

        // SAFETY: handle types are `#[repr(transparent)]` wrappers over `ResourceId32`.
        // Therefore `Option<Handle>` and `Option<ResourceId32>` have identical layout.
        unsafe { std::slice::from_raw_parts(self.raw.as_ptr().cast::<Option<H>>(), self.raw.len()) }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&'a Option<H>> {
        self.as_handle_slice().get(index)
    }

    pub fn iter(&self) -> std::slice::Iter<'a, Option<H>> {
        self.as_handle_slice().iter()
    }
}

impl<H> Index<usize> for HandleOptionSlice<'_, H> {
    type Output = Option<H>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_handle_slice()[index]
    }
}

impl<'a, H: 'a> IntoIterator for HandleOptionSlice<'a, H> {
    type Item = &'a Option<H>;
    type IntoIter = std::slice::Iter<'a, Option<H>>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_handle_slice().iter()
    }
}

impl<'a, H: 'a> IntoIterator for &'_ HandleOptionSlice<'a, H> {
    type Item = &'a Option<H>;
    type IntoIter = std::slice::Iter<'a, Option<H>>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_handle_slice().iter()
    }
}

/// Read view over the semantic map of a geometry.
///
/// Exposes semantic handle assignments per primitive level:
/// `points()`, `linestrings()`, and `surfaces()`. Each returns a [`HandleOptionSlice`]
/// with one optional [`SemanticHandle`] per primitive. `None` means no semantic is assigned
/// to that primitive.
#[derive(Clone, Copy, Debug)]
pub struct SemanticMapView<'a, VR: VertexRef> {
    inner: &'a SemanticOrMaterialMap<VR, ResourceId32>,
}

impl<'a, VR: VertexRef> SemanticMapView<'a, VR> {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn points(&self) -> HandleOptionSlice<'a, SemanticHandle> {
        HandleOptionSlice::new(self.inner.points())
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn linestrings(&self) -> HandleOptionSlice<'a, SemanticHandle> {
        HandleOptionSlice::new(self.inner.linestrings())
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn surfaces(&self) -> HandleOptionSlice<'a, SemanticHandle> {
        HandleOptionSlice::new(self.inner.surfaces())
    }
}

/// Read view over a material map for one theme.
///
/// Same structure as [`SemanticMapView`] but for material handles.
#[derive(Clone, Copy, Debug)]
pub struct MaterialMapView<'a, VR: VertexRef> {
    inner: &'a SemanticOrMaterialMap<VR, ResourceId32>,
}

impl<'a, VR: VertexRef> MaterialMapView<'a, VR> {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn points(&self) -> HandleOptionSlice<'a, MaterialHandle> {
        HandleOptionSlice::new(self.inner.points())
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn linestrings(&self) -> HandleOptionSlice<'a, MaterialHandle> {
        HandleOptionSlice::new(self.inner.linestrings())
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn surfaces(&self) -> HandleOptionSlice<'a, MaterialHandle> {
        HandleOptionSlice::new(self.inner.surfaces())
    }
}

/// Read view over all material themes for a geometry.
///
/// In `CityJSON`, material assignments are grouped by theme name. Iterate with
/// [`MaterialThemesView::iter`] to get `(theme_name, MaterialMapView)` pairs.
#[derive(Clone, Copy, Debug)]
pub struct MaterialThemesView<'a, VR: VertexRef, SS: StringStorage> {
    items: &'a [(ThemeName<SS>, SemanticOrMaterialMap<VR, ResourceId32>)],
}

impl<'a, VR: VertexRef, SS: StringStorage> MaterialThemesView<'a, VR, SS> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'a ThemeName<SS>, MaterialMapView<'a, VR>)> + 'a {
        self.items
            .iter()
            .map(|(theme, map)| (theme, MaterialMapView { inner: map }))
    }

    #[must_use]
    pub fn first(&self) -> Option<(&'a ThemeName<SS>, MaterialMapView<'a, VR>)> {
        self.items
            .first()
            .map(|(theme, map)| (theme, MaterialMapView { inner: map }))
    }
}

/// Read view over the texture map for one theme.
///
/// Texture assignments in `CityJSON` associate UV coordinates (`vertices-texture`) with rings.
/// `vertices()` returns the UV index per geometry vertex (`None` = not textured),
/// `rings()` is the offset array, and `ring_textures()` gives the texture handle per ring.
#[derive(Clone, Copy, Debug)]
pub struct TextureMapView<'a, VR: VertexRef> {
    inner: &'a TextureMapCore<VR, ResourceId32>,
}

impl<'a, VR: VertexRef> TextureMapView<'a, VR> {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn vertices(&self) -> &'a [Option<VertexIndex<VR>>] {
        self.inner.vertices()
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn rings(&self) -> &'a [VertexIndex<VR>] {
        self.inner.rings()
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn ring_textures(&self) -> HandleOptionSlice<'a, TextureHandle> {
        HandleOptionSlice::new(self.inner.ring_textures())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextureThemesView<'a, VR: VertexRef, SS: StringStorage> {
    items: &'a [(ThemeName<SS>, TextureMapCore<VR, ResourceId32>)],
}

impl<'a, VR: VertexRef, SS: StringStorage> TextureThemesView<'a, VR, SS> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'a ThemeName<SS>, TextureMapView<'a, VR>)> + 'a {
        self.items
            .iter()
            .map(|(theme, map)| (theme, TextureMapView { inner: map }))
    }

    #[must_use]
    pub fn first(&self) -> Option<(&'a ThemeName<SS>, TextureMapView<'a, VR>)> {
        self.items
            .first()
            .map(|(theme, map)| (theme, TextureMapView { inner: map }))
    }
}

impl<VR: VertexRef, SS: StringStorage> Geometry<VR, SS> {
    #[must_use]
    pub fn from_stored_parts(parts: StoredGeometryParts<VR, SS>) -> Self {
        let semantics = parts.semantics.map(SemanticMap::into_raw);
        let materials = parts.materials.map(|items| {
            items
                .into_iter()
                .map(|(theme, map)| (theme, map.into_raw()))
                .collect()
        });
        let textures = parts.textures.map(|items| {
            items
                .into_iter()
                .map(|(theme, map)| (theme, map.into_raw()))
                .collect()
        });
        let instance = parts.instance.map(|instance| {
            GeometryInstanceData::new(
                instance.template.to_raw(),
                instance.reference_point,
                instance.transformation,
            )
        });

        Self::from_raw_parts(
            parts.type_geometry,
            parts.lod,
            parts.boundaries,
            semantics,
            materials,
            textures,
            instance,
        )
    }

    /// Clone this geometry into the public stored-parts representation.
    #[must_use]
    pub fn clone_stored_parts(&self) -> StoredGeometryParts<VR, SS> {
        StoredGeometryParts {
            type_geometry: *self.type_geometry(),
            lod: self.lod().copied(),
            boundaries: self.boundaries().cloned(),
            semantics: self.inner.semantics().cloned().map(SemanticMap::from_raw),
            materials: self.inner.materials().map(|items| {
                items
                    .iter()
                    .map(|(theme, map)| (theme.clone(), MaterialMap::from_raw(map.clone())))
                    .collect()
            }),
            textures: self.inner.textures().map(|items| {
                items
                    .iter()
                    .map(|(theme, map)| (theme.clone(), TextureMap::from_raw(map.clone())))
                    .collect()
            }),
            instance: self.instance().map(|instance| StoredGeometryInstance {
                template: instance.template(),
                reference_point: instance.reference_point(),
                transformation: instance.transformation(),
            }),
        }
    }

    pub(crate) fn from_raw_parts(
        type_geometry: GeometryType,
        lod: Option<LoD>,
        boundaries: Option<Boundary<VR>>,
        semantics: Option<SemanticOrMaterialMap<VR, ResourceId32>>,
        materials: Option<ThemedMaterials<VR, ResourceId32, SS>>,
        textures: Option<ThemedTextures<VR, ResourceId32, SS>>,
        instance: Option<GeometryInstanceData<VR, ResourceId32>>,
    ) -> Self {
        Self {
            inner: GeometryCore::new(
                type_geometry,
                lod,
                boundaries,
                semantics,
                materials,
                textures,
                instance,
            ),
        }
    }

    pub(crate) fn raw(&self) -> &GeometryCore<VR, ResourceId32, SS> {
        &self.inner
    }

    pub fn type_geometry(&self) -> &GeometryType {
        self.inner.type_geometry()
    }

    pub fn lod(&self) -> Option<&LoD> {
        self.inner.lod()
    }

    pub fn boundaries(&self) -> Option<&Boundary<VR>> {
        self.inner.boundaries()
    }

    #[must_use]
    pub fn coordinates<'a, V: Coordinate>(
        &'a self,
        vertices: &'a Vertices<VR, V>,
    ) -> Option<BoundaryCoordinates<'a, VR, V>> {
        self.boundaries()
            .map(|boundary| boundary.coordinates(vertices))
    }

    pub fn unique_vertex_indices<'a>(
        &'a self,
        scratch: &'a mut Vec<VertexIndex<VR>>,
    ) -> Option<&'a [VertexIndex<VR>]> {
        self.boundaries()
            .map(|boundary| boundary.unique_vertex_indices(scratch))
    }

    #[must_use]
    pub fn unique_coordinates<'a, V: Coordinate>(
        &'a self,
        vertices: &'a Vertices<VR, V>,
        scratch: &'a mut Vec<VertexIndex<VR>>,
    ) -> Option<BoundaryUniqueCoordinates<'a, VR, V>> {
        self.boundaries()
            .map(|boundary| boundary.unique_coordinates(vertices, scratch))
    }

    pub fn semantics(&self) -> Option<SemanticMapView<'_, VR>> {
        self.inner
            .semantics()
            .map(|inner| SemanticMapView { inner })
    }

    pub fn materials(&self) -> Option<MaterialThemesView<'_, VR, SS>> {
        self.inner
            .materials()
            .map(|items| MaterialThemesView { items })
    }

    pub fn textures(&self) -> Option<TextureThemesView<'_, VR, SS>> {
        self.inner
            .textures()
            .map(|items| TextureThemesView { items })
    }

    pub fn instance(&self) -> Option<GeometryInstanceView<'_, VR>> {
        self.inner
            .instance()
            .map(|inner| GeometryInstanceView { inner })
    }
}

/// Build a material map with one assignment per flattened surface primitive.
///
/// # Errors
///
/// Returns an error when `geometry` is not surface-based, is a `GeometryInstance`, lacks
/// boundaries, or when `assign` returns a material handle that does not exist in `model`.
pub fn build_surface_material_map<VR, SS, F>(
    model: &CityModel<VR, SS>,
    geometry: &Geometry<VR, SS>,
    mut assign: F,
) -> Result<MaterialMap<VR>>
where
    VR: VertexRef,
    SS: StringStorage,
    F: FnMut(usize) -> Option<MaterialHandle>,
{
    let count = surface_primitive_count(geometry)?;
    let mut map = MaterialMap::new();
    for index in 0..count {
        let handle = assign(index);
        if let Some(handle) = handle
            && model.get_material(handle).is_none()
        {
            return Err(missing_resource("material", index, handle));
        }
        map.add_surface(handle);
    }
    Ok(map)
}

/// Build a semantic map with one assignment per flattened point primitive.
///
/// # Errors
///
/// Returns an error when `geometry` is not a `MultiPoint`, is a `GeometryInstance`, lacks
/// boundaries, or when `assign` returns a semantic handle that does not exist in `model`.
pub fn build_point_semantic_map<VR, SS, F>(
    model: &CityModel<VR, SS>,
    geometry: &Geometry<VR, SS>,
    mut assign: F,
) -> Result<SemanticMap<VR>>
where
    VR: VertexRef,
    SS: StringStorage,
    F: FnMut(usize) -> Option<SemanticHandle>,
{
    require_geometry_type(geometry, GeometryType::MultiPoint)?;
    let count = geometry_boundary(geometry)?.vertices().len();
    let mut map = SemanticMap::new();
    for index in 0..count {
        let handle = assign(index);
        if let Some(handle) = handle
            && model.get_semantic(handle).is_none()
        {
            return Err(missing_resource("semantic", index, handle));
        }
        map.add_point(handle);
    }
    Ok(map)
}

/// Build a semantic map with one assignment per flattened linestring primitive.
///
/// # Errors
///
/// Returns an error when `geometry` is not a `MultiLineString`, is a `GeometryInstance`, lacks
/// boundaries, or when `assign` returns a semantic handle that does not exist in `model`.
pub fn build_linestring_semantic_map<VR, SS, F>(
    model: &CityModel<VR, SS>,
    geometry: &Geometry<VR, SS>,
    mut assign: F,
) -> Result<SemanticMap<VR>>
where
    VR: VertexRef,
    SS: StringStorage,
    F: FnMut(usize) -> Option<SemanticHandle>,
{
    require_geometry_type(geometry, GeometryType::MultiLineString)?;
    let count = geometry_boundary(geometry)?.rings().len();
    let mut map = SemanticMap::new();
    for index in 0..count {
        let handle = assign(index);
        if let Some(handle) = handle
            && model.get_semantic(handle).is_none()
        {
            return Err(missing_resource("semantic", index, handle));
        }
        map.add_linestring(handle);
    }
    Ok(map)
}

/// Build a semantic map with one assignment per flattened surface primitive.
///
/// # Errors
///
/// Returns an error when `geometry` is not surface-based, is a `GeometryInstance`, lacks
/// boundaries, or when `assign` returns a semantic handle that does not exist in `model`.
pub fn build_surface_semantic_map<VR, SS, F>(
    model: &CityModel<VR, SS>,
    geometry: &Geometry<VR, SS>,
    mut assign: F,
) -> Result<SemanticMap<VR>>
where
    VR: VertexRef,
    SS: StringStorage,
    F: FnMut(usize) -> Option<SemanticHandle>,
{
    let count = surface_primitive_count(geometry)?;
    let mut map = SemanticMap::new();
    for index in 0..count {
        let handle = assign(index);
        if let Some(handle) = handle
            && model.get_semantic(handle).is_none()
        {
            return Err(missing_resource("semantic", index, handle));
        }
        map.add_surface(handle);
    }
    Ok(map)
}

fn surface_primitive_count<VR: VertexRef, SS: StringStorage>(
    geometry: &Geometry<VR, SS>,
) -> Result<usize> {
    match geometry.type_geometry() {
        GeometryType::MultiSurface
        | GeometryType::CompositeSurface
        | GeometryType::Solid
        | GeometryType::MultiSolid
        | GeometryType::CompositeSolid => Ok(geometry_boundary(geometry)?.surfaces().len()),
        found => Err(Error::InvalidGeometryType {
            expected: "surface-based geometry".to_string(),
            found: found.to_string(),
        }),
    }
}

fn require_geometry_type<VR: VertexRef, SS: StringStorage>(
    geometry: &Geometry<VR, SS>,
    expected: GeometryType,
) -> Result<()> {
    if *geometry.type_geometry() == expected {
        return Ok(());
    }
    Err(Error::InvalidGeometryType {
        expected: expected.to_string(),
        found: geometry.type_geometry().to_string(),
    })
}

fn geometry_boundary<VR: VertexRef, SS: StringStorage>(
    geometry: &Geometry<VR, SS>,
) -> Result<&Boundary<VR>> {
    geometry.boundaries().ok_or_else(|| {
        Error::IncompleteGeometry(format!(
            "{} geometry must have a boundary",
            geometry.type_geometry()
        ))
    })
}

fn missing_resource<H: std::fmt::Display>(resource: &str, index: usize, handle: H) -> Error {
    Error::InvalidGeometry(format!(
        "{resource} assignment {index} references missing resource {handle}"
    ))
}

impl<'a, VR: VertexRef, SS: StringStorage> GeometryView<'a, VR, SS> {
    pub(crate) fn from_geometry(
        geometry: &'a Geometry<VR, SS>,
        instance: Option<GeometryInstanceView<'a, VR>>,
    ) -> Self {
        Self { geometry, instance }
    }

    #[must_use]
    pub fn geometry(&self) -> &'a Geometry<VR, SS> {
        self.geometry
    }

    #[must_use]
    pub fn instance(&self) -> Option<GeometryInstanceView<'a, VR>> {
        self.instance
    }
}

impl<VR: VertexRef, SS: StringStorage> Deref for GeometryView<'_, VR, SS> {
    type Target = Geometry<VR, SS>;

    fn deref(&self) -> &Self::Target {
        self.geometry
    }
}
