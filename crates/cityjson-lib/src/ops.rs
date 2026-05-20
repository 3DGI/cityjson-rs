//! Native workflow helpers for CityJSON models.
//!
//! The `subset` and `merge` semantics are ported from `cjio`, and the Rust
//! implementation here is the crate-owned native rewrite of those workflows.
//! Selection-driven extraction uses the opaque `ModelSelection` carrier.

use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap, HashSet};
#[cfg(feature = "proj")]
use std::fmt;
#[cfg(feature = "proj")]
use std::sync::{Arc, Mutex, OnceLock};

use crate::cityjson_types::resources::storage::OwnedStringStorage;
use crate::cityjson_types::v2_0::attributes::Attributes;
#[cfg(feature = "proj")]
use crate::cityjson_types::v2_0::coordinate::RealWorldCoordinate;
use crate::cityjson_types::v2_0::geometry::{
    Geometry, GeometryType, StoredGeometryInstance, StoredGeometryParts,
};
use crate::cityjson_types::v2_0::metadata::BBox;
#[cfg(feature = "proj")]
use crate::cityjson_types::v2_0::metadata::CRS;
use crate::cityjson_types::v2_0::{
    CityObject, CityObjectIdentifier, MaterialMap, Metadata, SemanticMap, TextureMap, Transform,
    VertexIndex,
};
use crate::cityjson_types::{
    CityModelType,
    prelude::{
        CityObjectHandle, GeometryHandle, GeometryTemplateHandle, MaterialHandle, SemanticHandle,
        TextureHandle,
    },
    v2_0::Extensions,
};
use crate::{CityModel, Error, Result};

type OwnedMetadata = Metadata<OwnedStringStorage>;
type OwnedExtensions = Extensions<OwnedStringStorage>;
type OwnedCityObject = CityObject<OwnedStringStorage>;
type OwnedGeometry = Geometry<u32, OwnedStringStorage>;
#[cfg(feature = "proj")]
type CachedProj = Arc<Mutex<crate::proj::Proj>>;

#[cfg(feature = "proj")]
static TRANSFORMER_CACHE: OnceLock<Mutex<HashMap<(String, String), CachedProj>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
enum CityObjectSelection {
    Whole,
    Partial(HashSet<GeometryHandle>),
}

/// Opaque selection carrier for selection/extraction workflows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelSelection {
    cityobjects: HashMap<CityObjectHandle, CityObjectSelection>,
}

/// Context passed to cityobject-level selection predicates.
pub struct CityObjectSelectionContext<'a> {
    model: &'a CityModel,
    handle: CityObjectHandle,
    cityobject: &'a CityObject<OwnedStringStorage>,
}

impl<'a> CityObjectSelectionContext<'a> {
    pub fn model(&self) -> &'a CityModel {
        self.model
    }

    pub fn handle(&self) -> CityObjectHandle {
        self.handle
    }

    pub fn cityobject(&self) -> &'a CityObject<OwnedStringStorage> {
        self.cityobject
    }

    pub fn id(&self) -> &'a str {
        self.cityobject.id()
    }
}

/// Context passed to geometry-level selection predicates.
pub struct GeometrySelectionContext<'a> {
    model: &'a CityModel,
    cityobject_handle: CityObjectHandle,
    cityobject: &'a CityObject<OwnedStringStorage>,
    geometry_handle: GeometryHandle,
    geometry: &'a OwnedGeometry,
    geometry_index: usize,
}

impl<'a> GeometrySelectionContext<'a> {
    pub fn model(&self) -> &'a CityModel {
        self.model
    }

    pub fn cityobject_handle(&self) -> CityObjectHandle {
        self.cityobject_handle
    }

    pub fn cityobject(&self) -> &'a CityObject<OwnedStringStorage> {
        self.cityobject
    }

    pub fn cityobject_id(&self) -> &'a str {
        self.cityobject.id()
    }

    pub fn geometry_handle(&self) -> GeometryHandle {
        self.geometry_handle
    }

    pub fn geometry(&self) -> &'a OwnedGeometry {
        self.geometry
    }

    pub fn geometry_index(&self) -> usize {
        self.geometry_index
    }
}

impl ModelSelection {
    fn select_whole(&mut self, handle: CityObjectHandle) {
        self.cityobjects.insert(handle, CityObjectSelection::Whole);
    }

    fn select_geometry(
        &mut self,
        cityobject_handle: CityObjectHandle,
        geometry_handle: GeometryHandle,
    ) {
        match self.cityobjects.entry(cityobject_handle) {
            Entry::Vacant(entry) => {
                let mut geometries = HashSet::new();
                geometries.insert(geometry_handle);
                entry.insert(CityObjectSelection::Partial(geometries));
            }
            Entry::Occupied(mut entry) => match entry.get_mut() {
                CityObjectSelection::Whole => {}
                CityObjectSelection::Partial(geometries) => {
                    geometries.insert(geometry_handle);
                }
            },
        }
    }

    /// Expand the selection through parent and child relations.
    pub fn include_relatives(self, model: &CityModel) -> Result<Self> {
        let mut selection = self;
        let roots = selection.cityobjects.keys().copied().collect::<Vec<_>>();
        let relatives = collect_reachable_cityobjects(model, roots, true, true)?;

        for handle in relatives {
            selection
                .cityobjects
                .entry(handle)
                .or_insert(CityObjectSelection::Whole);
        }

        Ok(selection)
    }

    /// Combine two selections, preferring whole-cityobject selection.
    pub fn union(&self, other: &Self) -> Self {
        let mut selection = self.clone();

        for (handle, state) in &other.cityobjects {
            match selection.cityobjects.entry(*handle) {
                Entry::Vacant(entry) => {
                    entry.insert(state.clone());
                }
                Entry::Occupied(mut entry) => {
                    let merged = match (entry.get(), state) {
                        (CityObjectSelection::Whole, _) | (_, CityObjectSelection::Whole) => {
                            CityObjectSelection::Whole
                        }
                        (CityObjectSelection::Partial(lhs), CityObjectSelection::Partial(rhs)) => {
                            let geometries =
                                lhs.union(rhs).copied().collect::<HashSet<GeometryHandle>>();
                            CityObjectSelection::Partial(geometries)
                        }
                    };
                    entry.insert(merged);
                }
            }
        }

        selection
    }

    /// Keep only the overlap between two selections.
    pub fn intersection(&self, other: &Self) -> Self {
        let mut cityobjects = HashMap::new();

        for (handle, lhs_state) in &self.cityobjects {
            let Some(rhs_state) = other.cityobjects.get(handle) else {
                continue;
            };

            let merged = match (lhs_state, rhs_state) {
                (CityObjectSelection::Whole, CityObjectSelection::Whole) => {
                    CityObjectSelection::Whole
                }
                (CityObjectSelection::Whole, CityObjectSelection::Partial(geometries))
                | (CityObjectSelection::Partial(geometries), CityObjectSelection::Whole) => {
                    CityObjectSelection::Partial(geometries.clone())
                }
                (CityObjectSelection::Partial(lhs), CityObjectSelection::Partial(rhs)) => {
                    let geometries = lhs.intersection(rhs).copied().collect::<HashSet<_>>();
                    if geometries.is_empty() {
                        continue;
                    }
                    CityObjectSelection::Partial(geometries)
                }
            };

            cityobjects.insert(*handle, merged);
        }

        Self { cityobjects }
    }

    /// Return `true` when no CityObjects are selected.
    pub fn is_empty(&self) -> bool {
        self.cityobjects.is_empty()
    }
}

fn import_error(message: impl Into<String>) -> Error {
    Error::Import(message.into())
}

#[cfg(feature = "proj")]
fn projection_error(message: impl Into<String>) -> Error {
    Error::Projection(message.into())
}

#[cfg(feature = "proj")]
fn canonical_crs(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(code) = trimmed.strip_prefix("EPSG:")
        && let Ok(parsed) = code.parse::<u32>()
    {
        return format!("EPSG:{parsed}");
    }

    if let Some(code) = trimmed.rsplit(['/', ':']).find(|part| !part.is_empty())
        && let Ok(parsed) = code.parse::<u32>()
    {
        return format!("EPSG:{parsed}");
    }

    trimmed.to_owned()
}

/// Cached coordinate transformer backed by PROJ.
#[cfg(feature = "proj")]
#[derive(Clone)]
pub struct Transformer {
    inner: CachedProj,
}

#[cfg(feature = "proj")]
impl fmt::Debug for Transformer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transformer").finish_non_exhaustive()
    }
}

#[cfg(feature = "proj")]
impl Transformer {
    /// Transform one `[x, y, z]` point.
    ///
    /// # Errors
    ///
    /// Returns an error when PROJ rejects the point or the cached transformer lock is poisoned.
    pub fn transform(&self, point: [f64; 3]) -> Result<[f64; 3]> {
        let transformer = self
            .inner
            .lock()
            .map_err(|_| projection_error("cached PROJ transformer lock is poisoned"))?;
        let output = transformer
            .convert((point[0], point[1], point[2]))
            .map_err(|error| projection_error(error.to_string()))?;
        Ok([output.0, output.1, output.2])
    }
}

/// Return a cached transformer for the CRS pair.
///
/// # Errors
///
/// Returns an error when PROJ cannot create the CRS operation.
#[cfg(feature = "proj")]
pub fn transformer(source_crs: &str, target_crs: &str) -> Result<Transformer> {
    let key = (canonical_crs(source_crs), canonical_crs(target_crs));
    let cache = TRANSFORMER_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache
        .lock()
        .map_err(|_| projection_error("PROJ transformer cache lock is poisoned"))?;

    if let Some(transformer) = cache.get(&key) {
        return Ok(Transformer {
            inner: Arc::clone(transformer),
        });
    }

    let created = crate::proj::Proj::new_known_crs(&key.0, &key.1, None)
        .map_err(|error| projection_error(error.to_string()))?;
    let created = Arc::new(Mutex::new(created));
    cache.insert(key, Arc::clone(&created));

    Ok(Transformer { inner: created })
}

/// Reproject the main vertex pool into `target_crs`.
///
/// The input model is consumed. Only `vertices` are reprojected; template vertices and
/// geometry-instance transforms are preserved. `cityjson-lib` stores parsed vertices as
/// real-world coordinates, so any root transform is removed after projection because vertices
/// are returned in target real-world coordinates.
///
/// # Errors
///
/// Returns an error when `metadata.referenceSystem` is absent or PROJ cannot transform a vertex.
#[cfg(feature = "proj")]
pub fn reproject(mut model: CityModel, target_crs: &str) -> Result<CityModel> {
    let source_crs = model
        .metadata()
        .and_then(|metadata| metadata.reference_system())
        .ok_or_else(|| projection_error("CityJSON metadata.referenceSystem is missing"))?
        .to_string();
    let target_crs = canonical_crs(target_crs);
    let transformer = transformer(&source_crs, &target_crs)?;

    for vertex in model.vertices_mut().as_mut_slice() {
        let projected = transformer.transform(vertex.to_array())?;
        *vertex = RealWorldCoordinate::new(projected[0], projected[1], projected[2]);
    }

    model.clear_transform();
    model
        .metadata_mut()
        .set_reference_system(CRS::new(target_crs));

    Ok(model)
}

#[derive(Debug, Clone, PartialEq)]
enum TransformMergeState {
    Empty,
    Present(Transform),
    Cleared,
}

impl TransformMergeState {
    fn from_model(model: &CityModel) -> Self {
        match model.transform() {
            Some(transform) => Self::Present(transform.clone()),
            None => Self::Empty,
        }
    }
}

fn reconcile_transform_state(
    current: TransformMergeState,
    source: Option<&Transform>,
) -> TransformMergeState {
    match (current, source) {
        (TransformMergeState::Empty, None) => TransformMergeState::Empty,
        (TransformMergeState::Empty, Some(transform)) => {
            TransformMergeState::Present(transform.clone())
        }
        (TransformMergeState::Present(transform), None) => TransformMergeState::Present(transform),
        (TransformMergeState::Present(transform), Some(source_transform))
            if transform == *source_transform =>
        {
            TransformMergeState::Present(transform)
        }
        (TransformMergeState::Cleared, _) | (TransformMergeState::Present(_), Some(_)) => {
            TransformMergeState::Cleared
        }
    }
}

fn strip_transform(model: &CityModel) -> Result<CityModel> {
    let mut untransformed = model.clone();
    untransformed.clear_transform();
    Ok(untransformed)
}

fn apply_transform_state(target: &mut CityModel, state: &TransformMergeState) -> Result<()> {
    match state {
        TransformMergeState::Empty => {
            if target.transform().is_some() {
                *target = strip_transform(target)?;
            }
        }
        TransformMergeState::Present(transform) => {
            if target.transform().is_none() {
                *target.transform_mut() = transform.clone();
            } else if target.transform() != Some(transform) {
                *target = strip_transform(target)?;
                *target.transform_mut() = transform.clone();
            }
        }
        TransformMergeState::Cleared => {
            if target.transform().is_some() {
                *target = strip_transform(target)?;
            }
        }
    }

    Ok(())
}

fn append_kind_compatible(target_kind: CityModelType, source_kind: CityModelType) -> bool {
    target_kind == source_kind
        || (target_kind == CityModelType::CityJSON && source_kind == CityModelType::CityJSONFeature)
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

fn merge_attributes(
    target: &mut Attributes<OwnedStringStorage>,
    source: &Attributes<OwnedStringStorage>,
) {
    for (key, value) in source.iter() {
        target.insert(key.clone(), value.clone());
    }
}

fn merge_cityobject_extent(target: &mut OwnedCityObject, source: &OwnedCityObject) {
    match (
        target.geographical_extent().copied(),
        source.geographical_extent().copied(),
    ) {
        (None, Some(extent)) => target.set_geographical_extent(Some(extent)),
        (Some(lhs), Some(rhs)) if lhs != rhs => {
            target.set_geographical_extent(Some(union_bbox(lhs, rhs)))
        }
        _ => {}
    }
}

fn merge_metadata(target: &mut OwnedMetadata, source: &OwnedMetadata) {
    if target.geographical_extent().is_none()
        && let Some(extent) = source.geographical_extent().copied()
    {
        target.set_geographical_extent(extent);
    } else if let (Some(lhs), Some(rhs)) = (
        target.geographical_extent().copied(),
        source.geographical_extent().copied(),
    ) && lhs != rhs
    {
        target.set_geographical_extent(union_bbox(lhs, rhs));
    }

    if target.identifier().is_none()
        && let Some(identifier) = source.identifier().cloned()
    {
        target.set_identifier(identifier);
    }

    if target.reference_date().is_none()
        && let Some(date) = source.reference_date().cloned()
    {
        target.set_reference_date(date);
    }

    if target.reference_system().is_none()
        && let Some(crs) = source.reference_system().cloned()
    {
        target.set_reference_system(crs);
    }

    if target.title().is_none()
        && let Some(title) = source.title()
    {
        target.set_title(title.to_owned());
    }

    if target.point_of_contact().is_none()
        && let Some(contact) = source.point_of_contact().cloned()
    {
        target.set_point_of_contact(Some(contact));
    }

    if let Some(extra) = source.extra() {
        let target_extra = target.extra_mut();
        for (key, value) in extra.iter() {
            target_extra.insert(key.clone(), value.clone());
        }
    }
}

fn merge_root_extensions(target: &mut OwnedExtensions, source: &OwnedExtensions) {
    for extension in source {
        target.add(extension.clone());
    }
}

fn remap_vertex_indices(
    boundary: &crate::cityjson_types::v2_0::boundary::Boundary<u32>,
    vertex_map: &[VertexIndex<u32>],
) -> Result<crate::cityjson_types::v2_0::boundary::Boundary<u32>> {
    let mut boundary = boundary.clone();
    let remapped = boundary.vertices().iter().map(|index| {
        vertex_map
            .get(index.to_usize())
            .copied()
            .ok_or_else(|| import_error(format!("vertex index {} is out of range", index.value())))
    });
    boundary.set_vertices_from_iter(remapped.collect::<Result<Vec<_>>>()?);
    Ok(boundary)
}

fn remap_texture_map(
    map: &crate::cityjson_types::v2_0::geometry::TextureMapView<'_, u32>,
    uv_map: &[VertexIndex<u32>],
    texture_map: &HashMap<TextureHandle, TextureHandle>,
) -> Result<TextureMap<u32>> {
    let mut remapped = TextureMap::new();

    for vertex in map.vertices() {
        let mapped = vertex
            .map(|index| {
                uv_map.get(index.to_usize()).copied().ok_or_else(|| {
                    import_error(format!("uv vertex index {} is out of range", index.value()))
                })
            })
            .transpose()?;
        remapped.add_vertex(mapped);
    }

    for ring in map.rings() {
        remapped.add_ring(*ring);
    }

    for texture in map.ring_textures() {
        remapped.add_ring_texture(
            texture.map(|handle| texture_map.get(&handle).copied().unwrap_or(handle)),
        );
    }

    Ok(remapped)
}

fn remap_material_map<'a, I, J, K>(
    points: I,
    linestrings: J,
    surfaces: K,
    material_map: &HashMap<MaterialHandle, MaterialHandle>,
) -> MaterialMap<u32>
where
    I: IntoIterator<Item = &'a Option<MaterialHandle>>,
    J: IntoIterator<Item = &'a Option<MaterialHandle>>,
    K: IntoIterator<Item = &'a Option<MaterialHandle>>,
{
    let mut remapped = MaterialMap::new();

    for item in points {
        remapped.add_point(match item {
            Some(handle) => Some(material_map.get(handle).copied().unwrap_or(*handle)),
            None => None,
        });
    }
    for item in linestrings {
        remapped.add_linestring(match item {
            Some(handle) => Some(material_map.get(handle).copied().unwrap_or(*handle)),
            None => None,
        });
    }
    for item in surfaces {
        remapped.add_surface(match item {
            Some(handle) => Some(material_map.get(handle).copied().unwrap_or(*handle)),
            None => None,
        });
    }

    remapped
}

fn remap_semantic_map<'a, I, J, K>(
    points: I,
    linestrings: J,
    surfaces: K,
    semantic_map: &HashMap<SemanticHandle, SemanticHandle>,
) -> SemanticMap<u32>
where
    I: IntoIterator<Item = &'a Option<SemanticHandle>>,
    J: IntoIterator<Item = &'a Option<SemanticHandle>>,
    K: IntoIterator<Item = &'a Option<SemanticHandle>>,
{
    let mut remapped = SemanticMap::new();

    for item in points {
        remapped.add_point(match item {
            Some(handle) => Some(semantic_map.get(handle).copied().unwrap_or(*handle)),
            None => None,
        });
    }
    for item in linestrings {
        remapped.add_linestring(match item {
            Some(handle) => Some(semantic_map.get(handle).copied().unwrap_or(*handle)),
            None => None,
        });
    }
    for item in surfaces {
        remapped.add_surface(match item {
            Some(handle) => Some(semantic_map.get(handle).copied().unwrap_or(*handle)),
            None => None,
        });
    }

    remapped
}

fn remap_geometry(
    geometry: &OwnedGeometry,
    vertex_map: &[VertexIndex<u32>],
    template_map: &HashMap<GeometryTemplateHandle, GeometryTemplateHandle>,
    semantic_map: &HashMap<SemanticHandle, SemanticHandle>,
    material_map: &HashMap<MaterialHandle, MaterialHandle>,
    texture_map: &HashMap<TextureHandle, TextureHandle>,
    uv_map: &[VertexIndex<u32>],
) -> Result<OwnedGeometry> {
    let stored_parts = if let Some(instance) = geometry.instance() {
        let template = template_map
            .get(&instance.template())
            .copied()
            .ok_or_else(|| {
                import_error(format!(
                    "missing remap for geometry template {}",
                    instance.template()
                ))
            })?;
        Geometry::from_stored_parts(StoredGeometryParts {
            type_geometry: GeometryType::GeometryInstance,
            lod: None,
            boundaries: None,
            semantics: None,
            materials: None,
            textures: None,
            instance: Some(StoredGeometryInstance {
                template,
                reference_point: *vertex_map
                    .get(instance.reference_point().to_usize())
                    .ok_or_else(|| {
                        import_error(format!(
                            "vertex index {} is out of range",
                            instance.reference_point().value()
                        ))
                    })?,
                transformation: instance.transformation(),
            }),
        })
    } else {
        let boundaries = geometry
            .boundaries()
            .map(|boundary| remap_vertex_indices(boundary, vertex_map))
            .transpose()?;

        let semantics = geometry.semantics().map(|theme| {
            let points = theme.points();
            let linestrings = theme.linestrings();
            let surfaces = theme.surfaces();
            remap_semantic_map(
                points.iter(),
                linestrings.iter(),
                surfaces.iter(),
                semantic_map,
            )
        });

        let materials = geometry.materials().map(|themes| {
            themes
                .iter()
                .map(|(name, theme)| {
                    let points = theme.points();
                    let linestrings = theme.linestrings();
                    let surfaces = theme.surfaces();
                    (
                        name.clone(),
                        remap_material_map(
                            points.iter(),
                            linestrings.iter(),
                            surfaces.iter(),
                            material_map,
                        ),
                    )
                })
                .collect::<Vec<_>>()
        });

        let textures = geometry
            .textures()
            .map(|themes| {
                themes
                    .iter()
                    .map(|(name, theme)| {
                        remap_texture_map(&theme, uv_map, texture_map)
                            .map(|map| (name.clone(), map))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        Geometry::from_stored_parts(StoredGeometryParts {
            type_geometry: *geometry.type_geometry(),
            lod: geometry.lod().copied(),
            boundaries,
            semantics,
            materials,
            textures,
            instance: None,
        })
    };

    Ok(stored_parts)
}

fn append_vertices(target: &mut CityModel, source: &CityModel) -> Result<Vec<VertexIndex<u32>>> {
    let mut map = Vec::with_capacity(source.vertices().len());
    for vertex in source.vertices().as_slice() {
        map.push(target.add_vertex(*vertex)?);
    }
    Ok(map)
}

fn append_template_vertices(
    target: &mut CityModel,
    source: &CityModel,
) -> Result<Vec<VertexIndex<u32>>> {
    let mut map = Vec::with_capacity(source.template_vertices().len());
    for vertex in source.template_vertices().as_slice() {
        map.push(target.add_template_vertex(*vertex)?);
    }
    Ok(map)
}

fn append_uv_vertices(target: &mut CityModel, source: &CityModel) -> Result<Vec<VertexIndex<u32>>> {
    let mut map = Vec::with_capacity(source.vertices_texture().len());
    for uv in source.vertices_texture().as_slice() {
        map.push(target.add_uv_coordinate((*uv).clone())?);
    }
    Ok(map)
}

fn append_semantics(
    target: &mut CityModel,
    source: &CityModel,
) -> Result<HashMap<SemanticHandle, SemanticHandle>> {
    let mut map = HashMap::with_capacity(source.semantic_count());
    for (handle, semantic) in source.iter_semantics() {
        map.insert(handle, target.add_semantic(semantic.clone())?);
    }
    Ok(map)
}

fn append_materials(
    target: &mut CityModel,
    source: &CityModel,
) -> Result<HashMap<MaterialHandle, MaterialHandle>> {
    let mut map = HashMap::with_capacity(source.material_count());
    for (handle, material) in source.iter_materials() {
        map.insert(handle, target.add_material(material.clone())?);
    }
    Ok(map)
}

fn append_textures(
    target: &mut CityModel,
    source: &CityModel,
) -> Result<HashMap<TextureHandle, TextureHandle>> {
    let mut map = HashMap::with_capacity(source.texture_count());
    for (handle, texture) in source.iter_textures() {
        map.insert(handle, target.add_texture(texture.clone())?);
    }
    Ok(map)
}

fn append_geometry_templates(
    target: &mut CityModel,
    source: &CityModel,
    template_vertex_map: &[VertexIndex<u32>],
    template_map: &HashMap<GeometryTemplateHandle, GeometryTemplateHandle>,
    semantic_map: &HashMap<SemanticHandle, SemanticHandle>,
    material_map: &HashMap<MaterialHandle, MaterialHandle>,
    texture_map: &HashMap<TextureHandle, TextureHandle>,
    uv_map: &[VertexIndex<u32>],
) -> Result<HashMap<GeometryTemplateHandle, GeometryTemplateHandle>> {
    let mut map = HashMap::with_capacity(source.geometry_template_count());
    for (handle, geometry) in source.iter_geometry_templates() {
        let remapped = remap_geometry(
            geometry,
            template_vertex_map,
            template_map,
            semantic_map,
            material_map,
            texture_map,
            uv_map,
        )?;
        map.insert(handle, target.add_geometry_template(remapped)?);
    }
    Ok(map)
}

fn append_geometries(
    target: &mut CityModel,
    source: &CityModel,
    vertex_map: &[VertexIndex<u32>],
    template_map: &HashMap<GeometryTemplateHandle, GeometryTemplateHandle>,
    semantic_map: &HashMap<SemanticHandle, SemanticHandle>,
    material_map: &HashMap<MaterialHandle, MaterialHandle>,
    texture_map: &HashMap<TextureHandle, TextureHandle>,
    uv_map: &[VertexIndex<u32>],
) -> Result<HashMap<GeometryHandle, GeometryHandle>> {
    let mut map = HashMap::with_capacity(source.geometry_count());
    for (handle, geometry) in source.iter_geometries() {
        let remapped = remap_geometry(
            geometry,
            vertex_map,
            template_map,
            semantic_map,
            material_map,
            texture_map,
            uv_map,
        )?;
        map.insert(handle, target.add_geometry(remapped)?);
    }
    Ok(map)
}

fn merge_cityobject(
    target: &mut OwnedCityObject,
    source: &OwnedCityObject,
    cityobject_map: &HashMap<CityObjectHandle, CityObjectHandle>,
    geometry_map: &HashMap<GeometryHandle, GeometryHandle>,
) -> Result<()> {
    if target.type_cityobject() != source.type_cityobject() {
        return Err(import_error(format!(
            "conflicting CityObject types for '{}'",
            target.id()
        )));
    }

    if let Some(attributes) = source.attributes() {
        merge_attributes(target.attributes_mut(), attributes);
    }
    merge_cityobject_extent(target, source);

    if let Some(extra) = source.extra() {
        let target_extra = target.extra_mut();
        for (key, value) in extra.iter() {
            target_extra.insert(key.clone(), value.clone());
        }
    }

    if let Some(geometry_handles) = source.geometry() {
        let mut target_geometry = target
            .geometry()
            .map(|items| items.to_vec())
            .unwrap_or_default();
        for geometry in geometry_handles {
            let mapped = geometry_map.get(geometry).copied().ok_or_else(|| {
                import_error(format!(
                    "missing remap for geometry {}",
                    geometry.raw_parts().0
                ))
            })?;
            if !target_geometry.contains(&mapped) {
                target.add_geometry(mapped);
                target_geometry.push(mapped);
            }
        }
    }

    if let Some(children) = source.children() {
        let mut existing = target
            .children()
            .map(|items| items.to_vec())
            .unwrap_or_default();
        for child in children {
            let mapped = cityobject_map.get(child).copied().ok_or_else(|| {
                import_error(format!(
                    "missing remap for cityobject {}",
                    child.raw_parts().0
                ))
            })?;
            if !existing.contains(&mapped) {
                target.add_child(mapped);
                existing.push(mapped);
            }
        }
    }

    if let Some(parents) = source.parents() {
        let mut existing = target
            .parents()
            .map(|items| items.to_vec())
            .unwrap_or_default();
        for parent in parents {
            let mapped = cityobject_map.get(parent).copied().ok_or_else(|| {
                import_error(format!(
                    "missing remap for cityobject {}",
                    parent.raw_parts().0
                ))
            })?;
            if !existing.contains(&mapped) {
                target.add_parent(mapped);
                existing.push(mapped);
            }
        }
    }

    Ok(())
}

fn merge_one(
    target: &mut CityModel,
    source: &CityModel,
    transform_state: &mut TransformMergeState,
) -> Result<()> {
    if !append_kind_compatible(target.type_citymodel(), source.type_citymodel()) {
        return Err(import_error(
            "model merge currently requires compatible root types",
        ));
    }

    *transform_state = reconcile_transform_state(transform_state.clone(), source.transform());

    if target.metadata().is_none() {
        if let Some(metadata) = source.metadata() {
            *target.metadata_mut() = metadata.clone();
        }
    } else if let Some(source_metadata) = source.metadata() {
        merge_metadata(target.metadata_mut(), source_metadata);
    }

    if target.extra().is_none() {
        if let Some(extra) = source.extra() {
            *target.extra_mut() = extra.clone();
        }
    } else if let Some(extra) = source.extra() {
        let target_extra = target.extra_mut();
        for (key, value) in extra.iter() {
            target_extra.insert(key.clone(), value.clone());
        }
    }

    if target.extensions().is_none() {
        if let Some(extensions) = source.extensions() {
            *target.extensions_mut() = extensions.clone();
        }
    } else if let Some(extensions) = source.extensions() {
        merge_root_extensions(target.extensions_mut(), extensions);
    }

    if target.default_material_theme().is_none()
        && let Some(theme) = source.default_material_theme().cloned()
    {
        target.set_default_material_theme(Some(theme));
    }

    if target.default_texture_theme().is_none()
        && let Some(theme) = source.default_texture_theme().cloned()
    {
        target.set_default_texture_theme(Some(theme));
    }

    let vertex_map = append_vertices(target, source)?;
    let template_vertex_map = append_template_vertices(target, source)?;
    let uv_map = append_uv_vertices(target, source)?;
    let semantic_map = append_semantics(target, source)?;
    let material_map = append_materials(target, source)?;
    let texture_map = append_textures(target, source)?;
    let empty_template_map: HashMap<GeometryTemplateHandle, GeometryTemplateHandle> =
        HashMap::new();
    let template_map = append_geometry_templates(
        target,
        source,
        &template_vertex_map,
        &empty_template_map,
        &semantic_map,
        &material_map,
        &texture_map,
        &uv_map,
    )?;
    let geometry_map = append_geometries(
        target,
        source,
        &vertex_map,
        &template_map,
        &semantic_map,
        &material_map,
        &texture_map,
        &uv_map,
    )?;

    let mut cityobject_map = HashMap::with_capacity(source.cityobjects().len());
    for (handle, source_cityobject) in source.cityobjects().iter() {
        if let Some(existing) = target
            .cityobjects()
            .iter()
            .find(|(_, cityobject)| cityobject.id() == source_cityobject.id())
            .map(|(handle, _)| handle)
        {
            cityobject_map.insert(handle, existing);
            continue;
        }

        let placeholder = CityObject::new(
            CityObjectIdentifier::new(source_cityobject.id().to_owned()),
            source_cityobject.type_cityobject().clone(),
        );
        let new_handle = target.cityobjects_mut().add(placeholder)?;
        cityobject_map.insert(handle, new_handle);
    }

    if target.id().is_none()
        && let Some(source_id) = source.id()
        && let Some(mapped) = cityobject_map.get(&source_id).copied()
    {
        target.set_id(Some(mapped));
    }

    for (handle, source_cityobject) in source.cityobjects().iter() {
        let target_handle = cityobject_map.get(&handle).copied().ok_or_else(|| {
            import_error(format!(
                "missing remap for cityobject {}",
                source_cityobject.id()
            ))
        })?;
        let target_cityobject =
            target
                .cityobjects_mut()
                .get_mut(target_handle)
                .ok_or_else(|| {
                    import_error(format!(
                        "missing target cityobject for {}",
                        source_cityobject.id()
                    ))
                })?;
        merge_cityobject(
            target_cityobject,
            source_cityobject,
            &cityobject_map,
            &geometry_map,
        )?;
    }

    Ok(())
}

pub fn cleanup(model: &CityModel) -> Result<CityModel> {
    cityjson_json::cleanup(model).map_err(Error::from)
}

fn collect_reachable_cityobjects<I>(
    model: &CityModel,
    roots: I,
    include_parents: bool,
    include_children: bool,
) -> Result<HashSet<CityObjectHandle>>
where
    I: IntoIterator<Item = CityObjectHandle>,
{
    let mut selected = HashSet::new();
    let mut stack = roots.into_iter().collect::<Vec<_>>();

    while let Some(handle) = stack.pop() {
        let cityobject = model.cityobjects().get(handle).ok_or_else(|| {
            import_error(format!(
                "missing CityObject handle in traversal: {handle:?}"
            ))
        })?;
        if !selected.insert(handle) {
            continue;
        }

        if include_children && let Some(children) = cityobject.children() {
            stack.extend(children.iter().copied());
        }

        if include_parents && let Some(parents) = cityobject.parents() {
            stack.extend(parents.iter().copied());
        }
    }

    Ok(selected)
}

fn selected_geometry_handles(
    model: &CityModel,
    cityobject: &OwnedCityObject,
    state: &CityObjectSelection,
) -> Result<Vec<GeometryHandle>> {
    match state {
        CityObjectSelection::Whole => {
            let Some(original_geometry) = cityobject.geometry() else {
                return Ok(Vec::new());
            };

            let mut selected = Vec::with_capacity(original_geometry.len());
            for geometry in original_geometry {
                model.get_geometry(*geometry).ok_or_else(|| {
                    import_error(format!(
                        "selected geometry handle {:?} is missing from the source model",
                        geometry
                    ))
                })?;
                selected.push(*geometry);
            }

            Ok(selected)
        }
        CityObjectSelection::Partial(geometry_handles) => {
            let Some(original_geometry) = cityobject.geometry() else {
                if geometry_handles.is_empty() {
                    return Ok(Vec::new());
                }

                let missing =
                    geometry_handles.iter().copied().next().expect(
                        "partial selection with missing geometry requires at least one handle",
                    );
                return Err(import_error(format!(
                    "selected geometry handle {:?} is missing from CityObject {}",
                    missing,
                    cityobject.id()
                )));
            };

            let available = original_geometry.iter().copied().collect::<HashSet<_>>();
            for geometry in geometry_handles {
                if !available.contains(geometry) {
                    return Err(import_error(format!(
                        "selected geometry handle {:?} is missing from CityObject {}",
                        geometry,
                        cityobject.id()
                    )));
                }
            }

            let mut selected = Vec::with_capacity(geometry_handles.len());
            for geometry in original_geometry {
                if geometry_handles.contains(geometry) {
                    model.get_geometry(*geometry).ok_or_else(|| {
                        import_error(format!(
                            "selected geometry handle {:?} is missing from the source model",
                            geometry
                        ))
                    })?;
                    selected.push(*geometry);
                }
            }

            Ok(selected)
        }
    }
}

fn rebuild_model_with_selection(
    model: &CityModel,
    selection: &ModelSelection,
) -> Result<CityModel> {
    let mut result = model.clone();
    result.clear_cityobjects();

    let mut old_to_new = HashMap::with_capacity(selection.cityobjects.len());
    let mut kept = HashSet::with_capacity(selection.cityobjects.len());

    for (handle, cityobject) in model.cityobjects().iter() {
        let Some(state) = selection.cityobjects.get(&handle) else {
            continue;
        };

        let geometry_handles = selected_geometry_handles(model, cityobject, state)?;
        if matches!(state, CityObjectSelection::Partial(_)) && geometry_handles.is_empty() {
            continue;
        }

        let mut cloned = cityobject.clone();
        cloned.clear_children();
        cloned.clear_parents();
        cloned.clear_geometry();
        for geometry in &geometry_handles {
            cloned.add_geometry(*geometry);
        }
        let new_handle = result.cityobjects_mut().add(cloned)?;
        old_to_new.insert(handle, new_handle);
        kept.insert(handle);
    }

    for (handle, cityobject) in model.cityobjects().iter() {
        if !kept.contains(&handle) {
            continue;
        }

        let target_handle = *old_to_new.get(&handle).ok_or_else(|| {
            import_error(format!("missing remap for CityObject {}", cityobject.id()))
        })?;
        let target = result
            .cityobjects_mut()
            .get_mut(target_handle)
            .ok_or_else(|| {
                import_error(format!("missing target CityObject {}", cityobject.id()))
            })?;

        if let Some(children) = cityobject.children() {
            for child in children {
                model.cityobjects().get(*child).ok_or_else(|| {
                    import_error(format!("missing child CityObject handle {child:?}"))
                })?;
                if let Some(mapped) = old_to_new.get(child).copied() {
                    target.add_child(mapped);
                }
            }
        }

        if let Some(parents) = cityobject.parents() {
            for parent in parents {
                model.cityobjects().get(*parent).ok_or_else(|| {
                    import_error(format!("missing parent CityObject handle {parent:?}"))
                })?;
                if let Some(mapped) = old_to_new.get(parent).copied() {
                    target.add_parent(mapped);
                }
            }
        }
    }

    if let Some(root) = select_feature_root(model, &kept)? {
        let mapped_root = old_to_new.get(&root).copied().ok_or_else(|| {
            import_error("feature root selected for rebuild does not exist in the rebuilt model")
        })?;
        result.set_id(Some(mapped_root));
    }

    Ok(result)
}

fn rebuild_model_with_cityobjects(
    model: &CityModel,
    selected: &HashSet<CityObjectHandle>,
) -> Result<CityModel> {
    let mut result = model.clone();
    result.clear_cityobjects();

    let mut old_to_new = HashMap::with_capacity(selected.len());
    for (handle, cityobject) in model.cityobjects().iter() {
        if !selected.contains(&handle) {
            continue;
        }

        let mut cloned = cityobject.clone();
        cloned.clear_children();
        cloned.clear_parents();
        let new_handle = result.cityobjects_mut().add(cloned)?;
        old_to_new.insert(handle, new_handle);
    }

    for (handle, cityobject) in model.cityobjects().iter() {
        if !selected.contains(&handle) {
            continue;
        }

        let target_handle = *old_to_new.get(&handle).ok_or_else(|| {
            import_error(format!("missing remap for CityObject {}", cityobject.id()))
        })?;
        let target = result
            .cityobjects_mut()
            .get_mut(target_handle)
            .ok_or_else(|| {
                import_error(format!("missing target CityObject {}", cityobject.id()))
            })?;

        if let Some(children) = cityobject.children() {
            for child in children {
                model.cityobjects().get(*child).ok_or_else(|| {
                    import_error(format!("missing child CityObject handle {child:?}"))
                })?;
                if let Some(mapped) = old_to_new.get(child).copied() {
                    target.add_child(mapped);
                }
            }
        }

        if let Some(parents) = cityobject.parents() {
            for parent in parents {
                model.cityobjects().get(*parent).ok_or_else(|| {
                    import_error(format!("missing parent CityObject handle {parent:?}"))
                })?;
                if let Some(mapped) = old_to_new.get(parent).copied() {
                    target.add_parent(mapped);
                }
            }
        }
    }

    if let Some(root) = select_feature_root(model, selected)? {
        let mapped_root = old_to_new.get(&root).copied().ok_or_else(|| {
            import_error("feature root selected for rebuild does not exist in the rebuilt model")
        })?;
        result.set_id(Some(mapped_root));
    }

    Ok(result)
}

fn select_feature_root(
    model: &CityModel,
    selected: &HashSet<CityObjectHandle>,
) -> Result<Option<CityObjectHandle>> {
    let Some(root) = model.id() else {
        return Ok(None);
    };

    model
        .cityobjects()
        .get(root)
        .ok_or_else(|| import_error("feature root references a missing CityObject"))?;

    if selected.contains(&root) {
        return Ok(Some(root));
    }

    for (handle, cityobject) in model.cityobjects().iter() {
        if !selected.contains(&handle) {
            continue;
        }

        let is_parentless = cityobject
            .parents()
            .is_none_or(|parents| parents.iter().all(|parent| !selected.contains(parent)));
        if is_parentless {
            return Ok(Some(handle));
        }
    }

    Err(import_error(
        "feature root was removed and no parentless CityObject remained",
    ))
}

pub fn subset<'a, I>(model: &CityModel, cityobject_ids: I, exclude: bool) -> Result<CityModel>
where
    I: IntoIterator<Item = &'a str>,
{
    let ids = cityobject_ids
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return Err(import_error(
            "subset requires at least one CityObject identifier",
        ));
    }

    let id_to_handle = model
        .cityobjects()
        .iter()
        .map(|(handle, cityobject)| (cityobject.id().to_owned(), handle))
        .collect::<HashMap<_, _>>();

    let mut roots = Vec::new();
    let mut matched_any = false;

    for id in &ids {
        if let Some(handle) = id_to_handle.get(id).copied() {
            matched_any = true;
            roots.push(handle);
        }
    }

    if !matched_any {
        return Err(import_error("subset selection matched no CityObjects"));
    }

    let mut selected = collect_reachable_cityobjects(model, roots, false, true)?;

    if exclude {
        let excluded = selected;
        selected = model
            .cityobjects()
            .iter()
            .map(|(handle, _)| handle)
            .filter(|handle| !excluded.contains(handle))
            .collect();
    }

    rebuild_model_with_cityobjects(model, &selected)
}

/// Build a CityObject selection by evaluating a predicate over each CityObject.
pub fn select_cityobjects<F>(model: &CityModel, mut predicate: F) -> Result<ModelSelection>
where
    F: FnMut(CityObjectSelectionContext<'_>) -> bool,
{
    let mut selection = ModelSelection::default();

    for (handle, cityobject) in model.cityobjects().iter() {
        if predicate(CityObjectSelectionContext {
            model,
            handle,
            cityobject,
        }) {
            selection.select_whole(handle);
        }
    }

    Ok(selection)
}

/// Build a geometry selection by evaluating a predicate over every referenced geometry.
pub fn select_geometries<F>(model: &CityModel, mut predicate: F) -> Result<ModelSelection>
where
    F: FnMut(GeometrySelectionContext<'_>) -> bool,
{
    let mut selection = ModelSelection::default();

    for (cityobject_handle, cityobject) in model.cityobjects().iter() {
        let Some(geometry_handles) = cityobject.geometry() else {
            continue;
        };

        for (geometry_index, geometry_handle) in geometry_handles.iter().copied().enumerate() {
            let geometry = model.get_geometry(geometry_handle).ok_or_else(|| {
                import_error(format!(
                    "geometry handle {:?} referenced by CityObject {} is missing from the source model",
                    geometry_handle,
                    cityobject.id()
                ))
            })?;

            if predicate(GeometrySelectionContext {
                model,
                cityobject_handle,
                cityobject,
                geometry_handle,
                geometry,
                geometry_index,
            }) {
                selection.select_geometry(cityobject_handle, geometry_handle);
            }
        }
    }

    Ok(selection)
}

/// Rebuild a model from a selection.
pub fn extract(model: &CityModel, selection: &ModelSelection) -> Result<CityModel> {
    rebuild_model_with_selection(model, selection)
}

pub fn append(target: &mut CityModel, source: &CityModel) -> Result<()> {
    let mut transform_state = TransformMergeState::from_model(target);
    merge_one(target, source, &mut transform_state)?;
    apply_transform_state(target, &transform_state)
}

pub fn merge<I>(models: I) -> Result<CityModel>
where
    I: IntoIterator<Item = CityModel>,
{
    let mut models = models.into_iter();
    let Some(mut merged) = models.next() else {
        return Err(import_error("merge requires at least one model"));
    };

    let mut transform_state = TransformMergeState::from_model(&merged);

    for model in models {
        merge_one(&mut merged, &model, &mut transform_state)?;
    }

    apply_transform_state(&mut merged, &transform_state)?;

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn transformed_feature(id: &str, translate_x: f64) -> CityModel {
        let bytes = format!(
            r#"{{
                "type":"CityJSONFeature",
                "id":"{id}",
                "transform":{{"scale":[0.001,0.001,0.001],"translate":[{translate_x},0.25,-0.125]}},
                "CityObjects":{{
                    "{id}":{{
                        "type":"Building",
                        "geometry":[{{"type":"MultiSurface","lod":"1","boundaries":[[[0,1,2]]]}}]
                    }}
                }},
                "vertices":[[0,0,0],[1,0,0],[0,1,0]]
            }}"#
        );
        json::from_feature_slice(bytes.as_bytes()).expect("feature should parse")
    }

    #[test]
    fn merge_preserves_world_coordinates_when_transforms_differ() {
        let first_translate = 100.269;
        let second_translate = 200.949;
        let merged = merge([
            transformed_feature("first", first_translate),
            transformed_feature("second", second_translate),
        ])
        .expect("features should merge");

        assert!(merged.transform().is_none());
        let world_coords = merged
            .vertices()
            .as_slice()
            .iter()
            .map(|vertex| vertex.to_array())
            .collect::<Vec<_>>();

        assert!(world_coords.iter().any(|coord| {
            coord
                .iter()
                .zip([first_translate, 0.25, -0.125])
                .all(|(actual, expected)| (*actual - expected).abs() < 1e-9)
        }));
        assert!(world_coords.iter().any(|coord| {
            coord
                .iter()
                .zip([second_translate, 0.25, -0.125])
                .all(|(actual, expected)| (*actual - expected).abs() < 1e-9)
        }));
    }

    #[cfg(feature = "proj")]
    #[test]
    fn transformer_projects_known_point() {
        let transformer = transformer("EPSG:7415", "EPSG:4978").unwrap();
        let result = transformer
            .transform([85285.279, 446606.813, 10.0])
            .unwrap();

        assert!((result[0] - 3_923_215.044).abs() < 10.0);
        assert!((result[1] - 299_940.760).abs() < 10.0);
        assert!((result[2] - 5_003_047.651).abs() < 10.0);
    }

    #[cfg(feature = "proj")]
    #[test]
    fn reproject_applies_transform_and_updates_metadata() {
        let bytes = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "metadata":{"referenceSystem":"EPSG:4979"},
            "transform":{"scale":[2.0,3.0,4.0],"translate":[10.0,20.0,30.0]},
            "CityObjects":{},
            "vertices":[[1,2,3]]
        }"#;
        let model = json::from_slice(bytes).unwrap();
        let reprojected = reproject(model, "EPSG:4979").unwrap();
        let vertex = reprojected.vertices().as_slice()[0];

        assert_eq!(vertex.to_array(), [12.0, 26.0, 42.0]);
        assert!(reprojected.transform().is_none());
        assert_eq!(
            reprojected
                .metadata()
                .unwrap()
                .reference_system()
                .unwrap()
                .to_string(),
            "EPSG:4979"
        );
    }

    #[cfg(feature = "proj")]
    #[test]
    fn reproject_requires_source_crs() {
        let bytes = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "CityObjects":{},
            "vertices":[[1,2,3]]
        }"#;
        let model = json::from_slice(bytes).unwrap();

        assert!(reproject(model, "EPSG:4979").is_err());
    }

    #[cfg(feature = "proj")]
    #[test]
    fn reproject_leaves_template_vertices_unchanged() {
        let bytes = br#"{
            "type":"CityJSON",
            "version":"2.0",
            "metadata":{"referenceSystem":"EPSG:4979"},
            "CityObjects":{},
            "vertices":[[1,2,3]],
            "geometry-templates":{
                "templates":[],
                "vertices-templates":[[100,200,300]]
            }
        }"#;
        let model = json::from_slice(bytes).unwrap();
        let reprojected = reproject(model, "EPSG:4979").unwrap();

        assert_eq!(
            reprojected.template_vertices().as_slice()[0].to_array(),
            [100.0, 200.0, 300.0]
        );
    }
}
