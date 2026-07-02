//! Geometry authoring API.
//!
//! [`GeometryDraft`] is the entry point for building any of the eight `CityJSON` geometry types.
//! It accepts raw coordinates or existing vertex indices, deduplicates vertices, validates
//! the geometry, and inserts it into the model in one step.
//!
//! ## Building a `MultiSurface`
//!
//! ```rust
//! use cityjson_types::CityModelType;
//! use cityjson_types::v2_0::{
//!     GeometryDraft, LoD, OwnedCityModel, RingDraft, SurfaceDraft,
//! };
//!
//! let mut model = OwnedCityModel::new(CityModelType::CityJSON);
//! let lod = Some(LoD::LoD2);
//!
//! let ring = RingDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]);
//! let surface = SurfaceDraft::new(ring, []);
//! let handle = GeometryDraft::multi_surface(lod, [surface])
//!     .insert_into(&mut model)
//!     .unwrap();
//! assert!(model.get_geometry(handle).is_some());
//! ```
//!
//! ## Building a `GeometryInstance`
//!
//! First insert a template, then reference it:
//!
//! ```rust
//! use cityjson_types::CityModelType;
//! use cityjson_types::v2_0::{
//!     AffineTransform3D, GeometryDraft, OwnedCityModel, PointDraft, RealWorldCoordinate,
//! };
//!
//! let mut model = OwnedCityModel::new(CityModelType::CityJSON);
//!
//! let template = GeometryDraft::multi_point(
//!     None,
//!     [PointDraft::new(RealWorldCoordinate::new(0.0, 0.0, 0.0))],
//! )
//! .insert_template_into(&mut model)
//! .unwrap();
//!
//! let instance = GeometryDraft::instance(
//!     template,
//!     RealWorldCoordinate::new(84710.0, 446900.0, 0.0),
//!     AffineTransform3D::identity(),
//! )
//! .insert_into(&mut model)
//! .unwrap();
//!
//! let resolved = model.resolve_geometry(instance).unwrap();
//! assert_eq!(resolved.type_geometry(), &cityjson_types::v2_0::GeometryType::MultiPoint);
//! ```

use crate::backend::default::geometry::GeometryInstanceData;
use crate::cityjson::core::appearance::ThemeName;
use crate::error::{Error, Result};
use crate::resources::handles::{
    GeometryHandle, GeometryTemplateHandle, MaterialHandle, SemanticHandle, TextureHandle,
};
use crate::resources::id::ResourceId32;
use crate::resources::mapping::SemanticOrMaterialMap;
use crate::resources::mapping::textures::TextureMapCore;
use crate::resources::storage::StringStorage;
use crate::v2_0::Boundary;
use crate::v2_0::citymodel::CityModel;
use crate::v2_0::coordinate::{RealWorldCoordinate, UVCoordinate};
use crate::v2_0::geometry::{AffineTransform3D, Geometry, GeometryType, LoD};
use crate::v2_0::vertex::{VertexIndex, VertexRef};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DraftInsertMode {
    Regular,
    Template,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct RealWorldCoordinateKey([u64; 3]);

impl From<RealWorldCoordinate> for RealWorldCoordinateKey {
    fn from(value: RealWorldCoordinate) -> Self {
        Self([
            value.x().to_bits(),
            value.y().to_bits(),
            value.z().to_bits(),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct UvCoordinateKey([u32; 2]);

impl From<&UVCoordinate> for UvCoordinateKey {
    fn from(value: &UVCoordinate) -> Self {
        Self([value.u().to_bits(), value.v().to_bits()])
    }
}

/// A vertex reference for use in geometry drafts.
///
/// Use `Existing` to reference a vertex already in the model's vertex pool,
/// or `New` to provide a raw coordinate that will be deduplicated and inserted on commit.
/// Converts from `VertexIndex`, `RealWorldCoordinate`, and `[f64; 3]`.
#[derive(Clone, Debug, PartialEq)]
pub enum VertexDraft<VR: VertexRef> {
    Existing(VertexIndex<VR>),
    New(RealWorldCoordinate),
}

impl<VR: VertexRef> From<VertexIndex<VR>> for VertexDraft<VR> {
    fn from(value: VertexIndex<VR>) -> Self {
        Self::Existing(value)
    }
}

impl<VR: VertexRef> From<RealWorldCoordinate> for VertexDraft<VR> {
    fn from(value: RealWorldCoordinate) -> Self {
        Self::New(value)
    }
}

impl<VR: VertexRef> From<[f64; 3]> for VertexDraft<VR> {
    fn from(value: [f64; 3]) -> Self {
        Self::New(RealWorldCoordinate::from(value))
    }
}

/// A UV coordinate reference for use in texture drafts.
///
/// Same pattern as [`VertexDraft`]: `Existing` reuses an index from the model's UV pool,
/// `New` inserts a fresh coordinate. Converts from `VertexIndex`, `UVCoordinate`, and `[f32; 2]`.
#[derive(Clone, Debug, PartialEq)]
pub enum UvDraft<VR: VertexRef> {
    Existing(VertexIndex<VR>),
    New(UVCoordinate),
}

impl<VR: VertexRef> From<VertexIndex<VR>> for UvDraft<VR> {
    fn from(value: VertexIndex<VR>) -> Self {
        Self::Existing(value)
    }
}

impl<VR: VertexRef> From<UVCoordinate> for UvDraft<VR> {
    fn from(value: UVCoordinate) -> Self {
        Self::New(value)
    }
}

impl<VR: VertexRef> From<[f32; 2]> for UvDraft<VR> {
    fn from(value: [f32; 2]) -> Self {
        Self::New(UVCoordinate::new(value[0], value[1]))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RingTextureDraft<VR: VertexRef, SS: StringStorage> {
    theme: ThemeName<SS>,
    texture: TextureHandle,
    uvs: Vec<UvDraft<VR>>,
}

/// One point in a `MultiPoint` geometry draft, with an optional semantic handle.
#[derive(Clone, Debug, PartialEq)]
pub struct PointDraft<VR: VertexRef> {
    vertex: VertexDraft<VR>,
    semantic: Option<SemanticHandle>,
}

impl<VR: VertexRef> PointDraft<VR> {
    pub fn new<T>(vertex: T) -> Self
    where
        T: Into<VertexDraft<VR>>,
    {
        Self {
            vertex: vertex.into(),
            semantic: None,
        }
    }

    #[must_use]
    pub fn with_semantic(mut self, semantic: SemanticHandle) -> Self {
        self.semantic = Some(semantic);
        self
    }
}

/// One linestring in a `MultiLineString` geometry draft, with an optional semantic handle.
#[derive(Clone, Debug, PartialEq)]
pub struct LineStringDraft<VR: VertexRef> {
    vertices: Vec<VertexDraft<VR>>,
    semantic: Option<SemanticHandle>,
}

impl<VR: VertexRef> LineStringDraft<VR> {
    pub fn new<I, T>(vertices: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<VertexDraft<VR>>,
    {
        Self {
            vertices: vertices.into_iter().map(Into::into).collect(),
            semantic: None,
        }
    }

    #[must_use]
    pub fn with_semantic(mut self, semantic: SemanticHandle) -> Self {
        self.semantic = Some(semantic);
        self
    }
}

/// One ring (exterior or interior) in a surface draft.
///
/// Accepts an iterator of vertices that convert to [`VertexDraft`], e.g. `[f64; 3]` arrays.
/// Optionally attach per-ring texture UV coordinates with [`RingDraft::with_texture`].
#[derive(Clone, Debug, PartialEq)]
pub struct RingDraft<VR: VertexRef, SS: StringStorage> {
    vertices: Vec<VertexDraft<VR>>,
    textures: Vec<RingTextureDraft<VR, SS>>,
}

impl<VR: VertexRef, SS: StringStorage> RingDraft<VR, SS> {
    pub fn new<I, T>(vertices: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<VertexDraft<VR>>,
    {
        Self {
            vertices: vertices.into_iter().map(Into::into).collect(),
            textures: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_texture<I, T>(
        mut self,
        theme: impl Into<ThemeName<SS>>,
        texture: TextureHandle,
        uvs: I,
    ) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<UvDraft<VR>>,
    {
        self.textures.push(RingTextureDraft {
            theme: theme.into(),
            texture,
            uvs: uvs.into_iter().map(Into::into).collect(),
        });
        self
    }
}

/// One surface in a `MultiSurface`, `CompositeSurface`, or `Solid` shell draft.
///
/// Consists of an outer ring and zero or more inner rings (holes). Attach a semantic
/// with [`SurfaceDraft::with_semantic`] and a material theme with
/// [`SurfaceDraft::with_material`].
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceDraft<VR: VertexRef, SS: StringStorage> {
    outer: RingDraft<VR, SS>,
    inners: Vec<RingDraft<VR, SS>>,
    semantic: Option<SemanticHandle>,
    materials: Vec<(ThemeName<SS>, MaterialHandle)>,
}

impl<VR: VertexRef, SS: StringStorage> SurfaceDraft<VR, SS> {
    pub fn new<I>(outer: RingDraft<VR, SS>, inners: I) -> Self
    where
        I: IntoIterator<Item = RingDraft<VR, SS>>,
    {
        Self {
            outer,
            inners: inners.into_iter().collect(),
            semantic: None,
            materials: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_semantic(mut self, semantic: SemanticHandle) -> Self {
        self.semantic = Some(semantic);
        self
    }

    #[must_use]
    pub fn with_material(
        mut self,
        theme: impl Into<ThemeName<SS>>,
        material: MaterialHandle,
    ) -> Self {
        self.materials.push((theme.into(), material));
        self
    }
}

/// A shell (exterior or interior) in a `Solid` draft.
///
/// A shell is a collection of surfaces. The first shell of a `Solid` is the exterior;
/// additional shells are interior voids.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellDraft<VR: VertexRef, SS: StringStorage> {
    surfaces: Vec<SurfaceDraft<VR, SS>>,
}

impl<VR: VertexRef, SS: StringStorage> ShellDraft<VR, SS> {
    pub fn new<I>(surfaces: I) -> Self
    where
        I: IntoIterator<Item = SurfaceDraft<VR, SS>>,
    {
        Self {
            surfaces: surfaces.into_iter().collect(),
        }
    }
}

/// A solid in a `Solid`, `MultiSolid`, or `CompositeSolid` draft.
///
/// Holds one outer shell and zero or more inner shells.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidDraft<VR: VertexRef, SS: StringStorage> {
    outer: ShellDraft<VR, SS>,
    inners: Vec<ShellDraft<VR, SS>>,
}

impl<VR: VertexRef, SS: StringStorage> SolidDraft<VR, SS> {
    pub fn new<I>(outer: ShellDraft<VR, SS>, inners: I) -> Self
    where
        I: IntoIterator<Item = ShellDraft<VR, SS>>,
    {
        Self {
            outer,
            inners: inners.into_iter().collect(),
        }
    }
}

/// A geometry under construction, ready to be validated and inserted into a [`CityModel`].
///
/// One variant per `CityJSON` geometry type. Build using the constructor methods
/// (`multi_point`, `solid`, `instance`, etc.), then call [`insert_into`] or
/// [`insert_template_into`] to commit.
///
/// On insert, raw coordinates are deduplicated against the model's vertex pool and the
/// geometry is validated. If validation fails, an error is returned and the model is
/// unchanged.
///
/// [`insert_into`]: GeometryDraft::insert_into
/// [`insert_template_into`]: GeometryDraft::insert_template_into
#[derive(Clone, Debug, PartialEq)]
pub enum GeometryDraft<VR: VertexRef, SS: StringStorage> {
    MultiPoint {
        lod: Option<LoD>,
        points: Vec<PointDraft<VR>>,
    },
    MultiLineString {
        lod: Option<LoD>,
        linestrings: Vec<LineStringDraft<VR>>,
    },
    MultiSurface {
        lod: Option<LoD>,
        surfaces: Vec<SurfaceDraft<VR, SS>>,
    },
    CompositeSurface {
        lod: Option<LoD>,
        surfaces: Vec<SurfaceDraft<VR, SS>>,
    },
    Solid {
        lod: Option<LoD>,
        solid: SolidDraft<VR, SS>,
    },
    MultiSolid {
        lod: Option<LoD>,
        solids: Vec<SolidDraft<VR, SS>>,
    },
    CompositeSolid {
        lod: Option<LoD>,
        solids: Vec<SolidDraft<VR, SS>>,
    },
    GeometryInstance {
        template: GeometryTemplateHandle,
        reference_point: VertexDraft<VR>,
        transformation: AffineTransform3D,
    },
}

impl<VR: VertexRef, SS: StringStorage> GeometryDraft<VR, SS> {
    pub fn multi_point<I>(lod: Option<LoD>, points: I) -> Self
    where
        I: IntoIterator<Item = PointDraft<VR>>,
    {
        Self::MultiPoint {
            lod,
            points: points.into_iter().collect(),
        }
    }

    pub fn multi_line_string<I>(lod: Option<LoD>, linestrings: I) -> Self
    where
        I: IntoIterator<Item = LineStringDraft<VR>>,
    {
        Self::MultiLineString {
            lod,
            linestrings: linestrings.into_iter().collect(),
        }
    }

    pub fn multi_surface<I>(lod: Option<LoD>, surfaces: I) -> Self
    where
        I: IntoIterator<Item = SurfaceDraft<VR, SS>>,
    {
        Self::MultiSurface {
            lod,
            surfaces: surfaces.into_iter().collect(),
        }
    }

    pub fn composite_surface<I>(lod: Option<LoD>, surfaces: I) -> Self
    where
        I: IntoIterator<Item = SurfaceDraft<VR, SS>>,
    {
        Self::CompositeSurface {
            lod,
            surfaces: surfaces.into_iter().collect(),
        }
    }

    pub fn solid<I>(lod: Option<LoD>, outer: ShellDraft<VR, SS>, inners: I) -> Self
    where
        I: IntoIterator<Item = ShellDraft<VR, SS>>,
    {
        Self::Solid {
            lod,
            solid: SolidDraft::new(outer, inners),
        }
    }

    pub fn multi_solid<I>(lod: Option<LoD>, solids: I) -> Self
    where
        I: IntoIterator<Item = SolidDraft<VR, SS>>,
    {
        Self::MultiSolid {
            lod,
            solids: solids.into_iter().collect(),
        }
    }

    pub fn composite_solid<I>(lod: Option<LoD>, solids: I) -> Self
    where
        I: IntoIterator<Item = SolidDraft<VR, SS>>,
    {
        Self::CompositeSolid {
            lod,
            solids: solids.into_iter().collect(),
        }
    }

    pub fn instance<T>(
        template: GeometryTemplateHandle,
        reference_point: T,
        transformation: AffineTransform3D,
    ) -> Self
    where
        T: Into<VertexDraft<VR>>,
    {
        Self::GeometryInstance {
            template,
            reference_point: reference_point.into(),
            transformation,
        }
    }

    #[must_use]
    pub fn type_geometry(&self) -> GeometryType {
        match self {
            Self::MultiPoint { .. } => GeometryType::MultiPoint,
            Self::MultiLineString { .. } => GeometryType::MultiLineString,
            Self::MultiSurface { .. } => GeometryType::MultiSurface,
            Self::CompositeSurface { .. } => GeometryType::CompositeSurface,
            Self::Solid { .. } => GeometryType::Solid,
            Self::MultiSolid { .. } => GeometryType::MultiSolid,
            Self::CompositeSolid { .. } => GeometryType::CompositeSolid,
            Self::GeometryInstance { .. } => GeometryType::GeometryInstance,
        }
    }

    #[must_use]
    pub fn lod(&self) -> Option<LoD> {
        match self {
            Self::MultiPoint { lod, .. }
            | Self::MultiLineString { lod, .. }
            | Self::MultiSurface { lod, .. }
            | Self::CompositeSurface { lod, .. }
            | Self::Solid { lod, .. }
            | Self::MultiSolid { lod, .. }
            | Self::CompositeSolid { lod, .. } => *lod,
            Self::GeometryInstance { .. } => None,
        }
    }

    /// Inserts this draft into the regular geometry pool after validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the draft is invalid, references missing resources,
    /// or cannot reserve required capacity.
    pub fn insert_into(self, model: &mut CityModel<VR, SS>) -> Result<GeometryHandle> {
        let geometry = self.build_stored(model, DraftInsertMode::Regular)?;
        model.add_geometry_unchecked(geometry)
    }

    /// Inserts this draft into the template geometry pool after validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the draft is invalid, references missing resources,
    /// or cannot reserve required capacity.
    pub fn insert_template_into(
        self,
        model: &mut CityModel<VR, SS>,
    ) -> Result<GeometryTemplateHandle> {
        let geometry = self.build_stored(model, DraftInsertMode::Template)?;
        model.add_geometry_template_unchecked(geometry)
    }

    fn build_stored(
        self,
        model: &mut CityModel<VR, SS>,
        mode: DraftInsertMode,
    ) -> Result<Geometry<VR, SS>> {
        self.validate_draft()?;
        let analysis = self.analyze(model, mode)?;
        analysis.preflight(model, mode)?;

        let mut resolver = DraftResolver::new(model, mode);
        let geometry = match self {
            Self::GeometryInstance {
                template,
                reference_point,
                transformation,
            } => {
                let reference_point = resolver.resolve_vertex(&reference_point)?;
                Geometry::from_raw_parts(
                    GeometryType::GeometryInstance,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(GeometryInstanceData::new(
                        template.to_raw(),
                        reference_point,
                        transformation,
                    )),
                )
            }
            draft => draft.build_regular_geometry(&mut resolver)?,
        };

        Ok(geometry)
    }

    fn build_regular_geometry(
        self,
        resolver: &mut DraftResolver<'_, VR, SS>,
    ) -> Result<Geometry<VR, SS>> {
        let type_geometry = self.type_geometry();
        let lod = self.lod();
        let mut boundary = Boundary::new();
        let mut semantics = None;
        let mut materials = Vec::new();
        let mut textures = Vec::new();

        match self {
            Self::MultiPoint { points, .. } => {
                build_multi_point_geometry(resolver, &mut boundary, &mut semantics, &points)?;
            }
            Self::MultiLineString { linestrings, .. } => build_multi_linestring_geometry(
                resolver,
                &mut boundary,
                &mut semantics,
                &linestrings,
            )?,
            Self::MultiSurface { surfaces, .. } | Self::CompositeSurface { surfaces, .. } => {
                build_surface_geometry(
                    resolver,
                    &mut boundary,
                    &mut semantics,
                    &mut materials,
                    &mut textures,
                    &surfaces,
                )?;
            }
            Self::Solid { solid, .. } => {
                build_solid_geometry(
                    resolver,
                    &mut boundary,
                    &mut semantics,
                    &mut materials,
                    &mut textures,
                    &solid,
                )?;
            }
            Self::MultiSolid { solids, .. } | Self::CompositeSolid { solids, .. } => {
                build_multi_solid_geometry(
                    resolver,
                    &mut boundary,
                    &mut semantics,
                    &mut materials,
                    &mut textures,
                    &solids,
                )?;
            }
            Self::GeometryInstance { .. } => unreachable!("handled separately"),
        }

        Ok(Geometry::from_raw_parts(
            type_geometry,
            lod,
            Some(boundary),
            semantics,
            if materials.is_empty() {
                None
            } else {
                Some(materials)
            },
            if textures.is_empty() {
                None
            } else {
                Some(textures)
            },
            None,
        ))
    }

    fn validate_draft(&self) -> Result<()> {
        match self {
            Self::MultiPoint { points, .. } => {
                if points.is_empty() {
                    return Err(Error::InvalidGeometry(
                        "MultiPoint draft requires at least one point".to_string(),
                    ));
                }
            }
            Self::MultiLineString { linestrings, .. } => {
                if linestrings.is_empty() {
                    return Err(Error::InvalidGeometry(
                        "MultiLineString draft requires at least one linestring".to_string(),
                    ));
                }
                for linestring in linestrings {
                    validate_non_empty_vertices("linestring", &linestring.vertices)?;
                }
            }
            Self::MultiSurface { surfaces, .. } | Self::CompositeSurface { surfaces, .. } => {
                validate_surface_list("surface geometry", surfaces)?;
            }
            Self::Solid { solid, .. } => {
                validate_shell("solid outer shell", &solid.outer)?;
                for shell in &solid.inners {
                    validate_shell("solid inner shell", shell)?;
                }
            }
            Self::MultiSolid { solids, .. } | Self::CompositeSolid { solids, .. } => {
                if solids.is_empty() {
                    return Err(Error::InvalidGeometry(
                        "multi-solid draft requires at least one solid".to_string(),
                    ));
                }
                for solid in solids {
                    validate_shell("solid outer shell", &solid.outer)?;
                    for shell in &solid.inners {
                        validate_shell("solid inner shell", shell)?;
                    }
                }
            }
            Self::GeometryInstance { .. } => {}
        }

        Ok(())
    }

    fn analyze(&self, model: &CityModel<VR, SS>, mode: DraftInsertMode) -> Result<DraftAnalysis> {
        let mut analysis = DraftAnalysis::default();

        match self {
            Self::MultiPoint { points, .. } => {
                for point in points {
                    analysis.record_vertex(model, mode, &point.vertex)?;
                    DraftAnalysis::require_semantic(model, point.semantic)?;
                    analysis.vertices += 1;
                }
            }
            Self::MultiLineString { linestrings, .. } => {
                for linestring in linestrings {
                    analysis.rings += 1;
                    DraftAnalysis::require_semantic(model, linestring.semantic)?;
                    for vertex in &linestring.vertices {
                        analysis.record_vertex(model, mode, vertex)?;
                        analysis.vertices += 1;
                    }
                }
            }
            Self::MultiSurface { surfaces, .. } | Self::CompositeSurface { surfaces, .. } => {
                for surface in surfaces {
                    analyze_surface(model, mode, surface, &mut analysis)?;
                }
            }
            Self::Solid { solid, .. } => {
                for shell in std::iter::once(&solid.outer).chain(solid.inners.iter()) {
                    analysis.shells += 1;
                    for surface in &shell.surfaces {
                        analyze_surface(model, mode, surface, &mut analysis)?;
                    }
                }
            }
            Self::MultiSolid { solids, .. } | Self::CompositeSolid { solids, .. } => {
                for solid in solids {
                    analysis.solids += 1;
                    for shell in std::iter::once(&solid.outer).chain(solid.inners.iter()) {
                        analysis.shells += 1;
                        for surface in &shell.surfaces {
                            analyze_surface(model, mode, surface, &mut analysis)?;
                        }
                    }
                }
            }
            Self::GeometryInstance {
                template,
                reference_point,
                ..
            } => {
                if mode == DraftInsertMode::Template {
                    return Err(Error::InvalidGeometry(
                        "GeometryInstance cannot be inserted into the template geometry pool"
                            .to_string(),
                    ));
                }
                if model.get_geometry_template(*template).is_none() {
                    return Err(Error::InvalidGeometry(format!(
                        "GeometryInstance references missing template {}",
                        template.to_raw()
                    )));
                }
                analysis.record_vertex(model, DraftInsertMode::Regular, reference_point)?;
            }
        }

        VertexIndex::<VR>::try_from(analysis.vertices)?;
        VertexIndex::<VR>::try_from(analysis.rings)?;
        VertexIndex::<VR>::try_from(analysis.surfaces)?;
        VertexIndex::<VR>::try_from(analysis.shells)?;
        VertexIndex::<VR>::try_from(analysis.solids)?;

        Ok(analysis)
    }
}

fn validate_surface_list<VR: VertexRef, SS: StringStorage>(
    label: &str,
    surfaces: &[SurfaceDraft<VR, SS>],
) -> Result<()> {
    if surfaces.is_empty() {
        return Err(Error::InvalidGeometry(format!(
            "{label} requires at least one surface"
        )));
    }
    for surface in surfaces {
        validate_surface(surface)?;
    }
    Ok(())
}

fn validate_surface<VR: VertexRef, SS: StringStorage>(
    surface: &SurfaceDraft<VR, SS>,
) -> Result<()> {
    validate_ring(&surface.outer)?;
    for ring in &surface.inners {
        validate_ring(ring)?;
    }
    validate_unique_surface_themes(surface)?;
    Ok(())
}

fn validate_shell<VR: VertexRef, SS: StringStorage>(
    label: &str,
    shell: &ShellDraft<VR, SS>,
) -> Result<()> {
    if shell.surfaces.is_empty() {
        return Err(Error::InvalidGeometry(format!(
            "{label} requires at least one surface"
        )));
    }
    for surface in &shell.surfaces {
        validate_surface(surface)?;
    }
    Ok(())
}

fn validate_ring<VR: VertexRef, SS: StringStorage>(ring: &RingDraft<VR, SS>) -> Result<()> {
    validate_non_empty_vertices("ring", &ring.vertices)?;
    validate_unique_ring_themes(ring)?;
    for texture in &ring.textures {
        if texture.uvs.len() != ring.vertices.len() {
            return Err(Error::InvalidGeometry(format!(
                "ring texture theme '{}' has {} UVs for {} vertices",
                texture.theme,
                texture.uvs.len(),
                ring.vertices.len()
            )));
        }
    }
    Ok(())
}

fn validate_non_empty_vertices<VR: VertexRef>(
    label: &str,
    vertices: &[VertexDraft<VR>],
) -> Result<()> {
    if vertices.is_empty() {
        return Err(Error::InvalidGeometry(format!(
            "{label} requires at least one vertex"
        )));
    }
    Ok(())
}

fn validate_unique_ring_themes<VR: VertexRef, SS: StringStorage>(
    ring: &RingDraft<VR, SS>,
) -> Result<()> {
    let mut seen = HashSet::<&str>::new();
    for texture in &ring.textures {
        let theme = texture.theme.as_ref();
        if !seen.insert(theme) {
            return Err(Error::InvalidGeometry(format!(
                "ring texture theme '{theme}' is assigned more than once"
            )));
        }
    }
    Ok(())
}

fn validate_unique_surface_themes<VR: VertexRef, SS: StringStorage>(
    surface: &SurfaceDraft<VR, SS>,
) -> Result<()> {
    let mut seen = HashSet::<&str>::new();
    for (theme, _) in &surface.materials {
        let theme_ref = theme.as_ref();
        if !seen.insert(theme_ref) {
            return Err(Error::InvalidGeometry(format!(
                "surface material theme '{theme_ref}' is assigned more than once"
            )));
        }
    }
    Ok(())
}

#[derive(Default)]
struct DraftAnalysis {
    vertices: usize,
    rings: usize,
    surfaces: usize,
    shells: usize,
    solids: usize,
    new_vertices: usize,
    new_uvs: usize,
    seen_new_vertices: HashSet<RealWorldCoordinateKey>,
    seen_new_uvs: HashSet<UvCoordinateKey>,
}

impl DraftAnalysis {
    fn preflight<VR: VertexRef, SS: StringStorage>(
        &self,
        model: &mut CityModel<VR, SS>,
        mode: DraftInsertMode,
    ) -> Result<()> {
        model.reserve_draft_insert(mode, self.new_vertices, self.new_uvs)
    }

    fn record_vertex<VR: VertexRef, SS: StringStorage>(
        &mut self,
        model: &CityModel<VR, SS>,
        mode: DraftInsertMode,
        vertex: &VertexDraft<VR>,
    ) -> Result<()> {
        match vertex {
            VertexDraft::Existing(index) => {
                let exists = match mode {
                    DraftInsertMode::Regular => model.get_vertex(*index).is_some(),
                    DraftInsertMode::Template => model.get_template_vertex(*index).is_some(),
                };
                if !exists {
                    let pool = match mode {
                        DraftInsertMode::Regular => "regular",
                        DraftInsertMode::Template => "template",
                    };
                    return Err(Error::InvalidGeometry(format!(
                        "draft references missing {pool} vertex {index}"
                    )));
                }
            }
            VertexDraft::New(coord) => {
                if self.seen_new_vertices.insert((*coord).into()) {
                    self.new_vertices += 1;
                }
            }
        }
        Ok(())
    }

    fn record_uv<VR: VertexRef, SS: StringStorage>(
        &mut self,
        model: &CityModel<VR, SS>,
        uv: &UvDraft<VR>,
    ) -> Result<()> {
        match uv {
            UvDraft::Existing(index) => {
                if model.get_uv_coordinate(*index).is_none() {
                    return Err(Error::InvalidGeometry(format!(
                        "draft references missing UV {index}"
                    )));
                }
            }
            UvDraft::New(coord) => {
                if self.seen_new_uvs.insert(coord.into()) {
                    self.new_uvs += 1;
                }
            }
        }
        Ok(())
    }

    fn require_semantic<VR: VertexRef, SS: StringStorage>(
        model: &CityModel<VR, SS>,
        semantic: Option<SemanticHandle>,
    ) -> Result<()> {
        if let Some(handle) = semantic
            && model.get_semantic(handle).is_none()
        {
            return Err(Error::InvalidGeometry(format!(
                "draft references missing semantic {}",
                handle.to_raw()
            )));
        }
        Ok(())
    }

    fn require_material<VR: VertexRef, SS: StringStorage>(
        model: &CityModel<VR, SS>,
        material: MaterialHandle,
    ) -> Result<()> {
        if model.get_material(material).is_none() {
            return Err(Error::InvalidGeometry(format!(
                "draft references missing material {}",
                material.to_raw()
            )));
        }
        Ok(())
    }

    fn require_texture<VR: VertexRef, SS: StringStorage>(
        model: &CityModel<VR, SS>,
        texture: TextureHandle,
    ) -> Result<()> {
        if model.get_texture(texture).is_none() {
            return Err(Error::InvalidGeometry(format!(
                "draft references missing texture {}",
                texture.to_raw()
            )));
        }
        Ok(())
    }
}

fn analyze_surface<VR: VertexRef, SS: StringStorage>(
    model: &CityModel<VR, SS>,
    mode: DraftInsertMode,
    surface: &SurfaceDraft<VR, SS>,
    analysis: &mut DraftAnalysis,
) -> Result<()> {
    analysis.surfaces += 1;
    DraftAnalysis::require_semantic(model, surface.semantic)?;
    for (_, material) in &surface.materials {
        DraftAnalysis::require_material(model, *material)?;
    }
    analyze_ring(model, mode, &surface.outer, analysis)?;
    for ring in &surface.inners {
        analyze_ring(model, mode, ring, analysis)?;
    }
    Ok(())
}

fn analyze_ring<VR: VertexRef, SS: StringStorage>(
    model: &CityModel<VR, SS>,
    mode: DraftInsertMode,
    ring: &RingDraft<VR, SS>,
    analysis: &mut DraftAnalysis,
) -> Result<()> {
    analysis.rings += 1;
    for vertex in &ring.vertices {
        analysis.record_vertex(model, mode, vertex)?;
        analysis.vertices += 1;
    }
    for texture in &ring.textures {
        DraftAnalysis::require_texture(model, texture.texture)?;
        for uv in &texture.uvs {
            analysis.record_uv(model, uv)?;
        }
    }
    Ok(())
}

fn build_multi_point_geometry<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    semantics: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    points: &[PointDraft<VR>],
) -> Result<()> {
    for (point_index, point) in points.iter().enumerate() {
        let vertex = resolver.resolve_vertex(&point.vertex)?;
        boundary.vertices.push(vertex);
        push_dense_assignment(
            semantics,
            point_index,
            point.semantic.map(SemanticHandle::to_raw),
            DenseBucket::Points,
        );
    }
    Ok(())
}

fn build_multi_linestring_geometry<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    semantics: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    linestrings: &[LineStringDraft<VR>],
) -> Result<()> {
    for (linestring_index, linestring) in linestrings.iter().enumerate() {
        boundary
            .rings
            .push(VertexIndex::try_from(boundary.vertices.len())?);
        for vertex in &linestring.vertices {
            boundary.vertices.push(resolver.resolve_vertex(vertex)?);
        }
        push_dense_assignment(
            semantics,
            linestring_index,
            linestring.semantic.map(SemanticHandle::to_raw),
            DenseBucket::Linestrings,
        );
    }
    Ok(())
}

fn build_surface_geometry<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    semantics: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    materials: &mut Vec<(ThemeName<SS>, SemanticOrMaterialMap<VR, ResourceId32>)>,
    textures: &mut Vec<(ThemeName<SS>, TextureMapCore<VR, ResourceId32>)>,
    surfaces: &[SurfaceDraft<VR, SS>],
) -> Result<()> {
    let mut surface_index = 0;
    for surface in surfaces {
        flatten_surface(
            resolver,
            boundary,
            semantics,
            materials,
            textures,
            &mut surface_index,
            surface,
        )?;
    }
    Ok(())
}

fn build_solid_geometry<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    semantics: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    materials: &mut Vec<(ThemeName<SS>, SemanticOrMaterialMap<VR, ResourceId32>)>,
    textures: &mut Vec<(ThemeName<SS>, TextureMapCore<VR, ResourceId32>)>,
    solid: &SolidDraft<VR, SS>,
) -> Result<()> {
    let mut surface_index = 0;
    flatten_shell(
        resolver,
        boundary,
        semantics,
        materials,
        textures,
        &mut surface_index,
        &solid.outer,
    )?;
    for shell in &solid.inners {
        flatten_shell(
            resolver,
            boundary,
            semantics,
            materials,
            textures,
            &mut surface_index,
            shell,
        )?;
    }
    Ok(())
}

fn build_multi_solid_geometry<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    semantics: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    materials: &mut Vec<(ThemeName<SS>, SemanticOrMaterialMap<VR, ResourceId32>)>,
    textures: &mut Vec<(ThemeName<SS>, TextureMapCore<VR, ResourceId32>)>,
    solids: &[SolidDraft<VR, SS>],
) -> Result<()> {
    let mut surface_index = 0;
    for solid in solids {
        boundary
            .solids
            .push(VertexIndex::try_from(boundary.shells.len())?);
        flatten_shell(
            resolver,
            boundary,
            semantics,
            materials,
            textures,
            &mut surface_index,
            &solid.outer,
        )?;
        for shell in &solid.inners {
            flatten_shell(
                resolver,
                boundary,
                semantics,
                materials,
                textures,
                &mut surface_index,
                shell,
            )?;
        }
    }
    Ok(())
}

struct DraftResolver<'a, VR: VertexRef, SS: StringStorage> {
    model: &'a mut CityModel<VR, SS>,
    mode: DraftInsertMode,
    new_vertices: HashMap<RealWorldCoordinateKey, VertexIndex<VR>>,
    new_uvs: HashMap<UvCoordinateKey, VertexIndex<VR>>,
}

impl<'a, VR: VertexRef, SS: StringStorage> DraftResolver<'a, VR, SS> {
    fn new(model: &'a mut CityModel<VR, SS>, mode: DraftInsertMode) -> Self {
        Self {
            model,
            mode,
            new_vertices: HashMap::new(),
            new_uvs: HashMap::new(),
        }
    }

    fn resolve_vertex(&mut self, vertex: &VertexDraft<VR>) -> Result<VertexIndex<VR>> {
        match vertex {
            VertexDraft::Existing(index) => Ok(*index),
            VertexDraft::New(coord) => {
                let key: RealWorldCoordinateKey = (*coord).into();
                if let Some(index) = self.new_vertices.get(&key) {
                    return Ok(*index);
                }
                let index = match self.mode {
                    DraftInsertMode::Regular => self.model.add_vertex(*coord)?,
                    DraftInsertMode::Template => self.model.add_template_vertex(*coord)?,
                };
                self.new_vertices.insert(key, index);
                Ok(index)
            }
        }
    }

    fn resolve_uv(&mut self, uv: &UvDraft<VR>) -> Result<VertexIndex<VR>> {
        match uv {
            UvDraft::Existing(index) => Ok(*index),
            UvDraft::New(coord) => {
                let key: UvCoordinateKey = coord.into();
                if let Some(index) = self.new_uvs.get(&key) {
                    return Ok(*index);
                }
                let index = self.model.add_uv_coordinate(coord.clone())?;
                self.new_uvs.insert(key, index);
                Ok(index)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum DenseBucket {
    Points,
    Linestrings,
    Surfaces,
}

fn push_dense_assignment<VR: VertexRef>(
    map: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    current_index: usize,
    assignment: Option<ResourceId32>,
    bucket: DenseBucket,
) {
    if map.is_none() && assignment.is_none() {
        return;
    }

    let map = map.get_or_insert_with(|| {
        let mut map = SemanticOrMaterialMap::new();
        for _ in 0..current_index {
            match bucket {
                DenseBucket::Points => map.add_point(None),
                DenseBucket::Linestrings => map.add_linestring(None),
                DenseBucket::Surfaces => map.add_surface(None),
            }
        }
        map
    });

    match bucket {
        DenseBucket::Points => map.add_point(assignment),
        DenseBucket::Linestrings => map.add_linestring(assignment),
        DenseBucket::Surfaces => map.add_surface(assignment),
    }
}

fn flatten_shell<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    semantics: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    materials: &mut Vec<(ThemeName<SS>, SemanticOrMaterialMap<VR, ResourceId32>)>,
    textures: &mut Vec<(ThemeName<SS>, TextureMapCore<VR, ResourceId32>)>,
    surface_index: &mut usize,
    shell: &ShellDraft<VR, SS>,
) -> Result<()> {
    boundary
        .shells
        .push(VertexIndex::try_from(boundary.surfaces.len())?);
    for surface in &shell.surfaces {
        flatten_surface(
            resolver,
            boundary,
            semantics,
            materials,
            textures,
            surface_index,
            surface,
        )?;
    }
    Ok(())
}

fn flatten_surface<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    semantics: &mut Option<SemanticOrMaterialMap<VR, ResourceId32>>,
    materials: &mut Vec<(ThemeName<SS>, SemanticOrMaterialMap<VR, ResourceId32>)>,
    textures: &mut Vec<(ThemeName<SS>, TextureMapCore<VR, ResourceId32>)>,
    surface_index: &mut usize,
    surface: &SurfaceDraft<VR, SS>,
) -> Result<()> {
    let current_surface = *surface_index;
    boundary
        .surfaces
        .push(VertexIndex::try_from(boundary.rings.len())?);

    push_dense_assignment(
        semantics,
        current_surface,
        surface.semantic.map(SemanticHandle::to_raw),
        DenseBucket::Surfaces,
    );
    push_surface_materials::<VR, SS>(materials, current_surface, &surface.materials);

    flatten_ring(resolver, boundary, textures, &surface.outer)?;
    for ring in &surface.inners {
        flatten_ring(resolver, boundary, textures, ring)?;
    }

    *surface_index += 1;
    Ok(())
}

fn push_surface_materials<VR: VertexRef, SS: StringStorage>(
    materials: &mut Vec<(ThemeName<SS>, SemanticOrMaterialMap<VR, ResourceId32>)>,
    current_surface: usize,
    assignments: &[(ThemeName<SS>, MaterialHandle)],
) {
    for (_, map) in materials.iter_mut() {
        map.add_surface(None);
    }

    for (theme, material) in assignments {
        if let Some((_, map)) = materials.iter_mut().find(|(name, _)| name == theme) {
            if let Some(slot) = map.surfaces.last_mut() {
                *slot = Some(material.to_raw());
            }
            continue;
        }

        let mut map = SemanticOrMaterialMap::new();
        for _ in 0..current_surface {
            map.add_surface(None);
        }
        map.add_surface(Some(material.to_raw()));
        materials.push((theme.clone(), map));
    }
}

fn flatten_ring<VR: VertexRef, SS: StringStorage>(
    resolver: &mut DraftResolver<'_, VR, SS>,
    boundary: &mut Boundary<VR>,
    textures: &mut Vec<(ThemeName<SS>, TextureMapCore<VR, ResourceId32>)>,
    ring: &RingDraft<VR, SS>,
) -> Result<()> {
    let ring_index = boundary.rings.len();
    let ring_start = VertexIndex::try_from(boundary.vertices.len())?;
    let ring_vertex_start = boundary.vertices.len();

    boundary.rings.push(ring_start);
    for vertex in &ring.vertices {
        boundary.vertices.push(resolver.resolve_vertex(vertex)?);
    }

    for (_, texture_map) in textures.iter_mut() {
        texture_map.add_ring(ring_start);
        texture_map.add_ring_texture(None);
        for _ in &ring.vertices {
            texture_map.add_vertex(None);
        }
    }

    for texture in &ring.textures {
        let resolved_uvs: Result<Vec<_>> = texture
            .uvs
            .iter()
            .map(|uv| resolver.resolve_uv(uv))
            .collect();
        let resolved_uvs = resolved_uvs?;

        if let Some((_, texture_map)) = textures
            .iter_mut()
            .find(|(theme, _)| theme == &texture.theme)
        {
            let ring_slot = texture_map.ring_textures_mut()[ring_index..=ring_index]
                .first_mut()
                .expect("ring slot exists");
            *ring_slot = Some(texture.texture.to_raw());
            let vertex_slice = &mut texture_map.vertices_mut()
                [ring_vertex_start..ring_vertex_start + resolved_uvs.len()];
            for (slot, uv) in vertex_slice.iter_mut().zip(resolved_uvs) {
                *slot = Some(uv);
            }
            continue;
        }

        let mut texture_map = TextureMapCore::default();
        for prior_vertex in 0..ring_vertex_start {
            let _ = prior_vertex;
            texture_map.add_vertex(None);
        }
        for &prior_ring_start in &boundary.rings()[..ring_index] {
            texture_map.add_ring(prior_ring_start);
            texture_map.add_ring_texture(None);
        }
        texture_map.add_ring(ring_start);
        texture_map.add_ring_texture(Some(texture.texture.to_raw()));
        for uv in resolved_uvs {
            texture_map.add_vertex(Some(uv));
        }
        textures.push((texture.theme.clone(), texture_map));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CityModelType;
    use crate::resources::handles::{MaterialHandle, SemanticHandle, TextureHandle};
    use crate::resources::storage::OwnedStringStorage;
    use crate::v2_0::appearance::ImageType;
    use crate::v2_0::{OwnedCityModel, OwnedMaterial, OwnedSemantic, OwnedTexture, SemanticType};

    fn new_model() -> OwnedCityModel {
        OwnedCityModel::new(CityModelType::CityJSON)
    }

    fn missing_semantic() -> SemanticHandle {
        unsafe { SemanticHandle::from_raw_parts_unchecked(99, 0) }
    }

    fn missing_material() -> MaterialHandle {
        unsafe { MaterialHandle::from_raw_parts_unchecked(99, 0) }
    }

    fn missing_texture() -> TextureHandle {
        unsafe { TextureHandle::from_raw_parts_unchecked(99, 0) }
    }

    fn triangle() -> RingDraft<u32, OwnedStringStorage> {
        RingDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
    }

    /// Inputs: drafts with missing required point, line, surface, shell, or
    /// solid children. Assertions: insertion rejects each incomplete authoring
    /// shape. Purpose: authoring-level coverage for empty required geometry
    /// parts before storage validation.
    #[test]
    fn draft_authoring_rejects_empty_required_parts() {
        let cases = [
            GeometryDraft::multi_point(None, []),
            GeometryDraft::multi_line_string(None, []),
            GeometryDraft::multi_line_string(None, [LineStringDraft::new(Vec::<[f64; 3]>::new())]),
            GeometryDraft::multi_surface(None, []),
            GeometryDraft::solid(None, ShellDraft::new([]), []),
            GeometryDraft::multi_solid(None, []),
        ];

        for draft in cases {
            assert!(draft.insert_into(&mut new_model()).is_err());
        }
    }

    /// Inputs: one surface with duplicate material themes and one ring with
    /// duplicate texture themes. Assertions: insertion rejects both duplicates.
    /// Purpose: protect one-assignment-per-theme authoring invariants.
    #[test]
    fn draft_authoring_rejects_duplicate_material_and_texture_themes() {
        let mut model = new_model();
        let material = model
            .add_material(OwnedMaterial::new("mat-a".to_string()))
            .unwrap();
        let surface = SurfaceDraft::new(triangle(), [])
            .with_material("theme-a".to_string(), material)
            .with_material("theme-a".to_string(), material);
        assert!(
            GeometryDraft::multi_surface(None, [surface])
                .insert_into(&mut model)
                .is_err()
        );

        let mut model = new_model();
        let texture = model
            .add_texture(OwnedTexture::new("tex-a.png".to_string(), ImageType::Png))
            .unwrap();
        let ring = triangle()
            .with_texture(
                "theme-a".to_string(),
                texture,
                [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            )
            .with_texture(
                "theme-a".to_string(),
                texture,
                [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            );
        let surface = SurfaceDraft::new(ring, []);
        assert!(
            GeometryDraft::multi_surface(None, [surface])
                .insert_into(&mut model)
                .is_err()
        );
    }

    /// Inputs: drafts referencing missing regular/template vertices and missing
    /// semantic, material, texture, and UV handles. Assertions: every missing
    /// handle is rejected before insertion mutates the model. Purpose: direct
    /// authoring coverage for resource and vertex reference preflight.
    #[test]
    fn draft_authoring_rejects_missing_handles() {
        let existing_vertex = VertexIndex::new(0_u32);
        assert!(
            GeometryDraft::multi_point(None, [PointDraft::new(existing_vertex)])
                .insert_into(&mut new_model())
                .is_err()
        );
        assert!(
            GeometryDraft::multi_point(None, [PointDraft::new(existing_vertex)])
                .insert_template_into(&mut new_model())
                .is_err()
        );

        assert!(
            GeometryDraft::multi_point(
                None,
                [PointDraft::new([0.0, 0.0, 0.0]).with_semantic(missing_semantic())],
            )
            .insert_into(&mut new_model())
            .is_err()
        );

        let surface = SurfaceDraft::new(triangle(), [])
            .with_material("theme-a".to_string(), missing_material());
        assert!(
            GeometryDraft::multi_surface(None, [surface])
                .insert_into(&mut new_model())
                .is_err()
        );

        let ring = triangle().with_texture(
            "theme-a".to_string(),
            missing_texture(),
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        );
        assert!(
            GeometryDraft::multi_surface(None, [SurfaceDraft::new(ring, [])])
                .insert_into(&mut new_model())
                .is_err()
        );

        let mut model = new_model();
        let texture = model
            .add_texture(OwnedTexture::new("tex-a.png".to_string(), ImageType::Png))
            .unwrap();
        let ring = triangle().with_texture(
            "theme-a".to_string(),
            texture,
            [
                VertexIndex::new(0_u32),
                VertexIndex::new(1),
                VertexIndex::new(2),
            ],
        );
        assert!(
            GeometryDraft::multi_surface(None, [SurfaceDraft::new(ring, [])])
                .insert_into(&mut model)
                .is_err()
        );
    }

    /// Inputs: drafts that repeat identical coordinates and identical UVs in one
    /// insertion. Assertions: the stored boundary preserves all occurrences but
    /// model vertex and UV pools receive only unique values. Purpose: authoring
    /// coverage for insertion-time vertex and UV deduplication.
    #[test]
    fn draft_authoring_deduplicates_vertices_and_uvs() {
        let mut model = new_model();
        let handle = GeometryDraft::multi_point(
            None,
            [
                PointDraft::new([0.0, 0.0, 0.0]),
                PointDraft::new([0.0, 0.0, 0.0]),
                PointDraft::new([1.0, 0.0, 0.0]),
            ],
        )
        .insert_into(&mut model)
        .unwrap();
        let boundary = model.get_geometry(handle).unwrap().boundaries().unwrap();
        assert_eq!(model.vertices().len(), 2);
        assert_eq!(boundary.vertices()[0], boundary.vertices()[1]);
        assert_ne!(boundary.vertices()[1], boundary.vertices()[2]);

        let mut model = new_model();
        let texture = model
            .add_texture(OwnedTexture::new("tex-a.png".to_string(), ImageType::Png))
            .unwrap();
        let ring = triangle().with_texture(
            "theme-a".to_string(),
            texture,
            [[0.0, 0.0], [0.0, 0.0], [1.0, 0.0]],
        );
        let handle = GeometryDraft::multi_surface(None, [SurfaceDraft::new(ring, [])])
            .insert_into(&mut model)
            .unwrap();
        let texture_map = model
            .get_geometry(handle)
            .unwrap()
            .textures()
            .unwrap()
            .first()
            .unwrap()
            .1;
        assert!(model.get_uv_coordinate(VertexIndex::new(0)).is_some());
        assert!(model.get_uv_coordinate(VertexIndex::new(1)).is_some());
        assert!(model.get_uv_coordinate(VertexIndex::new(2)).is_none());
        assert_eq!(texture_map.vertices()[0], texture_map.vertices()[1]);
        assert_ne!(texture_map.vertices()[1], texture_map.vertices()[2]);
    }

    /// Inputs: one valid semantic handle attached to a point draft. Assertions:
    /// insertion succeeds and creates a semantic map. Purpose: sanity-check the
    /// positive authoring path alongside the missing-handle cases.
    #[test]
    fn draft_authoring_accepts_existing_resource_handles() {
        let mut model = new_model();
        let semantic = model
            .add_semantic(OwnedSemantic::new(SemanticType::RoofSurface))
            .unwrap();
        let handle = GeometryDraft::multi_point(
            None,
            [PointDraft::new([0.0, 0.0, 0.0]).with_semantic(semantic)],
        )
        .insert_into(&mut model)
        .unwrap();

        assert!(model.get_geometry(handle).unwrap().semantics().is_some());
    }
}
