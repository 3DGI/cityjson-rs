use cityjson_lib::{
    CityJSONVersion,
    cityjson_types::{CityModelType, v2_0::GeometryType},
    json::RootKind,
};

/// Stable status codes for the shared C ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_status_t {
    CJ_STATUS_SUCCESS = 0,
    CJ_STATUS_INVALID_ARGUMENT = 1,
    CJ_STATUS_IO = 2,
    CJ_STATUS_SYNTAX = 3,
    CJ_STATUS_VERSION = 4,
    CJ_STATUS_SHAPE = 5,
    CJ_STATUS_UNSUPPORTED = 6,
    CJ_STATUS_MODEL = 7,
    CJ_STATUS_INTERNAL = 8,
}

impl Default for cj_status_t {
    fn default() -> Self {
        Self::CJ_STATUS_SUCCESS
    }
}

/// Stable error categories for the shared C ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_error_kind_t {
    CJ_ERROR_KIND_NONE = 0,
    CJ_ERROR_KIND_INVALID_ARGUMENT = 1,
    CJ_ERROR_KIND_IO = 2,
    CJ_ERROR_KIND_SYNTAX = 3,
    CJ_ERROR_KIND_VERSION = 4,
    CJ_ERROR_KIND_SHAPE = 5,
    CJ_ERROR_KIND_UNSUPPORTED = 6,
    CJ_ERROR_KIND_MODEL = 7,
    CJ_ERROR_KIND_INTERNAL = 8,
}

impl Default for cj_error_kind_t {
    fn default() -> Self {
        Self::CJ_ERROR_KIND_NONE
    }
}

/// Stable root type discriminant for probed inputs.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_root_kind_t {
    CJ_ROOT_KIND_CITY_JSON = 0,
    CJ_ROOT_KIND_CITY_JSON_FEATURE = 1,
}

impl Default for cj_root_kind_t {
    fn default() -> Self {
        Self::CJ_ROOT_KIND_CITY_JSON
    }
}

/// Stable version discriminant for probed inputs.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_version_t {
    CJ_VERSION_UNKNOWN = 0,
    CJ_VERSION_V1_0 = 1,
    CJ_VERSION_V1_1 = 2,
    CJ_VERSION_V2_0 = 3,
}

impl Default for cj_version_t {
    fn default() -> Self {
        Self::CJ_VERSION_UNKNOWN
    }
}

/// Stable model type discriminant for `CityJSON` documents and features.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_model_type_t {
    CJ_MODEL_TYPE_CITY_JSON = 0,
    CJ_MODEL_TYPE_CITY_JSON_FEATURE = 1,
}

impl Default for cj_model_type_t {
    fn default() -> Self {
        Self::CJ_MODEL_TYPE_CITY_JSON
    }
}

/// Stable geometry type discriminant for stored `CityJSON` geometries.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_geometry_type_t {
    CJ_GEOMETRY_TYPE_MULTI_POINT = 0,
    CJ_GEOMETRY_TYPE_MULTI_LINE_STRING = 1,
    CJ_GEOMETRY_TYPE_MULTI_SURFACE = 2,
    CJ_GEOMETRY_TYPE_COMPOSITE_SURFACE = 3,
    CJ_GEOMETRY_TYPE_SOLID = 4,
    CJ_GEOMETRY_TYPE_MULTI_SOLID = 5,
    CJ_GEOMETRY_TYPE_COMPOSITE_SOLID = 6,
    CJ_GEOMETRY_TYPE_GEOMETRY_INSTANCE = 7,
}

impl Default for cj_geometry_type_t {
    fn default() -> Self {
        Self::CJ_GEOMETRY_TYPE_MULTI_POINT
    }
}

/// Stable value discriminant for recursive CityJSON attribute trees.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_value_kind_t {
    CJ_VALUE_NULL = 0,
    CJ_VALUE_BOOL = 1,
    CJ_VALUE_INT64 = 2,
    CJ_VALUE_FLOAT64 = 3,
    CJ_VALUE_STRING = 4,
    CJ_VALUE_ARRAY = 5,
    CJ_VALUE_OBJECT = 6,
    CJ_VALUE_GEOMETRY_REF = 7,
}

impl Default for cj_value_kind_t {
    fn default() -> Self {
        Self::CJ_VALUE_NULL
    }
}

/// Stable contact-role discriminant.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_contact_role_t {
    CJ_CONTACT_ROLE_AUTHOR = 0,
    CJ_CONTACT_ROLE_CO_AUTHOR = 1,
    CJ_CONTACT_ROLE_PROCESSOR = 2,
    CJ_CONTACT_ROLE_POINT_OF_CONTACT = 3,
    CJ_CONTACT_ROLE_OWNER = 4,
    CJ_CONTACT_ROLE_USER = 5,
    CJ_CONTACT_ROLE_DISTRIBUTOR = 6,
    CJ_CONTACT_ROLE_ORIGINATOR = 7,
    CJ_CONTACT_ROLE_CUSTODIAN = 8,
    CJ_CONTACT_ROLE_RESOURCE_PROVIDER = 9,
    CJ_CONTACT_ROLE_RIGHTS_HOLDER = 10,
    CJ_CONTACT_ROLE_SPONSOR = 11,
    CJ_CONTACT_ROLE_PRINCIPAL_INVESTIGATOR = 12,
    CJ_CONTACT_ROLE_STAKEHOLDER = 13,
    CJ_CONTACT_ROLE_PUBLISHER = 14,
}

impl Default for cj_contact_role_t {
    fn default() -> Self {
        Self::CJ_CONTACT_ROLE_AUTHOR
    }
}

/// Stable contact-type discriminant.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_contact_type_t {
    CJ_CONTACT_TYPE_INDIVIDUAL = 0,
    CJ_CONTACT_TYPE_ORGANIZATION = 1,
}

impl Default for cj_contact_type_t {
    fn default() -> Self {
        Self::CJ_CONTACT_TYPE_INDIVIDUAL
    }
}

/// Stable texture image-type discriminant.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_image_type_t {
    CJ_IMAGE_TYPE_PNG = 0,
    CJ_IMAGE_TYPE_JPG = 1,
}

impl Default for cj_image_type_t {
    fn default() -> Self {
        Self::CJ_IMAGE_TYPE_PNG
    }
}

/// Stable texture wrap-mode discriminant.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_wrap_mode_t {
    CJ_WRAP_MODE_WRAP = 0,
    CJ_WRAP_MODE_MIRROR = 1,
    CJ_WRAP_MODE_CLAMP = 2,
    CJ_WRAP_MODE_BORDER = 3,
    CJ_WRAP_MODE_NONE = 4,
}

impl Default for cj_wrap_mode_t {
    fn default() -> Self {
        Self::CJ_WRAP_MODE_WRAP
    }
}

/// Stable texture-mapping discriminant.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum cj_texture_type_t {
    CJ_TEXTURE_TYPE_UNKNOWN = 0,
    CJ_TEXTURE_TYPE_SPECIFIC = 1,
    CJ_TEXTURE_TYPE_TYPICAL = 2,
}

impl Default for cj_texture_type_t {
    fn default() -> Self {
        Self::CJ_TEXTURE_TYPE_UNKNOWN
    }
}

/// Stable typed id for a model-owned cityobject handle.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_cityobject_id_t {
    pub slot: u32,
    pub generation: u16,
    pub reserved: u16,
}

/// Stable typed id for a model-owned geometry handle.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_geometry_id_t {
    pub slot: u32,
    pub generation: u16,
    pub reserved: u16,
}

/// Stable typed id for a model-owned template-geometry handle.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_geometry_template_id_t {
    pub slot: u32,
    pub generation: u16,
    pub reserved: u16,
}

/// Stable typed id for a model-owned semantic handle.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_semantic_id_t {
    pub slot: u32,
    pub generation: u16,
    pub reserved: u16,
}

/// Stable typed id for a model-owned material handle.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_material_id_t {
    pub slot: u32,
    pub generation: u16,
    pub reserved: u16,
}

/// Stable typed id for a model-owned texture handle.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_texture_id_t {
    pub slot: u32,
    pub generation: u16,
    pub reserved: u16,
}

/// Opaque model handle type.
///
/// The ABI only ever passes pointers to this marker type. The actual storage is
/// a boxed `cityjson_lib::CityModel` allocated by the Rust side.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_model_t {
    _private: [u8; 0],
}

/// Opaque handle for a cached PROJ coordinate transformer.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug)]
pub struct cj_proj_transformer_t {
    _private: [u8; 0],
}

/// Opaque model-selection handle type.
///
/// The ABI only ever passes pointers to this marker type. The actual storage is
/// a boxed `cityjson_lib::ops::ModelSelection` allocated by the Rust side.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_model_selection_t {
    _private: [u8; 0],
}

/// Opaque typed value handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_value_t {
    _private: [u8; 0],
}

/// Opaque metadata-contact authoring handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_contact_t {
    _private: [u8; 0],
}

/// Opaque cityobject-draft authoring handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_cityobject_draft_t {
    _private: [u8; 0],
}

/// Opaque ring-draft authoring handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_ring_draft_t {
    _private: [u8; 0],
}

/// Opaque surface-draft authoring handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_surface_draft_t {
    _private: [u8; 0],
}

/// Opaque shell-draft authoring handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_shell_draft_t {
    _private: [u8; 0],
}

/// Opaque solid-draft authoring handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_solid_draft_t {
    _private: [u8; 0],
}

/// Opaque geometry-draft authoring handle.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cj_geometry_draft_t {
    _private: [u8; 0],
}

/// Owned byte buffer returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_bytes_t {
    pub data: *mut u8,
    pub len: usize,
}

impl cj_bytes_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null_mut(),
            len: 0,
        }
    }

    pub const fn is_null(self) -> bool {
        self.data.is_null()
    }
}

/// Owned byte-buffer list returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_bytes_list_t {
    pub data: *mut cj_bytes_t,
    pub len: usize,
}

impl cj_bytes_list_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null_mut(),
            len: 0,
        }
    }

    pub const fn is_null(self) -> bool {
        self.data.is_null()
    }
}

/// Packed 3D coordinate copied across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cj_vertex_t {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Packed UV coordinate copied across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cj_uv_t {
    pub u: f32,
    pub v: f32,
}

/// Owned vertex buffer returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_vertices_t {
    pub data: *mut cj_vertex_t,
    pub len: usize,
}

impl cj_vertices_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null_mut(),
            len: 0,
        }
    }

    pub const fn is_null(self) -> bool {
        self.data.is_null()
    }
}

/// Owned geometry-type list returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_geometry_types_t {
    pub data: *mut cj_geometry_type_t,
    pub len: usize,
}

impl cj_geometry_types_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null_mut(),
            len: 0,
        }
    }

    pub const fn is_null(self) -> bool {
        self.data.is_null()
    }
}

/// Owned UV buffer returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_uvs_t {
    pub data: *mut cj_uv_t,
    pub len: usize,
}

impl cj_uvs_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null_mut(),
            len: 0,
        }
    }

    pub const fn is_null(self) -> bool {
        self.data.is_null()
    }
}

/// Owned index buffer returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_indices_t {
    pub data: *mut usize,
    pub len: usize,
}

impl cj_indices_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null_mut(),
            len: 0,
        }
    }

    pub const fn is_null(self) -> bool {
        self.data.is_null()
    }
}

/// Borrowed UTF-8 string view passed into the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_string_view_t {
    pub data: *const u8,
    pub len: usize,
}

impl cj_string_view_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null(),
            len: 0,
        }
    }
}

/// Borrowed index-slice view passed into the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_indices_view_t {
    pub data: *const usize,
    pub len: usize,
}

impl cj_indices_view_t {
    pub const fn null() -> Self {
        Self {
            data: core::ptr::null(),
            len: 0,
        }
    }
}

/// Owned flat boundary payload returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_geometry_boundary_t {
    pub geometry_type: cj_geometry_type_t,
    pub has_boundaries: bool,
    pub vertex_indices: cj_indices_t,
    pub ring_offsets: cj_indices_t,
    pub surface_offsets: cj_indices_t,
    pub shell_offsets: cj_indices_t,
    pub solid_offsets: cj_indices_t,
}

/// Borrowed flat boundary payload passed into the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_geometry_boundary_view_t {
    pub geometry_type: cj_geometry_type_t,
    pub vertex_indices: cj_indices_view_t,
    pub ring_offsets: cj_indices_view_t,
    pub surface_offsets: cj_indices_view_t,
    pub shell_offsets: cj_indices_view_t,
    pub solid_offsets: cj_indices_view_t,
}

/// CityObject id and geometry index pair used by model-selection APIs.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_geometry_selection_spec_t {
    pub cityobject_id: cj_string_view_t,
    pub geometry_index: usize,
}

/// Probe result returned by the low-level ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_probe_t {
    pub root_kind: cj_root_kind_t,
    pub version: cj_version_t,
    pub has_version: bool,
}

impl cj_probe_t {
    pub fn from_probe(probe: &cityjson_lib::json::Probe) -> Self {
        Self {
            root_kind: probe.kind().into(),
            version: probe
                .version()
                .map_or(cj_version_t::CJ_VERSION_UNKNOWN, Into::into),
            has_version: probe.version().is_some(),
        }
    }
}

/// Aggregate model inspection summary returned across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_model_summary_t {
    pub model_type: cj_model_type_t,
    pub version: cj_version_t,
    pub cityobject_count: usize,
    pub geometry_count: usize,
    pub geometry_template_count: usize,
    pub vertex_count: usize,
    pub template_vertex_count: usize,
    pub uv_coordinate_count: usize,
    pub semantic_count: usize,
    pub material_count: usize,
    pub texture_count: usize,
    pub extension_count: usize,
    pub has_metadata: bool,
    pub has_transform: bool,
    pub has_templates: bool,
    pub has_appearance: bool,
}

/// Capacity hints for bulk import and model-building paths.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_model_capacities_t {
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

/// Explicit JSON write options for document, feature, and feature-stream output.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_json_write_options_t {
    pub pretty: bool,
    pub validate_default_themes: bool,
}

/// Explicit strict `CityJSONSeq` write options.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cj_cityjsonseq_write_options_t {
    pub validate_default_themes: bool,
    pub trailing_newline: bool,
    pub update_metadata_geographical_extent: bool,
}

/// Auto-transform options for strict `CityJSONSeq` writing.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cj_cityjsonseq_auto_transform_options_t {
    pub scale_x: f64,
    pub scale_y: f64,
    pub scale_z: f64,
    pub validate_default_themes: bool,
    pub trailing_newline: bool,
    pub update_metadata_geographical_extent: bool,
}

/// Explicit root-transform state for JSON write and edit workflows.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cj_transform_t {
    pub scale_x: f64,
    pub scale_y: f64,
    pub scale_z: f64,
    pub translate_x: f64,
    pub translate_y: f64,
    pub translate_z: f64,
}

/// Packed bbox copied across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cj_bbox_t {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

/// Packed RGB color copied across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cj_rgb_t {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// Packed RGBA color copied across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cj_rgba_t {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// Packed 4x4 affine transform copied across the ABI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct cj_affine_transform_4x4_t {
    pub elements: [f64; 16],
}

impl Default for cj_affine_transform_4x4_t {
    fn default() -> Self {
        Self {
            elements: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }
}

impl From<RootKind> for cj_root_kind_t {
    fn from(value: RootKind) -> Self {
        match value {
            RootKind::CityJSON => Self::CJ_ROOT_KIND_CITY_JSON,
            RootKind::CityJSONFeature => Self::CJ_ROOT_KIND_CITY_JSON_FEATURE,
        }
    }
}

impl From<cj_root_kind_t> for RootKind {
    fn from(value: cj_root_kind_t) -> Self {
        match value {
            cj_root_kind_t::CJ_ROOT_KIND_CITY_JSON => Self::CityJSON,
            cj_root_kind_t::CJ_ROOT_KIND_CITY_JSON_FEATURE => Self::CityJSONFeature,
        }
    }
}

impl From<CityJSONVersion> for cj_version_t {
    fn from(value: CityJSONVersion) -> Self {
        match value {
            CityJSONVersion::V1_0 => Self::CJ_VERSION_V1_0,
            CityJSONVersion::V1_1 => Self::CJ_VERSION_V1_1,
            CityJSONVersion::V2_0 => Self::CJ_VERSION_V2_0,
        }
    }
}

impl From<CityModelType> for cj_model_type_t {
    fn from(value: CityModelType) -> Self {
        match value {
            CityModelType::CityJSON => Self::CJ_MODEL_TYPE_CITY_JSON,
            CityModelType::CityJSONFeature => Self::CJ_MODEL_TYPE_CITY_JSON_FEATURE,
            _ => Self::CJ_MODEL_TYPE_CITY_JSON,
        }
    }
}

impl From<cj_model_type_t> for CityModelType {
    fn from(value: cj_model_type_t) -> Self {
        match value {
            cj_model_type_t::CJ_MODEL_TYPE_CITY_JSON => Self::CityJSON,
            cj_model_type_t::CJ_MODEL_TYPE_CITY_JSON_FEATURE => Self::CityJSONFeature,
        }
    }
}

impl From<GeometryType> for cj_geometry_type_t {
    fn from(value: GeometryType) -> Self {
        match value {
            GeometryType::MultiPoint => Self::CJ_GEOMETRY_TYPE_MULTI_POINT,
            GeometryType::MultiLineString => Self::CJ_GEOMETRY_TYPE_MULTI_LINE_STRING,
            GeometryType::MultiSurface => Self::CJ_GEOMETRY_TYPE_MULTI_SURFACE,
            GeometryType::CompositeSurface => Self::CJ_GEOMETRY_TYPE_COMPOSITE_SURFACE,
            GeometryType::Solid => Self::CJ_GEOMETRY_TYPE_SOLID,
            GeometryType::MultiSolid => Self::CJ_GEOMETRY_TYPE_MULTI_SOLID,
            GeometryType::CompositeSolid => Self::CJ_GEOMETRY_TYPE_COMPOSITE_SOLID,
            GeometryType::GeometryInstance => Self::CJ_GEOMETRY_TYPE_GEOMETRY_INSTANCE,
            _ => Self::CJ_GEOMETRY_TYPE_MULTI_POINT,
        }
    }
}

impl TryFrom<cj_version_t> for CityJSONVersion {
    type Error = ();

    fn try_from(value: cj_version_t) -> Result<Self, Self::Error> {
        match value {
            cj_version_t::CJ_VERSION_V1_0 => Ok(Self::V1_0),
            cj_version_t::CJ_VERSION_V1_1 => Ok(Self::V1_1),
            cj_version_t::CJ_VERSION_V2_0 => Ok(Self::V2_0),
            cj_version_t::CJ_VERSION_UNKNOWN => Err(()),
        }
    }
}

impl From<Option<CityJSONVersion>> for cj_version_t {
    fn from(value: Option<CityJSONVersion>) -> Self {
        match value {
            Some(version) => version.into(),
            None => Self::CJ_VERSION_UNKNOWN,
        }
    }
}

impl From<cityjson_lib::cityjson_types::v2_0::RealWorldCoordinate> for cj_vertex_t {
    fn from(value: cityjson_lib::cityjson_types::v2_0::RealWorldCoordinate) -> Self {
        Self {
            x: value.x(),
            y: value.y(),
            z: value.z(),
        }
    }
}

impl From<cj_vertex_t> for cityjson_lib::cityjson_types::v2_0::RealWorldCoordinate {
    fn from(value: cj_vertex_t) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<cityjson_lib::cityjson_types::v2_0::UVCoordinate> for cj_uv_t {
    fn from(value: cityjson_lib::cityjson_types::v2_0::UVCoordinate) -> Self {
        Self {
            u: value.u(),
            v: value.v(),
        }
    }
}

impl From<cj_uv_t> for cityjson_lib::cityjson_types::v2_0::UVCoordinate {
    fn from(value: cj_uv_t) -> Self {
        Self::new(value.u, value.v)
    }
}

impl From<cj_model_capacities_t> for cityjson_lib::cityjson_types::v2_0::CityModelCapacities {
    fn from(value: cj_model_capacities_t) -> Self {
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
