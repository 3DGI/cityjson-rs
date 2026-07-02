//! Geometry validation that operates on final stored geometry.

use crate::backend::default::geometry::{GeometryCore, GeometryType};
use crate::cityjson::core::boundary::Boundary;
use crate::cityjson::core::vertex::VertexRef;
use crate::error::{Error, Result};
use crate::resources::id::ResourceId;
use crate::resources::mapping::SemanticOrMaterialMap;
use crate::resources::mapping::textures::TextureMapCore;
use crate::resources::storage::StringStorage;
use crate::v2_0::boundary::BoundaryType;
use crate::v2_0::vertex::VertexIndex;

pub(crate) trait GeometryValidationContext<VR: VertexRef, RR: ResourceId> {
    fn semantic_exists(&self, id: RR) -> bool;
    fn material_exists(&self, id: RR) -> bool;
    fn texture_exists(&self, id: RR) -> bool;
    fn uv_exists(&self, id: VertexIndex<VR>) -> bool;
    fn regular_vertex_exists(&self, id: VertexIndex<VR>) -> bool;
    fn template_vertex_exists(&self, id: VertexIndex<VR>) -> bool;
    fn template_geometry_exists(&self, id: RR) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoundaryVertexSource {
    Regular,
    Template,
}

/// Validate all invariants for a stored `GeometryCore`.
pub(crate) fn validate_stored_geometry<VR, RR, SS, C>(
    geometry: &GeometryCore<VR, RR, SS>,
    context: &C,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
    SS: StringStorage,
    C: GeometryValidationContext<VR, RR>,
{
    validate_stored_geometry_for_boundary_source(geometry, context, BoundaryVertexSource::Regular)
}

/// Validate all invariants for a stored `GeometryCore` against a specific boundary-vertex store.
pub(crate) fn validate_stored_geometry_for_boundary_source<VR, RR, SS, C>(
    geometry: &GeometryCore<VR, RR, SS>,
    context: &C,
    boundary_vertex_source: BoundaryVertexSource,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
    SS: StringStorage,
    C: GeometryValidationContext<VR, RR>,
{
    if *geometry.type_geometry() == GeometryType::GeometryInstance {
        return validate_instance_isolation(geometry, context);
    }

    if geometry.instance().is_some() {
        return Err(Error::InvalidGeometry(format!(
            "{} must not carry GeometryInstance payload",
            geometry.type_geometry()
        )));
    }

    let boundary = validate_boundary_present(geometry)?;
    validate_boundary_consistent(boundary)?;
    validate_boundary_kind(*geometry.type_geometry(), boundary)?;
    validate_boundary_vertices(boundary, context, boundary_vertex_source)?;

    if let Some(semantics) = geometry.semantics() {
        validate_semantic_kind(*geometry.type_geometry(), semantics, boundary, context)?;
    }

    if let Some(themes) = geometry.materials() {
        for (theme_name, material_map) in themes {
            validate_material_kind(
                *geometry.type_geometry(),
                material_map,
                boundary,
                context,
                theme_name,
            )?;
        }
    }

    if let Some(themes) = geometry.textures() {
        for (theme_name, texture_map) in themes {
            validate_texture_map(theme_name, texture_map, boundary, context)?;
        }
    }

    Ok(())
}

fn validate_boundary_vertices<VR, RR, C>(
    boundary: &Boundary<VR>,
    context: &C,
    boundary_vertex_source: BoundaryVertexSource,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
    C: GeometryValidationContext<VR, RR>,
{
    let pool_name = match boundary_vertex_source {
        BoundaryVertexSource::Regular => "regular",
        BoundaryVertexSource::Template => "template",
    };

    for (index, vertex_ref) in boundary.vertices().iter().enumerate() {
        let exists = match boundary_vertex_source {
            BoundaryVertexSource::Regular => context.regular_vertex_exists(*vertex_ref),
            BoundaryVertexSource::Template => context.template_vertex_exists(*vertex_ref),
        };

        if !exists {
            return Err(Error::InvalidGeometry(format!(
                "boundary vertex {index} references missing {pool_name} vertex {vertex_ref}"
            )));
        }
    }

    Ok(())
}

pub(crate) fn validate_instance_isolation<VR, RR, SS, C>(
    geometry: &GeometryCore<VR, RR, SS>,
    context: &C,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
    SS: StringStorage,
    C: GeometryValidationContext<VR, RR>,
{
    if geometry.boundaries().is_some() {
        return Err(Error::InvalidGeometry(
            "GeometryInstance must not contain a boundary".to_string(),
        ));
    }
    if geometry.semantics().is_some() {
        return Err(Error::InvalidGeometry(
            "GeometryInstance must not contain semantics".to_string(),
        ));
    }
    if geometry.materials().is_some() {
        return Err(Error::InvalidGeometry(
            "GeometryInstance must not contain materials".to_string(),
        ));
    }
    if geometry.textures().is_some() {
        return Err(Error::InvalidGeometry(
            "GeometryInstance must not contain textures".to_string(),
        ));
    }

    let instance = geometry.instance().ok_or_else(|| {
        Error::IncompleteGeometry(
            "GeometryInstance must have instance data (template + reference + transform)"
                .to_string(),
        )
    })?;

    if !context.template_geometry_exists(*instance.template()) {
        return Err(Error::InvalidGeometry(format!(
            "GeometryInstance references missing template {}",
            instance.template()
        )));
    }

    if !context.regular_vertex_exists(*instance.reference_point()) {
        return Err(Error::InvalidGeometry(format!(
            "GeometryInstance references missing reference point {}",
            instance.reference_point()
        )));
    }

    Ok(())
}

pub(crate) fn validate_boundary_present<VR, RR, SS>(
    geometry: &GeometryCore<VR, RR, SS>,
) -> Result<&Boundary<VR>>
where
    VR: VertexRef,
    RR: ResourceId,
    SS: StringStorage,
{
    geometry.boundaries().ok_or_else(|| {
        Error::IncompleteGeometry(format!(
            "{} geometry must have a boundary",
            geometry.type_geometry()
        ))
    })
}

pub(crate) fn validate_boundary_consistent<VR: VertexRef>(boundary: &Boundary<VR>) -> Result<()> {
    if !boundary.is_consistent() {
        return Err(Error::InvalidGeometry(
            "boundary offsets are inconsistent".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_boundary_kind<VR: VertexRef>(
    type_geometry: GeometryType,
    boundary: &Boundary<VR>,
) -> Result<()> {
    validate_boundary_shape(type_geometry, boundary)?;
    let boundary_type = boundary.check_type();
    if !boundary_type_matches(type_geometry, boundary_type) {
        return Err(Error::InvalidGeometryType {
            expected: format!("{type_geometry}"),
            found: format!("{boundary_type}"),
        });
    }
    Ok(())
}

pub(crate) fn validate_semantic_kind<VR, RR, C>(
    type_geometry: GeometryType,
    semantics: &SemanticOrMaterialMap<VR, RR>,
    boundary: &Boundary<VR>,
    context: &C,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
    C: GeometryValidationContext<VR, RR>,
{
    validate_resource_map("semantic", type_geometry, semantics, boundary, |id| {
        context.semantic_exists(id)
    })
}

pub(crate) fn validate_material_kind<VR, RR, C>(
    type_geometry: GeometryType,
    materials: &SemanticOrMaterialMap<VR, RR>,
    boundary: &Boundary<VR>,
    context: &C,
    theme_name: &impl std::fmt::Display,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
    C: GeometryValidationContext<VR, RR>,
{
    validate_resource_map(
        &format!("material theme '{theme_name}'"),
        type_geometry,
        materials,
        boundary,
        |id| context.material_exists(id),
    )
}

fn validate_resource_map<VR, RR>(
    map_name: &str,
    type_geometry: GeometryType,
    map: &SemanticOrMaterialMap<VR, RR>,
    boundary: &Boundary<VR>,
    resource_exists: impl Fn(RR) -> bool,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
{
    let expected_bucket = expected_bucket_name(type_geometry)?;
    let populated_bucket_count = usize::from(!map.points().is_empty())
        + usize::from(!map.linestrings().is_empty())
        + usize::from(!map.surfaces().is_empty());

    if populated_bucket_count != 1 {
        return Err(Error::InvalidGeometry(format!(
            "{map_name} must populate exactly one assignment bucket"
        )));
    }

    let (actual_bucket, assignments) = if !map.points().is_empty() {
        ("points", map.points())
    } else if !map.linestrings().is_empty() {
        ("linestrings", map.linestrings())
    } else if !map.surfaces().is_empty() {
        ("surfaces", map.surfaces())
    } else {
        unreachable!("populated_bucket_count == 1 guarantees one populated bucket");
    };

    let expected_count = match type_geometry {
        GeometryType::MultiPoint => boundary.vertices().len(),
        GeometryType::MultiLineString => boundary.rings().len(),
        GeometryType::MultiSurface
        | GeometryType::CompositeSurface
        | GeometryType::Solid
        | GeometryType::MultiSolid
        | GeometryType::CompositeSolid => boundary.surfaces().len(),
        GeometryType::GeometryInstance => {
            return Err(Error::InvalidGeometry(
                "GeometryInstance must not have semantic or material mappings".to_string(),
            ));
        }
    };

    if actual_bucket != expected_bucket {
        return Err(Error::InvalidGeometry(format!(
            "{map_name} for {type_geometry} must only use the '{expected_bucket}' bucket"
        )));
    }

    if assignments.len() != expected_count {
        return Err(Error::InvalidGeometry(format!(
            "{map_name} {actual_bucket} bucket length {} does not match expected primitive count {expected_count}",
            assignments.len()
        )));
    }

    for (index, assignment) in assignments.iter().enumerate() {
        if let Some(resource_id) = assignment
            && !resource_exists(*resource_id)
        {
            return Err(Error::InvalidGeometry(format!(
                "{map_name} assignment {index} references missing resource {resource_id}"
            )));
        }
    }

    Ok(())
}

fn validate_texture_map<VR, RR, C>(
    theme_name: &impl std::fmt::Display,
    texture_map: &TextureMapCore<VR, RR>,
    boundary: &Boundary<VR>,
    context: &C,
) -> Result<()>
where
    VR: VertexRef,
    RR: ResourceId,
    C: GeometryValidationContext<VR, RR>,
{
    if texture_map.rings().len() != boundary.rings().len() {
        return Err(Error::InvalidGeometry(format!(
            "texture theme '{theme_name}' ring count {} does not match boundary ring count {}",
            texture_map.rings().len(),
            boundary.rings().len()
        )));
    }

    if texture_map.ring_textures().len() != boundary.rings().len() {
        return Err(Error::InvalidGeometry(format!(
            "texture theme '{theme_name}' ring_textures count {} does not match boundary ring count {}",
            texture_map.ring_textures().len(),
            boundary.rings().len()
        )));
    }

    if texture_map.vertices().len() != boundary.vertices().len() {
        return Err(Error::InvalidGeometry(format!(
            "texture theme '{theme_name}' UV vertex count {} does not match boundary vertex count {}",
            texture_map.vertices().len(),
            boundary.vertices().len()
        )));
    }

    for (ring_index, ring_start) in texture_map.rings().iter().enumerate() {
        if *ring_start != boundary.rings()[ring_index] {
            return Err(Error::InvalidGeometry(format!(
                "texture theme '{theme_name}' ring {ring_index} start {ring_start} does not match boundary ring start {}",
                boundary.rings()[ring_index]
            )));
        }

        let slice_start = ring_start.to_usize();
        let slice_end = boundary
            .rings()
            .get(ring_index + 1)
            .map_or(boundary.vertices().len(), VertexIndex::to_usize);
        let uv_slice = &texture_map.vertices()[slice_start..slice_end];

        match texture_map.ring_textures()[ring_index] {
            Some(texture_id) => {
                if !context.texture_exists(texture_id) {
                    return Err(Error::InvalidGeometry(format!(
                        "texture theme '{theme_name}' ring {ring_index} references missing texture {texture_id}"
                    )));
                }

                for (uv_index, uv_ref) in uv_slice.iter().enumerate() {
                    let uv_ref = uv_ref.ok_or_else(|| {
                        Error::InvalidGeometry(format!(
                            "texture theme '{theme_name}' ring {ring_index} has null UV at boundary occurrence {uv_index}"
                        ))
                    })?;

                    if !context.uv_exists(uv_ref) {
                        return Err(Error::InvalidGeometry(format!(
                            "texture theme '{theme_name}' ring {ring_index} references missing UV {uv_ref}"
                        )));
                    }
                }
            }
            None => {
                if uv_slice.iter().any(Option::is_some) {
                    return Err(Error::InvalidGeometry(format!(
                        "texture theme '{theme_name}' ring {ring_index} is untextured but carries UV payload"
                    )));
                }
            }
        }
    }

    Ok(())
}

fn expected_bucket_name(type_geometry: GeometryType) -> Result<&'static str> {
    match type_geometry {
        GeometryType::MultiPoint => Ok("points"),
        GeometryType::MultiLineString => Ok("linestrings"),
        GeometryType::MultiSurface
        | GeometryType::CompositeSurface
        | GeometryType::Solid
        | GeometryType::MultiSolid
        | GeometryType::CompositeSolid => Ok("surfaces"),
        GeometryType::GeometryInstance => Err(Error::InvalidGeometry(
            "GeometryInstance must not carry semantic or material mappings".to_string(),
        )),
    }
}

fn validate_boundary_shape<VR: VertexRef>(
    type_geometry: GeometryType,
    boundary: &Boundary<VR>,
) -> Result<()> {
    match type_geometry {
        GeometryType::MultiPoint => {
            require_non_empty(type_geometry, "vertices", boundary.vertices().is_empty())?;
            require_empty(type_geometry, "rings", !boundary.rings().is_empty())?;
            require_empty(type_geometry, "surfaces", !boundary.surfaces().is_empty())?;
            require_empty(type_geometry, "shells", !boundary.shells().is_empty())?;
            require_empty(type_geometry, "solids", !boundary.solids().is_empty())?;
        }
        GeometryType::MultiLineString => {
            require_non_empty(type_geometry, "vertices", boundary.vertices().is_empty())?;
            require_non_empty(type_geometry, "rings", boundary.rings().is_empty())?;
            require_empty(type_geometry, "surfaces", !boundary.surfaces().is_empty())?;
            require_empty(type_geometry, "shells", !boundary.shells().is_empty())?;
            require_empty(type_geometry, "solids", !boundary.solids().is_empty())?;
        }
        GeometryType::MultiSurface | GeometryType::CompositeSurface => {
            require_non_empty(type_geometry, "vertices", boundary.vertices().is_empty())?;
            require_non_empty(type_geometry, "rings", boundary.rings().is_empty())?;
            require_non_empty(type_geometry, "surfaces", boundary.surfaces().is_empty())?;
            require_empty(type_geometry, "shells", !boundary.shells().is_empty())?;
            require_empty(type_geometry, "solids", !boundary.solids().is_empty())?;
        }
        GeometryType::Solid => {
            require_non_empty(type_geometry, "vertices", boundary.vertices().is_empty())?;
            require_non_empty(type_geometry, "rings", boundary.rings().is_empty())?;
            require_non_empty(type_geometry, "surfaces", boundary.surfaces().is_empty())?;
            require_non_empty(type_geometry, "shells", boundary.shells().is_empty())?;
            require_empty(type_geometry, "solids", !boundary.solids().is_empty())?;
        }
        GeometryType::MultiSolid | GeometryType::CompositeSolid => {
            require_non_empty(type_geometry, "vertices", boundary.vertices().is_empty())?;
            require_non_empty(type_geometry, "rings", boundary.rings().is_empty())?;
            require_non_empty(type_geometry, "surfaces", boundary.surfaces().is_empty())?;
            require_non_empty(type_geometry, "shells", boundary.shells().is_empty())?;
            require_non_empty(type_geometry, "solids", boundary.solids().is_empty())?;
        }
        GeometryType::GeometryInstance => unreachable!("GeometryInstance has no boundary"),
    }

    validate_non_empty_segments(
        "ring",
        "vertex",
        boundary.rings(),
        boundary.vertices().len(),
    )?;
    validate_non_empty_segments(
        "surface",
        "ring",
        boundary.surfaces(),
        boundary.rings().len(),
    )?;
    validate_non_empty_segments(
        "shell",
        "surface",
        boundary.shells(),
        boundary.surfaces().len(),
    )?;
    validate_non_empty_segments("solid", "shell", boundary.solids(), boundary.shells().len())?;

    Ok(())
}

fn require_non_empty(type_geometry: GeometryType, level_name: &str, missing: bool) -> Result<()> {
    if missing {
        return Err(Error::InvalidGeometry(format!(
            "{type_geometry} requires non-empty {level_name}"
        )));
    }
    Ok(())
}

fn require_empty(type_geometry: GeometryType, level_name: &str, present: bool) -> Result<()> {
    if present {
        return Err(Error::InvalidGeometry(format!(
            "{type_geometry} must not carry {level_name}"
        )));
    }
    Ok(())
}

fn validate_non_empty_segments<VR: VertexRef>(
    parent_name: &str,
    child_name: &str,
    offsets: &[VertexIndex<VR>],
    child_len: usize,
) -> Result<()> {
    for (index, start) in offsets.iter().enumerate() {
        let end = offsets
            .get(index + 1)
            .map_or(child_len, VertexIndex::to_usize);
        if start.to_usize() == end {
            return Err(Error::InvalidGeometry(format!(
                "{parent_name} {index} must own at least one {child_name}"
            )));
        }
    }
    Ok(())
}

fn boundary_type_matches(type_geometry: GeometryType, boundary_type: BoundaryType) -> bool {
    matches!(
        (type_geometry, boundary_type),
        (GeometryType::MultiPoint, BoundaryType::MultiPoint)
            | (GeometryType::MultiLineString, BoundaryType::MultiLineString)
            | (
                GeometryType::MultiSurface | GeometryType::CompositeSurface,
                BoundaryType::MultiOrCompositeSurface
            )
            | (GeometryType::Solid, BoundaryType::Solid)
            | (
                GeometryType::MultiSolid | GeometryType::CompositeSolid,
                BoundaryType::MultiOrCompositeSolid
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::default::geometry::{AffineTransform3D, GeometryInstanceData, LoD};
    use crate::backend::default::geometry::{ThemedMaterials, ThemedTextures};
    use crate::cityjson::core::appearance::ThemeName;
    use crate::resources::id::ResourceId32;
    use crate::resources::storage::OwnedStringStorage;
    use crate::v2_0::boundary::nested::{
        BoundaryNestedMultiOrCompositeSurface32, BoundaryNestedMultiPoint32,
    };

    type B = Boundary<u32>;
    type G = GeometryCore<u32, ResourceId32, OwnedStringStorage>;

    struct TestContext {
        semantics: u32,
        materials: u32,
        textures: u32,
        uvs: u32,
        vertices: u32,
        template_vertices: u32,
        templates: u32,
    }

    impl Default for TestContext {
        fn default() -> Self {
            Self {
                semantics: 0,
                materials: 0,
                textures: 0,
                uvs: 0,
                vertices: 8,
                template_vertices: 8,
                templates: 0,
            }
        }
    }

    impl GeometryValidationContext<u32, ResourceId32> for TestContext {
        fn semantic_exists(&self, id: ResourceId32) -> bool {
            id.generation() == 0 && id.index() < self.semantics
        }

        fn material_exists(&self, id: ResourceId32) -> bool {
            id.generation() == 0 && id.index() < self.materials
        }

        fn texture_exists(&self, id: ResourceId32) -> bool {
            id.generation() == 0 && id.index() < self.textures
        }

        fn uv_exists(&self, id: VertexIndex<u32>) -> bool {
            id.value() < self.uvs
        }

        fn regular_vertex_exists(&self, id: VertexIndex<u32>) -> bool {
            id.value() < self.vertices
        }

        fn template_vertex_exists(&self, id: VertexIndex<u32>) -> bool {
            id.value() < self.template_vertices
        }

        fn template_geometry_exists(&self, id: ResourceId32) -> bool {
            id.generation() == 0 && id.index() < self.templates
        }
    }

    fn rid(index: u32) -> ResourceId32 {
        ResourceId32::new(index, 0)
    }

    fn vi(index: u32) -> VertexIndex<u32> {
        VertexIndex::new(index)
    }

    fn theme(value: &str) -> ThemeName<OwnedStringStorage> {
        ThemeName::new(value.to_string())
    }

    fn s1_boundary() -> B {
        let nested: BoundaryNestedMultiOrCompositeSurface32 = vec![
            vec![vec![0u32, 1, 4], vec![0, 2, 1]],
            vec![vec![2, 3, 4, 5]],
        ];
        nested.try_into().unwrap()
    }

    fn multipoint_boundary() -> B {
        let nested: BoundaryNestedMultiPoint32 = vec![0u32, 1, 2];
        nested.into()
    }

    fn surface_map(
        assignments: &[Option<ResourceId32>],
    ) -> SemanticOrMaterialMap<u32, ResourceId32> {
        let mut map = SemanticOrMaterialMap::new();
        for assignment in assignments {
            map.add_surface(*assignment);
        }
        map
    }

    fn dense_texture_map(boundary: &B) -> TextureMapCore<u32, ResourceId32> {
        let mut texture_map = TextureMapCore::default();
        for ring_start in boundary.rings() {
            texture_map.add_ring(*ring_start);
        }

        texture_map.add_ring_texture(Some(rid(0)));
        texture_map.add_ring_texture(None);
        texture_map.add_ring_texture(Some(rid(0)));

        texture_map.add_vertex(Some(vi(0)));
        texture_map.add_vertex(Some(vi(1)));
        texture_map.add_vertex(Some(vi(2)));
        texture_map.add_vertex(None);
        texture_map.add_vertex(None);
        texture_map.add_vertex(None);
        texture_map.add_vertex(Some(vi(3)));
        texture_map.add_vertex(Some(vi(4)));
        texture_map.add_vertex(Some(vi(5)));
        texture_map.add_vertex(Some(vi(6)));

        texture_map
    }

    fn geometry_of_type(
        type_geometry: GeometryType,
        boundary: B,
        semantics: Option<SemanticOrMaterialMap<u32, ResourceId32>>,
        materials: Option<ThemedMaterials<u32, ResourceId32, OwnedStringStorage>>,
        textures: Option<ThemedTextures<u32, ResourceId32, OwnedStringStorage>>,
    ) -> G {
        GeometryCore::new(
            type_geometry,
            Some(LoD::LoD2),
            Some(boundary),
            semantics,
            materials,
            textures,
            None,
        )
    }

    fn regular_geometry(
        boundary: B,
        semantics: Option<SemanticOrMaterialMap<u32, ResourceId32>>,
        materials: Option<ThemedMaterials<u32, ResourceId32, OwnedStringStorage>>,
        textures: Option<ThemedTextures<u32, ResourceId32, OwnedStringStorage>>,
    ) -> G {
        geometry_of_type(
            GeometryType::MultiSurface,
            boundary,
            semantics,
            materials,
            textures,
        )
    }

    fn boundary_with_empty_surface() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: vec![vi(0)],
            surfaces: vec![vi(0), vi(1)],
            shells: Vec::new(),
            solids: Vec::new(),
        }
    }

    fn boundary_with_empty_shell() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: vec![vi(0)],
            surfaces: vec![vi(0)],
            shells: vec![vi(0), vi(1)],
            solids: Vec::new(),
        }
    }

    fn multisurface_with_shells() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: vec![vi(0)],
            surfaces: vec![vi(0)],
            shells: vec![vi(0)],
            solids: Vec::new(),
        }
    }

    fn multisurface_with_solids() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: vec![vi(0)],
            surfaces: vec![vi(0)],
            shells: vec![vi(0)],
            solids: vec![vi(0)],
        }
    }

    fn solid_with_solids() -> B {
        multisurface_with_solids()
    }

    fn multisolid_missing_solids() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: vec![vi(0)],
            surfaces: vec![vi(0)],
            shells: vec![vi(0)],
            solids: Vec::new(),
        }
    }

    fn multisolid_missing_shells() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: vec![vi(0)],
            surfaces: vec![vi(0)],
            shells: Vec::new(),
            solids: vec![vi(0)],
        }
    }

    fn multisolid_missing_surfaces() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: vec![vi(0)],
            surfaces: Vec::new(),
            shells: vec![vi(0)],
            solids: vec![vi(0)],
        }
    }

    fn multisolid_missing_rings() -> B {
        Boundary {
            vertices: vec![vi(0), vi(1), vi(2)],
            rings: Vec::new(),
            surfaces: vec![vi(0)],
            shells: vec![vi(0)],
            solids: vec![vi(0)],
        }
    }

    fn multisolid_missing_vertices() -> B {
        Boundary {
            vertices: Vec::new(),
            rings: vec![vi(0)],
            surfaces: vec![vi(0)],
            shells: vec![vi(0)],
            solids: vec![vi(0)],
        }
    }

    /// Inputs: valid S1-like stored geometry with semantic, material, texture,
    /// and UV maps. Assertions: the shared validator accepts the full payload.
    /// Purpose: positive baseline for the negative mutation families below.
    #[test]
    fn valid_regular_geometry_passes_full_shared_validator() {
        let boundary = s1_boundary();
        let semantics = Some(surface_map(&[Some(rid(0)), Some(rid(1))]));
        let materials = Some(vec![(theme("theme-a"), surface_map(&[Some(rid(0)), None]))]);
        let textures = Some(vec![(theme("theme-a"), dense_texture_map(&boundary))]);
        let geometry = regular_geometry(boundary, semantics, materials, textures);
        let context = TestContext {
            semantics: 2,
            materials: 1,
            textures: 1,
            uvs: 7,
            ..TestContext::default()
        };

        assert!(validate_stored_geometry(&geometry, &context).is_ok());
    }

    /// Inputs: regular and template validation of the same `MultiPoint` boundary
    /// with too few vertices in the selected source pool. Assertions: each mode
    /// reports the corresponding missing vertex source. Purpose: coverage for
    /// boundary vertex source separation.
    #[test]
    fn missing_boundary_vertices_are_rejected_by_source() {
        let geometry = geometry_of_type(
            GeometryType::MultiPoint,
            multipoint_boundary(),
            None,
            None,
            None,
        );

        let err = validate_stored_geometry(
            &geometry,
            &TestContext {
                vertices: 2,
                ..TestContext::default()
            },
        )
        .unwrap_err();
        assert!(format!("{err}").contains("missing regular vertex"));

        let err = validate_stored_geometry_for_boundary_source(
            &geometry,
            &TestContext {
                template_vertices: 2,
                ..TestContext::default()
            },
            BoundaryVertexSource::Template,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("missing template vertex"));
    }

    /// Inputs: regular `MultiSurface` geometry without boundaries. Assertions:
    /// stored validation rejects the incomplete payload. Purpose: direct coverage
    /// for missing required stored geometry parts.
    #[test]
    fn regular_geometry_without_boundary_fails() {
        let geometry: G = GeometryCore::new(
            GeometryType::MultiSurface,
            Some(LoD::LoD2),
            None,
            None,
            None::<ThemedMaterials<u32, ResourceId32, OwnedStringStorage>>,
            None::<ThemedTextures<u32, ResourceId32, OwnedStringStorage>>,
            None,
        );

        assert!(validate_stored_geometry(&geometry, &TestContext::default()).is_err());
    }

    /// Inputs: semantic/material map mutations over an S1-like boundary.
    /// Assertions: wrong bucket, wrong dense length, and missing semantic or
    /// material handles are rejected with the expected error class. Purpose:
    /// table-driven negative coverage for resource map shape and references.
    #[test]
    fn resource_maps_reject_wrong_bucket_length_and_missing_refs() {
        let mut wrong_bucket = SemanticOrMaterialMap::new();
        wrong_bucket.add_linestring(Some(rid(0)));
        wrong_bucket.add_linestring(Some(rid(1)));

        let cases = [
            (
                regular_geometry(s1_boundary(), Some(wrong_bucket), None, None),
                TestContext {
                    semantics: 2,
                    ..TestContext::default()
                },
                "surfaces",
            ),
            (
                regular_geometry(
                    s1_boundary(),
                    Some(surface_map(&[Some(rid(0))])),
                    None,
                    None,
                ),
                TestContext {
                    semantics: 1,
                    ..TestContext::default()
                },
                "primitive count",
            ),
            (
                regular_geometry(
                    s1_boundary(),
                    Some(surface_map(&[Some(rid(9)), Some(rid(0))])),
                    None,
                    None,
                ),
                TestContext {
                    semantics: 1,
                    ..TestContext::default()
                },
                "missing resource",
            ),
            (
                regular_geometry(
                    s1_boundary(),
                    None,
                    Some(vec![(theme("theme-a"), surface_map(&[Some(rid(2)), None]))]),
                    None,
                ),
                TestContext {
                    materials: 1,
                    ..TestContext::default()
                },
                "missing resource",
            ),
        ];

        for (geometry, context, expected) in cases {
            let err = validate_stored_geometry(&geometry, &context).unwrap_err();
            assert!(format!("{err}").contains(expected));
        }
    }

    /// Inputs: S1-like texture maps with one topology or reference mutation at a
    /// time. Assertions: missing texture/UV handles, ring-start mismatch, UVs on
    /// untextured rings, and null UVs on textured rings are rejected. Purpose:
    /// compact negative coverage for dense texture topology rules.
    #[test]
    fn texture_maps_reject_bad_refs_and_topology() {
        fn texture_case(
            mutate: impl FnOnce(&B, &mut TextureMapCore<u32, ResourceId32>),
            expected: &str,
        ) {
            let boundary = s1_boundary();
            let mut textures = dense_texture_map(&boundary);
            mutate(&boundary, &mut textures);
            let geometry = regular_geometry(
                boundary,
                None,
                None,
                Some(vec![(theme("theme-a"), textures)]),
            );
            let err = validate_stored_geometry(
                &geometry,
                &TestContext {
                    textures: 1,
                    uvs: 7,
                    ..TestContext::default()
                },
            )
            .unwrap_err();
            assert!(format!("{err}").contains(expected));
        }

        texture_case(
            |_, textures| textures.ring_textures_mut()[0] = Some(rid(9)),
            "missing texture",
        );
        texture_case(
            |_, textures| textures.vertices_mut()[0] = Some(vi(99)),
            "missing UV",
        );
        texture_case(
            |boundary, textures| textures.rings_mut()[1] = boundary.rings()[0],
            "does not match boundary ring start",
        );
        texture_case(
            |_, textures| textures.vertices_mut()[3] = Some(vi(3)),
            "untextured",
        );
        texture_case(|_, textures| textures.vertices_mut()[0] = None, "null UV");
    }

    /// Inputs: `GeometryInstance` payloads with either a missing template or a
    /// missing regular reference point. Assertions: each missing dependency is
    /// reported precisely. Purpose: split the old ambiguous instance resolution
    /// test into two targeted cases.
    #[test]
    fn instance_template_and_reference_are_rejected_independently() {
        let missing_template: G = GeometryCore::new(
            GeometryType::GeometryInstance,
            None,
            None,
            None,
            None::<ThemedMaterials<u32, ResourceId32, OwnedStringStorage>>,
            None::<ThemedTextures<u32, ResourceId32, OwnedStringStorage>>,
            Some(GeometryInstanceData::new(
                rid(1),
                vi(0),
                AffineTransform3D::identity(),
            )),
        );
        let err = validate_stored_geometry(
            &missing_template,
            &TestContext {
                templates: 1,
                vertices: 1,
                ..TestContext::default()
            },
        )
        .unwrap_err();
        assert!(format!("{err}").contains("missing template"));

        let missing_reference: G = GeometryCore::new(
            GeometryType::GeometryInstance,
            None,
            None,
            None,
            None::<ThemedMaterials<u32, ResourceId32, OwnedStringStorage>>,
            None::<ThemedTextures<u32, ResourceId32, OwnedStringStorage>>,
            Some(GeometryInstanceData::new(
                rid(0),
                vi(2),
                AffineTransform3D::identity(),
            )),
        );
        let err = validate_stored_geometry(
            &missing_reference,
            &TestContext {
                templates: 1,
                vertices: 2,
                ..TestContext::default()
            },
        )
        .unwrap_err();
        assert!(format!("{err}").contains("missing reference point"));
    }

    /// Inputs: boundaries whose populated hierarchy does not match the declared
    /// geometry kind. Assertions: `MultiPoint`-as-`MultiSurface`, `MultiSurface` with
    /// shells/solids, and Solid with solids are rejected. Purpose: negative
    /// coverage for boundary kind mismatch.
    #[test]
    fn boundary_shape_mismatches_are_rejected() {
        let cases = [
            (
                regular_geometry(multipoint_boundary(), None, None, None),
                "",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSurface,
                    multisurface_with_shells(),
                    None,
                    None,
                    None,
                ),
                "must not carry shells",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSurface,
                    multisurface_with_solids(),
                    None,
                    None,
                    None,
                ),
                "must not carry",
            ),
            (
                geometry_of_type(GeometryType::Solid, solid_with_solids(), None, None, None),
                "must not carry solids",
            ),
        ];

        for (geometry, expected) in cases {
            let err = validate_stored_geometry(&geometry, &TestContext::default()).unwrap_err();
            if !expected.is_empty() {
                assert!(format!("{err}").contains(expected));
            }
        }
    }

    /// Inputs: boundaries missing required child levels or containing empty
    /// parent segments for `MultiSurface`, `Solid`, `MultiSolid`, and template-style
    /// validation. Assertions: every required level is rejected. Purpose:
    /// compact coverage for empty required boundary levels.
    #[test]
    fn empty_required_boundary_levels_are_rejected() {
        let cases = [
            (
                geometry_of_type(
                    GeometryType::MultiSurface,
                    boundary_with_empty_surface(),
                    None,
                    None,
                    None,
                ),
                "surface 1",
            ),
            (
                geometry_of_type(
                    GeometryType::Solid,
                    boundary_with_empty_shell(),
                    None,
                    None,
                    None,
                ),
                "shell 1",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSolid,
                    multisolid_missing_solids(),
                    None,
                    None,
                    None,
                ),
                "requires non-empty solids",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSolid,
                    multisolid_missing_shells(),
                    None,
                    None,
                    None,
                ),
                "requires non-empty shells",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSolid,
                    multisolid_missing_surfaces(),
                    None,
                    None,
                    None,
                ),
                "requires non-empty surfaces",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSolid,
                    multisolid_missing_rings(),
                    None,
                    None,
                    None,
                ),
                "requires non-empty rings",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSolid,
                    multisolid_missing_vertices(),
                    None,
                    None,
                    None,
                ),
                "requires non-empty vertices",
            ),
            (
                geometry_of_type(
                    GeometryType::MultiSurface,
                    boundary_with_empty_surface(),
                    None,
                    None,
                    None,
                ),
                "surface 1",
            ),
        ];

        for (geometry, expected) in cases {
            let err = validate_stored_geometry(&geometry, &TestContext::default()).unwrap_err();
            assert!(format!("{err}").contains(expected));
        }
    }
}
