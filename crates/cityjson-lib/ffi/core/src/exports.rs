#![allow(non_camel_case_types)]

use std::collections::HashSet;
use std::ffi::c_char;
#[cfg(feature = "native-formats")]
use std::io::Cursor;
use std::ptr::{self, NonNull};
use std::slice;
use std::str::FromStr;

use cityjson_lib::cityjson_types::v2_0::{
    Boundary, BoundaryNestedMultiLineString, BoundaryNestedMultiOrCompositeSolid,
    BoundaryNestedMultiOrCompositeSurface, BoundaryNestedMultiPoint, BoundaryNestedSolid,
    CityModelIdentifier, CityObject, CityObjectIdentifier, CityObjectType, Contact, ContactRole,
    ContactType, Extension, Geometry, GeometryType, ImageType, LoD, Material, RGB, RGBA, Semantic,
    SemanticType, StoredGeometryParts, Texture, TextureType, Transform, WrapMode,
};
use cityjson_lib::{
    CityJSONVersion, CityModel, Error, cityjson_types::CityModelType, json::RootKind,
};

use crate::abi::{
    cj_affine_transform_4x4_t, cj_bbox_t, cj_bytes_list_t, cj_bytes_t,
    cj_cityjsonseq_auto_transform_options_t, cj_cityjsonseq_write_options_t, cj_cityobject_draft_t,
    cj_cityobject_id_t, cj_contact_role_t, cj_contact_t, cj_contact_type_t, cj_error_kind_t,
    cj_geometry_boundary_t, cj_geometry_boundary_view_t, cj_geometry_draft_t, cj_geometry_id_t,
    cj_geometry_selection_spec_t, cj_geometry_template_id_t, cj_geometry_type_t,
    cj_geometry_types_t, cj_image_type_t, cj_indices_t, cj_indices_view_t, cj_json_write_options_t,
    cj_material_id_t, cj_model_capacities_t, cj_model_selection_t, cj_model_summary_t, cj_model_t,
    cj_model_type_t, cj_probe_t, cj_proj_transformer_t, cj_rgb_t, cj_rgba_t, cj_ring_draft_t,
    cj_semantic_id_t, cj_shell_draft_t, cj_solid_draft_t, cj_status_t, cj_string_view_t,
    cj_surface_draft_t, cj_texture_id_t, cj_texture_type_t, cj_transform_t, cj_uv_t, cj_uvs_t,
    cj_value_t, cj_vertex_t, cj_vertices_t, cj_wrap_mode_t,
};
use crate::authoring::{
    GeometryAuthoring, LineStringAuthoring, OwnedCityObject, OwnedContact, OwnedMaterial,
    OwnedSemantic, OwnedTexture, OwnedValue, PointAuthoring, RingAuthoring, RingTextureAuthoring,
    ShellAuthoring, SolidAuthoring, SurfaceAuthoring, UvAuthoring, VertexAuthoring,
};
use crate::error::{
    AbiError, clear_last_error, copy_last_error_message, last_error_kind, last_error_message_len,
    run_ffi,
};
use crate::handle::{
    bytes_free as free_bytes, bytes_from_string, bytes_from_vec,
    bytes_list_free as free_bytes_list, bytes_list_from_vec, cityobject_draft_as_mut,
    cityobject_draft_free, cityobject_draft_into_handle, cityobject_draft_take, contact_as_mut,
    contact_free, contact_into_handle, contact_take,
    geometry_boundary_free as free_geometry_boundary, geometry_draft_as_mut, geometry_draft_free,
    geometry_draft_into_handle, geometry_draft_take, geometry_types_free as free_geometry_types,
    geometry_types_from_vec, indices_free as free_indices, indices_from_vec, model_as_mut,
    model_as_ref, model_free, model_into_handle, model_selection_as_ref, model_selection_free,
    model_selection_into_handle, ring_draft_as_mut, ring_draft_free, ring_draft_into_handle,
    ring_draft_take, shell_draft_as_mut, shell_draft_free, shell_draft_into_handle,
    shell_draft_take, solid_draft_as_mut, solid_draft_free, solid_draft_into_handle,
    solid_draft_take, surface_draft_as_mut, surface_draft_free, surface_draft_into_handle,
    surface_draft_take, uvs_free as free_uvs, uvs_from_vec, value_as_mut, value_free,
    value_into_handle, value_take, vertices_free as free_vertices, vertices_from_vec,
};

/// cbindgen:ignore
type OwnedGeometry = cityjson_lib::cityjson_types::v2_0::Geometry<
    u32,
    cityjson_lib::cityjson_types::resources::storage::OwnedStringStorage,
>;

fn invalid_argument(message: impl Into<String>) -> AbiError {
    AbiError::invalid_argument(message)
}

#[cfg_attr(feature = "proj", allow(dead_code))]
fn unsupported(message: impl Into<String>) -> AbiError {
    AbiError::new(
        cj_status_t::CJ_STATUS_UNSUPPORTED,
        cj_error_kind_t::CJ_ERROR_KIND_UNSUPPORTED,
        message,
    )
}

fn ffi_status(result: Result<(), cj_status_t>) -> cj_status_t {
    match result {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

fn required_bytes<'a>(
    data: *const u8,
    len: usize,
    name: &'static str,
) -> Result<&'a [u8], AbiError> {
    if len == 0 {
        return Ok(&[]);
    }

    let ptr = NonNull::new(data.cast_mut())
        .ok_or_else(|| invalid_argument(format!("{name} must not be null when len is non-zero")))?;

    // SAFETY: the caller promises `len` readable bytes when the pointer is non-null.
    Ok(unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), len) })
}

fn optional_bytes<'a>(
    data: *const u8,
    len: usize,
    name: &'static str,
) -> Result<Option<&'a [u8]>, AbiError> {
    if len == 0 {
        return Ok(None);
    }

    required_bytes(data, len, name).map(Some)
}

fn optional_utf8(
    data: *const u8,
    len: usize,
    name: &'static str,
) -> Result<Option<String>, AbiError> {
    optional_bytes(data, len, name)?
        .map(|bytes| {
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|error| invalid_argument(format!("{name} must be valid UTF-8: {error}")))
        })
        .transpose()
}

fn required_utf8(data: *const u8, len: usize, name: &'static str) -> Result<String, AbiError> {
    let bytes = required_bytes(data, len, name)?;
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|error| invalid_argument(format!("{name} must be valid UTF-8: {error}")))
}

fn view_utf8(view: cj_string_view_t, name: &'static str) -> Result<String, AbiError> {
    required_utf8(view.data, view.len, name)
}

fn optional_view_utf8(
    view: cj_string_view_t,
    name: &'static str,
) -> Result<Option<String>, AbiError> {
    optional_utf8(view.data, view.len, name)
}

fn required_indices_view(
    view: cj_indices_view_t,
    name: &'static str,
) -> Result<&'static [usize], AbiError> {
    if view.len == 0 {
        return Ok(&[]);
    }

    let ptr = NonNull::new(view.data.cast_mut())
        .ok_or_else(|| invalid_argument(format!("{name} must not be null when len is non-zero")))?;

    // SAFETY: the caller promises `len` readable indices when the pointer is non-null.
    Ok(unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), view.len) })
}

fn required_string_views(
    data: *const cj_string_view_t,
    len: usize,
    name: &'static str,
) -> Result<&'static [cj_string_view_t], AbiError> {
    if len == 0 {
        return Ok(&[]);
    }

    let ptr = NonNull::new(data.cast_mut())
        .ok_or_else(|| invalid_argument(format!("{name} must not be null when len is non-zero")))?;

    // SAFETY: the caller promises `len` readable views when the pointer is non-null.
    Ok(unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), len) })
}

fn required_geometry_selection_specs(
    data: *const cj_geometry_selection_spec_t,
    len: usize,
    name: &'static str,
) -> Result<&'static [cj_geometry_selection_spec_t], AbiError> {
    if len == 0 {
        return Ok(&[]);
    }

    let ptr = NonNull::new(data.cast_mut())
        .ok_or_else(|| invalid_argument(format!("{name} must not be null when len is non-zero")))?;

    // SAFETY: the caller promises `len` readable specs when the pointer is non-null.
    Ok(unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), len) })
}

fn required_model_ref<'a>(model: *const cj_model_t) -> Result<&'a CityModel, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { model_as_ref(model) }.ok_or_else(|| invalid_argument("model must not be null"))
}

fn required_model_mut<'a>(model: *mut cj_model_t) -> Result<&'a mut CityModel, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { model_as_mut(model) }.ok_or_else(|| invalid_argument("model must not be null"))
}

fn required_model_selection_ref<'a>(
    selection: *const cj_model_selection_t,
) -> Result<&'a cityjson_lib::ops::ModelSelection, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { model_selection_as_ref(selection) }
        .ok_or_else(|| invalid_argument("selection must not be null"))
}

fn required_value_mut<'a>(value: *mut cj_value_t) -> Result<&'a mut OwnedValue, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { value_as_mut(value) }.ok_or_else(|| invalid_argument("value must not be null"))
}

fn required_contact_mut<'a>(contact: *mut cj_contact_t) -> Result<&'a mut OwnedContact, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { contact_as_mut(contact) }.ok_or_else(|| invalid_argument("contact must not be null"))
}

fn required_cityobject_draft_mut<'a>(
    draft: *mut cj_cityobject_draft_t,
) -> Result<&'a mut OwnedCityObject, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { cityobject_draft_as_mut(draft) }
        .ok_or_else(|| invalid_argument("draft must not be null"))
}

fn required_ring_draft_mut<'a>(
    draft: *mut cj_ring_draft_t,
) -> Result<&'a mut RingAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { ring_draft_as_mut(draft) }.ok_or_else(|| invalid_argument("ring must not be null"))
}

fn required_surface_draft_mut<'a>(
    draft: *mut cj_surface_draft_t,
) -> Result<&'a mut SurfaceAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { surface_draft_as_mut(draft) }
        .ok_or_else(|| invalid_argument("surface must not be null"))
}

fn required_shell_draft_mut<'a>(
    draft: *mut cj_shell_draft_t,
) -> Result<&'a mut ShellAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { shell_draft_as_mut(draft) }.ok_or_else(|| invalid_argument("shell must not be null"))
}

fn required_solid_draft_mut<'a>(
    draft: *mut cj_solid_draft_t,
) -> Result<&'a mut SolidAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { solid_draft_as_mut(draft) }.ok_or_else(|| invalid_argument("solid must not be null"))
}

fn required_geometry_draft_mut<'a>(
    draft: *mut cj_geometry_draft_t,
) -> Result<&'a mut GeometryAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    unsafe { geometry_draft_as_mut(draft) }
        .ok_or_else(|| invalid_argument("geometry draft must not be null"))
}

#[cfg(feature = "proj")]
fn required_proj_transformer<'a>(
    transformer: *const cj_proj_transformer_t,
) -> Result<&'a cityjson_lib::ops::Transformer, AbiError> {
    let ptr = NonNull::new(transformer.cast_mut())
        .ok_or_else(|| invalid_argument("transformer must not be null"))?;
    // SAFETY: valid transformer handles originate from `cj_proj_transformer_create`.
    Ok(unsafe { &*ptr.as_ptr().cast::<cityjson_lib::ops::Transformer>() })
}

fn write_value<T>(out: *mut T, name: &'static str, value: T) -> Result<(), AbiError> {
    let out =
        NonNull::new(out).ok_or_else(|| invalid_argument(format!("{name} must not be null")))?;

    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), value);
    }

    Ok(())
}

fn required_out<T>(out: *mut T, name: &'static str) -> Result<NonNull<T>, AbiError> {
    NonNull::new(out).ok_or_else(|| invalid_argument(format!("{name} must not be null")))
}

fn write_model_handle(out_model: *mut *mut cj_model_t, model: CityModel) -> Result<(), AbiError> {
    let out = required_out(out_model, "out_model")?;

    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), model_into_handle(model));
    }

    Ok(())
}

fn write_model_selection_handle(
    out_selection: *mut *mut cj_model_selection_t,
    selection: cityjson_lib::ops::ModelSelection,
) -> Result<(), AbiError> {
    let out = required_out(out_selection, "out_selection")?;

    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), model_selection_into_handle(selection));
    }

    Ok(())
}

fn write_bytes(out_bytes: *mut cj_bytes_t, bytes: Vec<u8>) -> Result<(), AbiError> {
    let out = required_out(out_bytes, "out_bytes")?;

    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), bytes_from_vec(bytes));
    }

    Ok(())
}

fn write_vertices(
    out_vertices: *mut cj_vertices_t,
    vertices: Vec<cj_vertex_t>,
) -> Result<(), AbiError> {
    let out = required_out(out_vertices, "out_vertices")?;

    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), vertices_from_vec(vertices));
    }

    Ok(())
}

fn write_uvs(out_uvs: *mut cj_uvs_t, uvs: Vec<cj_uv_t>) -> Result<(), AbiError> {
    let out = required_out(out_uvs, "out_uvs")?;

    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), uvs_from_vec(uvs));
    }

    Ok(())
}

fn write_boundary(
    out_boundary: *mut cj_geometry_boundary_t,
    boundary: cj_geometry_boundary_t,
) -> Result<(), AbiError> {
    let out = required_out(out_boundary, "out_boundary")?;

    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), boundary);
    }

    Ok(())
}

fn required_model_refs<'a>(
    models: *const *const cj_model_t,
    model_count: usize,
    name: &'static str,
) -> Result<Vec<&'a CityModel>, AbiError> {
    if model_count == 0 {
        return Ok(Vec::new());
    }

    let models_ptr = NonNull::new(models.cast_mut()).ok_or_else(|| {
        invalid_argument(format!(
            "{name} must not be null when model_count is non-zero"
        ))
    })?;
    let models = unsafe { slice::from_raw_parts(models_ptr.as_ptr().cast_const(), model_count) };
    models
        .iter()
        .map(|handle| required_model_ref(*handle))
        .collect::<Result<Vec<_>, _>>()
}

fn transform_from_abi(transform: cj_transform_t) -> Transform {
    let mut value = Transform::new();
    value.set_scale([transform.scale_x, transform.scale_y, transform.scale_z]);
    value.set_translate([
        transform.translate_x,
        transform.translate_y,
        transform.translate_z,
    ]);
    value
}

fn bbox_from_abi(bbox: cj_bbox_t) -> cityjson_lib::cityjson_types::v2_0::BBox {
    cityjson_lib::cityjson_types::v2_0::BBox::new(
        bbox.min_x, bbox.min_y, bbox.min_z, bbox.max_x, bbox.max_y, bbox.max_z,
    )
}

fn affine_transform_from_abi(
    value: cj_affine_transform_4x4_t,
) -> cityjson_lib::cityjson_types::v2_0::AffineTransform3D {
    cityjson_lib::cityjson_types::v2_0::AffineTransform3D::new(value.elements)
}

fn semantic_from_abi(
    value: cj_semantic_id_t,
) -> cityjson_lib::cityjson_types::resources::handles::SemanticHandle {
    value.into()
}

fn material_from_abi(
    value: cj_material_id_t,
) -> cityjson_lib::cityjson_types::resources::handles::MaterialHandle {
    value.into()
}

fn texture_from_abi(
    value: cj_texture_id_t,
) -> cityjson_lib::cityjson_types::resources::handles::TextureHandle {
    value.into()
}

fn geometry_from_abi(
    value: cj_geometry_id_t,
) -> cityjson_lib::cityjson_types::resources::handles::GeometryHandle {
    value.into()
}

fn geometry_template_from_abi(
    value: cj_geometry_template_id_t,
) -> cityjson_lib::cityjson_types::resources::handles::GeometryTemplateHandle {
    value.into()
}

fn cityobject_from_abi(
    value: cj_cityobject_id_t,
) -> cityjson_lib::cityjson_types::resources::handles::CityObjectHandle {
    value.into()
}

fn contact_role_from_abi(value: cj_contact_role_t) -> ContactRole {
    match value {
        cj_contact_role_t::CJ_CONTACT_ROLE_AUTHOR => ContactRole::Author,
        cj_contact_role_t::CJ_CONTACT_ROLE_CO_AUTHOR => ContactRole::CoAuthor,
        cj_contact_role_t::CJ_CONTACT_ROLE_PROCESSOR => ContactRole::Processor,
        cj_contact_role_t::CJ_CONTACT_ROLE_POINT_OF_CONTACT => ContactRole::PointOfContact,
        cj_contact_role_t::CJ_CONTACT_ROLE_OWNER => ContactRole::Owner,
        cj_contact_role_t::CJ_CONTACT_ROLE_USER => ContactRole::User,
        cj_contact_role_t::CJ_CONTACT_ROLE_DISTRIBUTOR => ContactRole::Distributor,
        cj_contact_role_t::CJ_CONTACT_ROLE_ORIGINATOR => ContactRole::Originator,
        cj_contact_role_t::CJ_CONTACT_ROLE_CUSTODIAN => ContactRole::Custodian,
        cj_contact_role_t::CJ_CONTACT_ROLE_RESOURCE_PROVIDER => ContactRole::ResourceProvider,
        cj_contact_role_t::CJ_CONTACT_ROLE_RIGHTS_HOLDER => ContactRole::RightsHolder,
        cj_contact_role_t::CJ_CONTACT_ROLE_SPONSOR => ContactRole::Sponsor,
        cj_contact_role_t::CJ_CONTACT_ROLE_PRINCIPAL_INVESTIGATOR => {
            ContactRole::PrincipalInvestigator
        }
        cj_contact_role_t::CJ_CONTACT_ROLE_STAKEHOLDER => ContactRole::Stakeholder,
        cj_contact_role_t::CJ_CONTACT_ROLE_PUBLISHER => ContactRole::Publisher,
    }
}

fn contact_type_from_abi(value: cj_contact_type_t) -> ContactType {
    match value {
        cj_contact_type_t::CJ_CONTACT_TYPE_INDIVIDUAL => ContactType::Individual,
        cj_contact_type_t::CJ_CONTACT_TYPE_ORGANIZATION => ContactType::Organization,
    }
}

fn image_type_from_abi(value: cj_image_type_t) -> ImageType {
    match value {
        cj_image_type_t::CJ_IMAGE_TYPE_PNG => ImageType::Png,
        cj_image_type_t::CJ_IMAGE_TYPE_JPG => ImageType::Jpg,
    }
}

fn wrap_mode_from_abi(value: cj_wrap_mode_t) -> WrapMode {
    match value {
        cj_wrap_mode_t::CJ_WRAP_MODE_WRAP => WrapMode::Wrap,
        cj_wrap_mode_t::CJ_WRAP_MODE_MIRROR => WrapMode::Mirror,
        cj_wrap_mode_t::CJ_WRAP_MODE_CLAMP => WrapMode::Clamp,
        cj_wrap_mode_t::CJ_WRAP_MODE_BORDER => WrapMode::Border,
        cj_wrap_mode_t::CJ_WRAP_MODE_NONE => WrapMode::None,
    }
}

fn texture_type_from_abi(value: cj_texture_type_t) -> TextureType {
    match value {
        cj_texture_type_t::CJ_TEXTURE_TYPE_UNKNOWN => TextureType::Unknown,
        cj_texture_type_t::CJ_TEXTURE_TYPE_SPECIFIC => TextureType::Specific,
        cj_texture_type_t::CJ_TEXTURE_TYPE_TYPICAL => TextureType::Typical,
    }
}

/// cbindgen:ignore
fn semantic_type_from_string(
    value: String,
) -> SemanticType<cityjson_lib::cityjson_types::resources::storage::OwnedStringStorage> {
    match value.as_str() {
        "Default" => SemanticType::Default,
        "RoofSurface" => SemanticType::RoofSurface,
        "GroundSurface" => SemanticType::GroundSurface,
        "WallSurface" => SemanticType::WallSurface,
        "ClosureSurface" => SemanticType::ClosureSurface,
        "OuterCeilingSurface" => SemanticType::OuterCeilingSurface,
        "OuterFloorSurface" => SemanticType::OuterFloorSurface,
        "Window" => SemanticType::Window,
        "Door" => SemanticType::Door,
        "InteriorWallSurface" => SemanticType::InteriorWallSurface,
        "CeilingSurface" => SemanticType::CeilingSurface,
        "FloorSurface" => SemanticType::FloorSurface,
        "WaterSurface" => SemanticType::WaterSurface,
        "WaterGroundSurface" => SemanticType::WaterGroundSurface,
        "WaterClosureSurface" => SemanticType::WaterClosureSurface,
        "TrafficArea" => SemanticType::TrafficArea,
        "AuxiliaryTrafficArea" => SemanticType::AuxiliaryTrafficArea,
        "TransportationMarking" => SemanticType::TransportationMarking,
        "TransportationHole" => SemanticType::TransportationHole,
        _ if value.starts_with('+') => SemanticType::Extension(value),
        _ => SemanticType::Extension(value),
    }
}

fn rgb_from_abi(value: cj_rgb_t) -> RGB {
    RGB::new(value.r, value.g, value.b)
}

fn rgba_from_abi(value: cj_rgba_t) -> RGBA {
    RGBA::new(value.r, value.g, value.b, value.a)
}

fn copy_string_bytes(value: Option<&str>) -> Vec<u8> {
    value.unwrap_or_default().as_bytes().to_vec()
}

fn cityobject_ids_from_model(model: &CityModel) -> Vec<cj_bytes_t> {
    model
        .cityobjects()
        .iter()
        .map(|(_, cityobject)| bytes_from_string(cityobject.id().to_owned()))
        .collect()
}

fn geometry_types_from_model(model: &CityModel) -> Vec<cj_geometry_type_t> {
    model
        .iter_geometries()
        .map(|(_, geometry)| (*geometry.type_geometry()).into())
        .collect()
}

fn index_values(indices: &[cityjson_lib::cityjson_types::v2_0::VertexIndex<u32>]) -> Vec<usize> {
    indices.iter().map(|index| index.to_usize()).collect()
}

fn empty_boundary(geometry: &OwnedGeometry) -> cj_geometry_boundary_t {
    cj_geometry_boundary_t {
        geometry_type: (*geometry.type_geometry()).into(),
        has_boundaries: false,
        vertex_indices: cj_indices_t::null(),
        ring_offsets: cj_indices_t::null(),
        surface_offsets: cj_indices_t::null(),
        shell_offsets: cj_indices_t::null(),
        solid_offsets: cj_indices_t::null(),
    }
}

fn boundary_from_geometry(geometry: &OwnedGeometry) -> cj_geometry_boundary_t {
    let Some(boundary) = geometry.boundaries() else {
        return empty_boundary(geometry);
    };

    let columnar = boundary.to_columnar();
    cj_geometry_boundary_t {
        geometry_type: (*geometry.type_geometry()).into(),
        has_boundaries: true,
        vertex_indices: indices_from_vec(index_values(columnar.vertices)),
        ring_offsets: indices_from_vec(index_values(columnar.ring_offsets)),
        surface_offsets: indices_from_vec(index_values(columnar.surface_offsets)),
        shell_offsets: indices_from_vec(index_values(columnar.shell_offsets)),
        solid_offsets: indices_from_vec(index_values(columnar.solid_offsets)),
    }
}

fn geometry_at(model: &CityModel, index: usize) -> Result<&OwnedGeometry, AbiError> {
    model
        .iter_geometries()
        .nth(index)
        .map(|(_, geometry)| geometry)
        .ok_or_else(|| invalid_argument(format!("geometry index {index} is out of range")))
}

fn geometry_boundary_coordinates(
    model: &CityModel,
    index: usize,
) -> Result<Vec<cj_vertex_t>, AbiError> {
    let geometry = geometry_at(model, index)?;

    Ok(geometry
        .coordinates(model.vertices())
        .map_or_else(Vec::new, |coordinates| {
            coordinates.copied().map(Into::into).collect()
        }))
}

/// cbindgen:ignore
fn find_cityobject_mut<'a>(
    model: &'a mut CityModel,
    id: &str,
) -> Result<
    &'a mut CityObject<cityjson_lib::cityjson_types::resources::storage::OwnedStringStorage>,
    AbiError,
> {
    model
        .cityobjects_mut()
        .iter_mut()
        .find_map(|(_, cityobject)| (cityobject.id() == id).then_some(cityobject))
        .ok_or_else(|| invalid_argument(format!("CityObject '{id}' was not found")))
}

fn find_geometry_handle(
    model: &CityModel,
    index: usize,
) -> Result<cityjson_lib::cityjson_types::resources::handles::GeometryHandle, AbiError> {
    model
        .iter_geometries()
        .nth(index)
        .map(|(handle, _)| handle)
        .ok_or_else(|| invalid_argument(format!("geometry index {index} is out of range")))
}

fn required_semantic_mut(
    model: &mut CityModel,
    semantic: cj_semantic_id_t,
) -> Result<&mut OwnedSemantic, AbiError> {
    model
        .get_semantic_mut(semantic_from_abi(semantic))
        .ok_or_else(|| invalid_argument("semantic id is invalid for this model"))
}

fn required_material_mut(
    model: &mut CityModel,
    material: cj_material_id_t,
) -> Result<&mut OwnedMaterial, AbiError> {
    model
        .get_material_mut(material_from_abi(material))
        .ok_or_else(|| invalid_argument("material id is invalid for this model"))
}

fn required_texture_mut(
    model: &mut CityModel,
    texture: cj_texture_id_t,
) -> Result<&mut OwnedTexture, AbiError> {
    model
        .get_texture_mut(texture_from_abi(texture))
        .ok_or_else(|| invalid_argument("texture id is invalid for this model"))
}

fn required_cityobject_by_handle_mut(
    model: &mut CityModel,
    cityobject: cj_cityobject_id_t,
) -> Result<&mut OwnedCityObject, AbiError> {
    model
        .cityobjects_mut()
        .get_mut(cityobject_from_abi(cityobject))
        .ok_or_else(|| invalid_argument("cityobject id is invalid for this model"))
}

fn parse_lod(value: Option<String>) -> Result<Option<LoD>, AbiError> {
    fn parse_one(lod: &str) -> Option<LoD> {
        Some(match lod {
            "0" => LoD::LoD0,
            "0.0" => LoD::LoD0_0,
            "0.1" => LoD::LoD0_1,
            "0.2" => LoD::LoD0_2,
            "0.3" => LoD::LoD0_3,
            "1" => LoD::LoD1,
            "1.0" => LoD::LoD1_0,
            "1.1" => LoD::LoD1_1,
            "1.2" => LoD::LoD1_2,
            "1.3" => LoD::LoD1_3,
            "2" => LoD::LoD2,
            "2.0" => LoD::LoD2_0,
            "2.1" => LoD::LoD2_1,
            "2.2" => LoD::LoD2_2,
            "2.3" => LoD::LoD2_3,
            "3" => LoD::LoD3,
            "3.0" => LoD::LoD3_0,
            "3.1" => LoD::LoD3_1,
            "3.2" => LoD::LoD3_2,
            "3.3" => LoD::LoD3_3,
            _ => return None,
        })
    }

    value
        .map(|lod| {
            parse_one(&lod).ok_or_else(|| invalid_argument(format!("invalid lod value '{lod}'")))
        })
        .transpose()
}

fn take_value_handle(handle: *mut cj_value_t) -> Result<OwnedValue, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let value =
        unsafe { value_take(handle) }.ok_or_else(|| invalid_argument("value must not be null"))?;
    Ok(*value)
}

fn take_contact_handle(handle: *mut cj_contact_t) -> Result<OwnedContact, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let contact = unsafe { contact_take(handle) }
        .ok_or_else(|| invalid_argument("contact must not be null"))?;
    Ok(*contact)
}

fn take_cityobject_draft_handle(
    handle: *mut cj_cityobject_draft_t,
) -> Result<OwnedCityObject, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let draft = unsafe { cityobject_draft_take(handle) }
        .ok_or_else(|| invalid_argument("draft must not be null"))?;
    Ok(*draft)
}

fn take_ring_draft_handle(handle: *mut cj_ring_draft_t) -> Result<RingAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let ring = unsafe { ring_draft_take(handle) }
        .ok_or_else(|| invalid_argument("ring must not be null"))?;
    Ok(*ring)
}

fn take_surface_draft_handle(
    handle: *mut cj_surface_draft_t,
) -> Result<SurfaceAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let surface = unsafe { surface_draft_take(handle) }
        .ok_or_else(|| invalid_argument("surface must not be null"))?;
    Ok(*surface)
}

fn take_shell_draft_handle(handle: *mut cj_shell_draft_t) -> Result<ShellAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let shell = unsafe { shell_draft_take(handle) }
        .ok_or_else(|| invalid_argument("shell must not be null"))?;
    Ok(*shell)
}

fn take_solid_draft_handle(handle: *mut cj_solid_draft_t) -> Result<SolidAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let solid = unsafe { solid_draft_take(handle) }
        .ok_or_else(|| invalid_argument("solid must not be null"))?;
    Ok(*solid)
}

fn take_geometry_draft_handle(
    handle: *mut cj_geometry_draft_t,
) -> Result<GeometryAuthoring, AbiError> {
    // SAFETY: null is rejected here; valid handles originate from Rust.
    let draft = unsafe { geometry_draft_take(handle) }
        .ok_or_else(|| invalid_argument("geometry draft must not be null"))?;
    Ok(*draft)
}

fn geometry_type_from_abi(value: cj_geometry_type_t) -> GeometryType {
    match value {
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_POINT => GeometryType::MultiPoint,
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_LINE_STRING => GeometryType::MultiLineString,
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_SURFACE => GeometryType::MultiSurface,
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_COMPOSITE_SURFACE => GeometryType::CompositeSurface,
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_SOLID => GeometryType::Solid,
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_SOLID => GeometryType::MultiSolid,
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_COMPOSITE_SOLID => GeometryType::CompositeSolid,
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_GEOMETRY_INSTANCE => GeometryType::GeometryInstance,
    }
}

fn convert_indices(indices: &[usize], name: &'static str) -> Result<Vec<u32>, AbiError> {
    indices
        .iter()
        .map(|index| {
            u32::try_from(*index)
                .map_err(|_| invalid_argument(format!("{name} index {index} exceeds u32")))
        })
        .collect()
}

fn segment_ranges(
    offsets: &[usize],
    total_len: usize,
    name: &'static str,
) -> Result<Vec<(usize, usize)>, AbiError> {
    if offsets.is_empty() {
        return Ok(if total_len == 0 {
            Vec::new()
        } else {
            vec![(0, total_len)]
        });
    }

    if offsets[0] != 0 {
        return Err(invalid_argument(format!(
            "{name} offsets must start at zero"
        )));
    }

    let mut ranges = Vec::with_capacity(offsets.len());
    for (index, start) in offsets.iter().copied().enumerate() {
        let end = offsets.get(index + 1).copied().unwrap_or(total_len);
        if start > end || end > total_len {
            return Err(invalid_argument(format!(
                "{name} offsets must be monotonically increasing and within bounds"
            )));
        }
        ranges.push((start, end));
    }

    Ok(ranges)
}

fn boundary_from_view(
    view: cj_geometry_boundary_view_t,
) -> Result<Option<Boundary<u32>>, AbiError> {
    let vertices = required_indices_view(view.vertex_indices, "boundary.vertex_indices")?;
    let ring_offsets = required_indices_view(view.ring_offsets, "boundary.ring_offsets")?;
    let surface_offsets = required_indices_view(view.surface_offsets, "boundary.surface_offsets")?;
    let shell_offsets = required_indices_view(view.shell_offsets, "boundary.shell_offsets")?;
    let solid_offsets = required_indices_view(view.solid_offsets, "boundary.solid_offsets")?;

    let vertices = convert_indices(vertices, "boundary.vertex_indices")?;
    let ring_ranges = segment_ranges(ring_offsets, vertices.len(), "boundary.ring")?;
    let rings = ring_ranges
        .iter()
        .map(|(start, end)| vertices[*start..*end].to_vec())
        .collect::<Vec<_>>();

    let boundary = match view.geometry_type {
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_POINT => {
            if !ring_offsets.is_empty()
                || !surface_offsets.is_empty()
                || !shell_offsets.is_empty()
                || !solid_offsets.is_empty()
            {
                return Err(invalid_argument(
                    "MultiPoint boundaries must not provide nested offsets",
                ));
            }
            Some(BoundaryNestedMultiPoint::from(vertices).into())
        }
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_LINE_STRING => {
            if !surface_offsets.is_empty() || !shell_offsets.is_empty() || !solid_offsets.is_empty()
            {
                return Err(invalid_argument(
                    "MultiLineString boundaries must only provide ring offsets",
                ));
            }
            Some(
                Boundary::try_from(BoundaryNestedMultiLineString::from(rings))
                    .map_err(cityjson_lib::Error::from)
                    .map_err(AbiError::from)?,
            )
        }
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_SURFACE
        | cj_geometry_type_t::CJ_GEOMETRY_TYPE_COMPOSITE_SURFACE => {
            if !shell_offsets.is_empty() || !solid_offsets.is_empty() {
                return Err(invalid_argument(
                    "surface boundaries must not provide shell or solid offsets",
                ));
            }
            let surface_ranges = segment_ranges(surface_offsets, rings.len(), "boundary.surface")?;
            let surfaces = surface_ranges
                .iter()
                .map(|(start, end)| rings[*start..*end].to_vec())
                .collect::<Vec<_>>();
            Some(
                Boundary::try_from(BoundaryNestedMultiOrCompositeSurface::from(surfaces))
                    .map_err(cityjson_lib::Error::from)
                    .map_err(AbiError::from)?,
            )
        }
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_SOLID => {
            if !solid_offsets.is_empty() {
                return Err(invalid_argument(
                    "Solid boundaries must not provide solid offsets",
                ));
            }
            let surface_ranges = segment_ranges(surface_offsets, rings.len(), "boundary.surface")?;
            let surfaces = surface_ranges
                .iter()
                .map(|(start, end)| rings[*start..*end].to_vec())
                .collect::<Vec<_>>();
            let shell_ranges = segment_ranges(shell_offsets, surfaces.len(), "boundary.shell")?;
            let shells = shell_ranges
                .iter()
                .map(|(start, end)| surfaces[*start..*end].to_vec())
                .collect::<Vec<_>>();
            Some(
                Boundary::try_from(BoundaryNestedSolid::from(shells))
                    .map_err(cityjson_lib::Error::from)
                    .map_err(AbiError::from)?,
            )
        }
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_MULTI_SOLID
        | cj_geometry_type_t::CJ_GEOMETRY_TYPE_COMPOSITE_SOLID => {
            let surface_ranges = segment_ranges(surface_offsets, rings.len(), "boundary.surface")?;
            let surfaces = surface_ranges
                .iter()
                .map(|(start, end)| rings[*start..*end].to_vec())
                .collect::<Vec<_>>();
            let shell_ranges = segment_ranges(shell_offsets, surfaces.len(), "boundary.shell")?;
            let shells = shell_ranges
                .iter()
                .map(|(start, end)| surfaces[*start..*end].to_vec())
                .collect::<Vec<_>>();
            let solid_ranges = segment_ranges(solid_offsets, shells.len(), "boundary.solid")?;
            let solids = solid_ranges
                .iter()
                .map(|(start, end)| shells[*start..*end].to_vec())
                .collect::<Vec<_>>();
            Some(
                Boundary::try_from(BoundaryNestedMultiOrCompositeSolid::from(solids))
                    .map_err(cityjson_lib::Error::from)
                    .map_err(AbiError::from)?,
            )
        }
        cj_geometry_type_t::CJ_GEOMETRY_TYPE_GEOMETRY_INSTANCE => None,
    };

    Ok(boundary)
}

/// cbindgen:ignore
fn geometry_from_boundary_view(
    view: cj_geometry_boundary_view_t,
    lod: Option<LoD>,
) -> Result<
    Geometry<u32, cityjson_lib::cityjson_types::resources::storage::OwnedStringStorage>,
    AbiError,
> {
    let boundary = boundary_from_view(view)?;
    Ok(Geometry::from_stored_parts(StoredGeometryParts {
        type_geometry: geometry_type_from_abi(view.geometry_type),
        lod,
        boundaries: boundary,
        semantics: None,
        materials: None,
        textures: None,
        instance: None,
    }))
}

fn reject_unsupported_document_version(version: Option<CityJSONVersion>) -> Result<(), AbiError> {
    match version {
        Some(CityJSONVersion::V2_0) => Ok(()),
        Some(found) => Err(AbiError::from(Error::UnsupportedVersion {
            found: found.to_string(),
            supported: CityJSONVersion::V2_0.to_string(),
        })),
        None => Err(AbiError::from(Error::MissingVersion)),
    }
}

fn reject_unsupported_feature_version(version: Option<CityJSONVersion>) -> Result<(), AbiError> {
    match version {
        Some(found) => Err(AbiError::from(Error::UnsupportedVersion {
            found: found.to_string(),
            supported: CityJSONVersion::V2_0.to_string(),
        })),
        None => Ok(()),
    }
}

fn summarize_model(model: &CityModel) -> cj_model_summary_t {
    let extension_count = model.extensions().map_or(0, |extensions| extensions.len());
    let material_count = model.material_count();
    let texture_count = model.texture_count();
    let uv_coordinate_count = model.vertices_texture().len();
    let geometry_template_count = model.geometry_template_count();
    let template_vertex_count = model.template_vertices().len();

    cj_model_summary_t {
        model_type: model.type_citymodel().into(),
        version: if model.version().is_some() {
            crate::abi::cj_version_t::CJ_VERSION_V2_0
        } else {
            crate::abi::cj_version_t::CJ_VERSION_UNKNOWN
        },
        cityobject_count: model.cityobjects().len(),
        geometry_count: model.geometry_count(),
        geometry_template_count,
        vertex_count: model.vertices().len(),
        template_vertex_count,
        uv_coordinate_count,
        semantic_count: model.semantic_count(),
        material_count,
        texture_count,
        extension_count,
        has_metadata: model.metadata().is_some(),
        has_transform: model.transform().is_some(),
        has_templates: geometry_template_count > 0 || template_vertex_count > 0,
        has_appearance: material_count > 0 || texture_count > 0 || uv_coordinate_count > 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_free(handle: *mut cj_model_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            model_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_selection_free(handle: *mut cj_model_selection_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            model_selection_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_bytes_free(bytes: cj_bytes_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if bytes.data.is_null() {
            if bytes.len == 0 {
                return Ok(());
            }

            return Err(invalid_argument(
                "bytes data must not be null when len is non-zero",
            ));
        }

        // SAFETY: the ABI only frees buffers allocated by `bytes_from_vec`.
        unsafe {
            free_bytes(bytes);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_bytes_list_free(bytes: cj_bytes_list_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if bytes.data.is_null() {
            if bytes.len == 0 {
                return Ok(());
            }

            return Err(invalid_argument(
                "bytes list data must not be null when len is non-zero",
            ));
        }

        // SAFETY: the ABI only frees lists allocated by `bytes_list_from_vec`.
        unsafe {
            free_bytes_list(bytes);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_vertices_free(vertices: cj_vertices_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if vertices.data.is_null() {
            if vertices.len == 0 {
                return Ok(());
            }

            return Err(invalid_argument(
                "vertices data must not be null when len is non-zero",
            ));
        }

        // SAFETY: the ABI only frees buffers allocated by `vertices_from_vec`.
        unsafe {
            free_vertices(vertices);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_types_free(types: cj_geometry_types_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if types.data.is_null() {
            if types.len == 0 {
                return Ok(());
            }

            return Err(invalid_argument(
                "geometry types data must not be null when len is non-zero",
            ));
        }

        // SAFETY: the ABI only frees lists allocated by `geometry_types_from_vec`.
        unsafe {
            free_geometry_types(types);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_uvs_free(uvs: cj_uvs_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if uvs.data.is_null() {
            if uvs.len == 0 {
                return Ok(());
            }

            return Err(invalid_argument(
                "uvs data must not be null when len is non-zero",
            ));
        }

        // SAFETY: the ABI only frees buffers allocated by `uvs_from_vec`.
        unsafe {
            free_uvs(uvs);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_indices_free(indices: cj_indices_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if indices.data.is_null() {
            if indices.len == 0 {
                return Ok(());
            }

            return Err(invalid_argument(
                "indices data must not be null when len is non-zero",
            ));
        }

        // SAFETY: the ABI only frees buffers allocated by `indices_from_vec`.
        unsafe {
            free_indices(indices);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_boundary_free(boundary: cj_geometry_boundary_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        // SAFETY: the ABI only frees boundary payloads allocated by `boundary_from_geometry`.
        unsafe {
            free_geometry_boundary(boundary);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_free(handle: *mut cj_value_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            value_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_free(handle: *mut cj_contact_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            contact_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_cityobject_draft_free(handle: *mut cj_cityobject_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            cityobject_draft_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_ring_draft_free(handle: *mut cj_ring_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            ring_draft_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_surface_draft_free(handle: *mut cj_surface_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            surface_draft_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_shell_draft_free(handle: *mut cj_shell_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            shell_draft_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_solid_draft_free(handle: *mut cj_solid_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            solid_draft_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_draft_free(handle: *mut cj_geometry_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if handle.is_null() {
            return Ok(());
        }

        // SAFETY: the ABI only frees handles that it allocated.
        unsafe {
            geometry_draft_free(handle);
        }

        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_last_error_kind() -> cj_error_kind_t {
    last_error_kind()
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_last_error_message_len() -> usize {
    last_error_message_len()
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_last_error_message_copy(
    buffer: *mut u8,
    capacity: usize,
    out_len: *mut usize,
) -> cj_status_t {
    // SAFETY: this helper validates the out-pointer and buffer/capacity pairing.
    unsafe { copy_last_error_message(buffer.cast::<c_char>(), capacity, out_len) }
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_clear_error() -> cj_status_t {
    clear_last_error();
    cj_status_t::CJ_STATUS_SUCCESS
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_probe_bytes(
    data: *const u8,
    len: usize,
    out_probe: *mut cj_probe_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let input = required_bytes(data, len, "data")?;
        let probe = cityjson_lib::json::probe(input)?;
        write_value(out_probe, "out_probe", cj_probe_t::from_probe(&probe))
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_parse_document_bytes(
    data: *const u8,
    len: usize,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let input = required_bytes(data, len, "data")?;
        let probe = cityjson_lib::json::probe(input)?;
        if probe.kind() != RootKind::CityJSON {
            return Err(AbiError::from(Error::ExpectedCityJSON(
                probe.kind().to_string(),
            )));
        }

        reject_unsupported_document_version(probe.version())?;
        let model = cityjson_lib::json::from_slice_assume_cityjson_v2_0(input)?;
        write_model_handle(out_model, model)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_parse_feature_bytes(
    data: *const u8,
    len: usize,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let input = required_bytes(data, len, "data")?;
        let probe = cityjson_lib::json::probe(input)?;
        if probe.kind() != RootKind::CityJSONFeature {
            return Err(AbiError::from(Error::ExpectedCityJSONFeature(
                probe.kind().to_string(),
            )));
        }

        reject_unsupported_feature_version(probe.version())?;
        let model = cityjson_lib::json::from_feature_slice_assume_cityjson_feature_v2_0(input)?;
        write_model_handle(out_model, model)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_parse_feature_with_base_bytes(
    feature_data: *const u8,
    feature_len: usize,
    base_data: *const u8,
    base_len: usize,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let feature = required_bytes(feature_data, feature_len, "feature_data")?;
        let base = required_bytes(base_data, base_len, "base_data")?;

        let feature_probe = cityjson_lib::json::probe(feature)?;
        if feature_probe.kind() != RootKind::CityJSONFeature {
            return Err(AbiError::from(Error::ExpectedCityJSONFeature(
                feature_probe.kind().to_string(),
            )));
        }

        reject_unsupported_feature_version(feature_probe.version())?;

        let base_probe = cityjson_lib::json::probe(base)?;
        if base_probe.kind() != RootKind::CityJSON {
            return Err(AbiError::from(Error::ExpectedCityJSON(
                base_probe.kind().to_string(),
            )));
        }

        reject_unsupported_document_version(base_probe.version())?;
        let model =
            cityjson_lib::json::staged::from_feature_slice_with_base_assume_cityjson_feature_v2_0(
                feature, base,
            )?;
        write_model_handle(out_model, model)
    }))
}

#[cfg(feature = "native-formats")]
#[unsafe(no_mangle)]
pub extern "C" fn cj_model_parse_arrow_bytes(
    data: *const u8,
    len: usize,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let input = required_bytes(data, len, "data")?;
        let model = cityjson_lib::arrow::from_reader(Cursor::new(input))?;
        write_model_handle(out_model, model)
    }))
}

#[cfg(feature = "native-formats")]
#[unsafe(no_mangle)]
pub extern "C" fn cj_model_parse_parquet_file(
    path: cj_string_view_t,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let path = view_utf8(path, "path")?;
        let model = cityjson_lib::parquet::from_file(path)?;
        write_model_handle(out_model, model)
    }))
}

#[cfg(feature = "native-formats")]
#[unsafe(no_mangle)]
pub extern "C" fn cj_model_parse_parquet_dataset_dir(
    path: cj_string_view_t,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let path = view_utf8(path, "path")?;
        let model = cityjson_lib::parquet::from_dir(path)?;
        write_model_handle(out_model, model)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_document(
    model: *const cj_model_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let bytes = cityjson_lib::json::to_vec(model)?;
        write_bytes(out_bytes, bytes)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_feature(
    model: *const cj_model_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let bytes = cityjson_lib::json::to_feature_vec_with_options(
            model,
            cityjson_lib::json::WriteOptions::default(),
        )?;
        write_bytes(out_bytes, bytes)
    }))
}

#[cfg(feature = "native-formats")]
#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_arrow_bytes(
    model: *const cj_model_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let bytes = cityjson_lib::arrow::to_vec(model)?;
        write_bytes(out_bytes, bytes)
    }))
}

#[cfg(feature = "native-formats")]
#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_parquet_file(
    model: *const cj_model_t,
    path: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let path = view_utf8(path, "path")?;
        let _ = cityjson_lib::parquet::to_file(path, model)?;
        Ok(())
    }))
}

#[cfg(feature = "native-formats")]
#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_parquet_dataset_dir(
    model: *const cj_model_t,
    path: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let path = view_utf8(path, "path")?;
        let _ = cityjson_lib::parquet::to_dir(path, model)?;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_get_summary(
    model: *const cj_model_t,
    out_summary: *mut cj_model_summary_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        write_value(out_summary, "out_summary", summarize_model(model))
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_get_metadata_title(
    model: *const cj_model_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let bytes = copy_string_bytes(model.metadata().and_then(|metadata| metadata.title()));
        write_bytes(out_bytes, bytes)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_get_metadata_identifier(
    model: *const cj_model_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let bytes = copy_string_bytes(
            model
                .metadata()
                .and_then(|metadata| metadata.identifier())
                .map(|identifier| identifier.to_string())
                .as_deref(),
        );
        write_bytes(out_bytes, bytes)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_get_cityobject_id(
    model: *const cj_model_t,
    index: usize,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let cityobject = model
            .cityobjects()
            .iter()
            .nth(index)
            .map(|(_, cityobject)| cityobject)
            .ok_or_else(|| invalid_argument(format!("cityobject index {index} is out of range")))?;
        write_bytes(out_bytes, cityobject.id().as_bytes().to_vec())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_copy_cityobject_ids(
    model: *const cj_model_t,
    out_ids: *mut cj_bytes_list_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        write_value(
            out_ids,
            "out_ids",
            bytes_list_from_vec(cityobject_ids_from_model(model)),
        )
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_get_geometry_type(
    model: *const cj_model_t,
    index: usize,
    out_type: *mut cj_geometry_type_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let geometry_type = model
            .iter_geometries()
            .nth(index)
            .map(|(_, geometry)| *geometry.type_geometry())
            .ok_or_else(|| invalid_argument(format!("geometry index {index} is out of range")))?;
        write_value(out_type, "out_type", geometry_type.into())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_copy_geometry_types(
    model: *const cj_model_t,
    out_types: *mut cj_geometry_types_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        write_value(
            out_types,
            "out_types",
            geometry_types_from_vec(geometry_types_from_model(model)),
        )
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_copy_geometry_boundary(
    model: *const cj_model_t,
    index: usize,
    out_boundary: *mut cj_geometry_boundary_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let geometry = geometry_at(model, index)?;
        write_boundary(out_boundary, boundary_from_geometry(geometry))
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_copy_geometry_boundary_coordinates(
    model: *const cj_model_t,
    index: usize,
    out_vertices: *mut cj_vertices_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let vertices = geometry_boundary_coordinates(model, index)?;
        write_vertices(out_vertices, vertices)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_copy_vertices(
    model: *const cj_model_t,
    out_vertices: *mut cj_vertices_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let vertices = model
            .vertices()
            .as_slice()
            .iter()
            .copied()
            .map(Into::into)
            .collect();
        write_vertices(out_vertices, vertices)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_copy_template_vertices(
    model: *const cj_model_t,
    out_vertices: *mut cj_vertices_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let vertices = model
            .template_vertices()
            .as_slice()
            .iter()
            .copied()
            .map(Into::into)
            .collect();
        write_vertices(out_vertices, vertices)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_copy_uv_coordinates(
    model: *const cj_model_t,
    out_uvs: *mut cj_uvs_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = required_model_ref(model)?;
        let uvs = model
            .vertices_texture()
            .as_slice()
            .iter()
            .cloned()
            .map(Into::into)
            .collect();
        write_uvs(out_uvs, uvs)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_create(
    model_type: cj_model_type_t,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model = CityModel::new(CityModelType::from(model_type));
        write_model_handle(out_model, model)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_reserve_import(
    model: *mut cj_model_t,
    capacities: cj_model_capacities_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .reserve_import(capacities.into())
            .map_err(cityjson_lib::Error::from)?;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_vertex(
    model: *mut cj_model_t,
    vertex: cj_vertex_t,
    out_index: *mut usize,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let index = required_model_mut(model)?
            .add_vertex(vertex.into())
            .map_err(cityjson_lib::Error::from)?
            .to_usize();
        write_value(out_index, "out_index", index)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_template_vertex(
    model: *mut cj_model_t,
    vertex: cj_vertex_t,
    out_index: *mut usize,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let index = required_model_mut(model)?
            .add_template_vertex(vertex.into())
            .map_err(cityjson_lib::Error::from)?
            .to_usize();
        write_value(out_index, "out_index", index)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_vertex(
    model: *mut cj_model_t,
    index: usize,
    vertex: cj_vertex_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let vertices = required_model_mut(model)?.vertices_mut().as_mut_slice();
        let slot = vertices
            .get_mut(index)
            .ok_or_else(|| invalid_argument(format!("vertex index {index} is out of range")))?;
        *slot = vertex.into();
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_template_vertex(
    model: *mut cj_model_t,
    index: usize,
    vertex: cj_vertex_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let vertices = required_model_mut(model)?
            .template_vertices_mut()
            .as_mut_slice();
        let slot = vertices.get_mut(index).ok_or_else(|| {
            invalid_argument(format!("template vertex index {index} is out of range"))
        })?;
        *slot = vertex.into();
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_uv_coordinate(
    model: *mut cj_model_t,
    uv: cj_uv_t,
    out_index: *mut usize,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let index = required_model_mut(model)?
            .add_uv_coordinate(uv.into())
            .map_err(cityjson_lib::Error::from)?
            .to_usize();
        write_value(out_index, "out_index", index)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_metadata_title(
    model: *mut cj_model_t,
    title: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let title = view_utf8(title, "title")?;
        let metadata = required_model_mut(model)?.metadata_mut();
        metadata.set_title(title);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_metadata_identifier(
    model: *mut cj_model_t,
    identifier: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let identifier = view_utf8(identifier, "identifier")?;
        let metadata = required_model_mut(model)?.metadata_mut();
        metadata.set_identifier(CityModelIdentifier::new(identifier));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_transform(
    model: *mut cj_model_t,
    transform: cj_transform_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let transform = transform_from_abi(transform);
        let transform_mut = required_model_mut(model)?.transform_mut();
        transform_mut.set_scale(transform.scale());
        transform_mut.set_translate(transform.translate());
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_clear_transform(model: *mut cj_model_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let model_ref = required_model_ref(model)?;
        let bytes = cityjson_lib::json::to_vec_with_options(
            model_ref,
            cityjson_lib::json::WriteOptions {
                pretty: false,
                validate_default_themes: false,
            },
        )?;
        let mut root = match serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|error| AbiError::from(Error::Syntax(error.to_string())))?
        {
            serde_json::Value::Object(root) => root,
            _ => {
                return Err(AbiError::from(Error::Import(
                    "serialized CityJSON root is not an object".into(),
                )));
            }
        };
        root.remove("transform");
        let bytes = serde_json::to_vec(&serde_json::Value::Object(root))
            .map_err(|error| AbiError::from(Error::Syntax(error.to_string())))?;
        let replacement = match model_ref.type_citymodel() {
            CityModelType::CityJSON => cityjson_lib::json::from_slice(&bytes)?,
            CityModelType::CityJSONFeature => cityjson_lib::json::from_feature_slice(&bytes)?,
            other => return Err(AbiError::from(Error::UnsupportedType(other.to_string()))),
        };
        *required_model_mut(model)? = replacement;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_reproject(
    model: *mut cj_model_t,
    target_crs: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let target_crs = view_utf8(target_crs, "target_crs")?;
        #[cfg(feature = "proj")]
        {
            let model_mut = required_model_mut(model)?;
            let reprojected = cityjson_lib::ops::reproject(model_mut.clone(), &target_crs)?;
            *model_mut = reprojected;
            Ok(())
        }
        #[cfg(not(feature = "proj"))]
        {
            let _ = model;
            let _ = target_crs;
            Err(unsupported(
                "PROJ support is not enabled; rebuild cityjson-lib-ffi-core with the proj feature",
            ))
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_remove_cityobject(
    model: *mut cj_model_t,
    id: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let id = view_utf8(id, "id")?;
        let cityobjects = required_model_mut(model)?.cityobjects_mut();
        let Some(handle) = cityobjects
            .iter()
            .find_map(|(handle, cityobject)| (cityobject.id() == id).then_some(handle))
        else {
            return Err(invalid_argument(format!("CityObject '{id}' was not found")));
        };
        cityobjects.remove(handle);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_attach_geometry_to_cityobject(
    model: *mut cj_model_t,
    cityobject_id: cj_string_view_t,
    geometry_index: usize,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let cityobject_id = view_utf8(cityobject_id, "cityobject_id")?;
        let geometry_handle = find_geometry_handle(required_model_ref(model)?, geometry_index)?;
        find_cityobject_mut(required_model_mut(model)?, &cityobject_id)?
            .add_geometry(geometry_handle);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_clear_cityobject_geometry(
    model: *mut cj_model_t,
    cityobject_id: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let cityobject_id = view_utf8(cityobject_id, "cityobject_id")?;
        find_cityobject_mut(required_model_mut(model)?, &cityobject_id)?.clear_geometry();
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_geometry_from_boundary(
    model: *mut cj_model_t,
    boundary: cj_geometry_boundary_view_t,
    lod: cj_string_view_t,
    out_index: *mut usize,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let lod = parse_lod(optional_view_utf8(lod, "lod")?)?;
        let geometry = geometry_from_boundary_view(boundary, lod)?;
        let model = required_model_mut(model)?;
        let index = model.geometry_count();
        model
            .add_geometry(geometry)
            .map_err(cityjson_lib::Error::from)?;
        write_value(out_index, "out_index", index)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_cleanup(model: *mut cj_model_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let cleaned = cityjson_lib::ops::cleanup(required_model_ref(model)?)?;
        *required_model_mut(model)? = cleaned;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_append_model(
    target_model: *mut cj_model_t,
    source_model: *const cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if target_model.is_null() {
            return Err(invalid_argument("target_model must not be null"));
        }

        if source_model.is_null() {
            return Err(invalid_argument("source_model must not be null"));
        }

        if ptr::eq(target_model.cast_const(), source_model) {
            return Err(invalid_argument(
                "target_model and source_model must not alias",
            ));
        }

        let source = required_model_ref(source_model)?;
        cityjson_lib::ops::append(required_model_mut(target_model)?, source)?;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_subset_cityobjects(
    model: *const cj_model_t,
    cityobject_ids: *const cj_string_view_t,
    cityobject_count: usize,
    exclude: bool,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let views = required_string_views(cityobject_ids, cityobject_count, "cityobject_ids")?;
        let ids = views
            .iter()
            .map(|view| view_utf8(*view, "cityobject_ids[]"))
            .collect::<Result<Vec<_>, _>>()?;
        let borrowed = ids.iter().map(String::as_str).collect::<Vec<_>>();
        let subset = cityjson_lib::ops::subset(required_model_ref(model)?, borrowed, exclude)?;
        write_model_handle(out_model, subset)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_select_cityobjects_by_id(
    model: *const cj_model_t,
    cityobject_ids: *const cj_string_view_t,
    cityobject_count: usize,
    out_selection: *mut *mut cj_model_selection_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let views = required_string_views(cityobject_ids, cityobject_count, "cityobject_ids")?;
        let ids = views
            .iter()
            .map(|view| view_utf8(*view, "cityobject_ids[]"))
            .collect::<Result<HashSet<_>, _>>()?;
        let selection = cityjson_lib::ops::select_cityobjects(required_model_ref(model)?, |ctx| {
            ids.contains(ctx.id())
        })?;
        write_model_selection_handle(out_selection, selection)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_select_geometries_by_cityobject_id_and_index(
    model: *const cj_model_t,
    specs: *const cj_geometry_selection_spec_t,
    spec_count: usize,
    out_selection: *mut *mut cj_model_selection_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let specs = required_geometry_selection_specs(specs, spec_count, "specs")?;
        let pairs = specs
            .iter()
            .map(|spec| {
                Ok((
                    view_utf8(spec.cityobject_id, "specs[].cityobject_id")?,
                    spec.geometry_index,
                ))
            })
            .collect::<Result<HashSet<_>, AbiError>>()?;
        let selection = cityjson_lib::ops::select_geometries(required_model_ref(model)?, |ctx| {
            pairs.contains(&(ctx.cityobject_id().to_owned(), ctx.geometry_index()))
        })?;
        write_model_selection_handle(out_selection, selection)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_selection_include_relatives(
    selection: *const cj_model_selection_t,
    model: *const cj_model_t,
    out_selection: *mut *mut cj_model_selection_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let selection = required_model_selection_ref(selection)?
            .clone()
            .include_relatives(required_model_ref(model)?)?;
        write_model_selection_handle(out_selection, selection)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_selection_union(
    lhs: *const cj_model_selection_t,
    rhs: *const cj_model_selection_t,
    out_selection: *mut *mut cj_model_selection_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let selection =
            required_model_selection_ref(lhs)?.union(required_model_selection_ref(rhs)?);
        write_model_selection_handle(out_selection, selection)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_selection_intersection(
    lhs: *const cj_model_selection_t,
    rhs: *const cj_model_selection_t,
    out_selection: *mut *mut cj_model_selection_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let selection =
            required_model_selection_ref(lhs)?.intersection(required_model_selection_ref(rhs)?);
        write_model_selection_handle(out_selection, selection)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_selection_is_empty(
    selection: *const cj_model_selection_t,
    out_bool: *mut bool,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        write_value(
            out_bool,
            "out_bool",
            required_model_selection_ref(selection)?.is_empty(),
        )
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_extract_selection(
    model: *const cj_model_t,
    selection: *const cj_model_selection_t,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let extracted = cityjson_lib::ops::extract(
            required_model_ref(model)?,
            required_model_selection_ref(selection)?,
        )?;
        write_model_handle(out_model, extracted)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_merge_models(
    models: *const *const cj_model_t,
    model_count: usize,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if model_count == 0 {
            return Err(invalid_argument("models must contain at least one handle"));
        }

        let models_ptr = NonNull::new(models.cast_mut()).ok_or_else(|| {
            invalid_argument("models must not be null when model_count is non-zero")
        })?;
        let models =
            unsafe { slice::from_raw_parts(models_ptr.as_ptr().cast_const(), model_count) };
        let owned = models
            .iter()
            .map(|handle| required_model_ref(*handle).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let merged = cityjson_lib::ops::merge(owned)?;
        write_model_handle(out_model, merged)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_document_with_options(
    model: *const cj_model_t,
    options: cj_json_write_options_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let bytes = cityjson_lib::json::to_vec_with_options(
            required_model_ref(model)?,
            cityjson_lib::json::WriteOptions {
                pretty: options.pretty,
                validate_default_themes: options.validate_default_themes,
            },
        )?;
        write_bytes(out_bytes, bytes)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_feature_with_options(
    model: *const cj_model_t,
    options: cj_json_write_options_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let bytes = cityjson_lib::json::to_feature_vec_with_options(
            required_model_ref(model)?,
            cityjson_lib::json::WriteOptions {
                pretty: options.pretty,
                validate_default_themes: options.validate_default_themes,
            },
        )?;
        write_bytes(out_bytes, bytes)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_parse_feature_stream_merge_bytes(
    data: *const u8,
    len: usize,
    out_model: *mut *mut cj_model_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let input = required_bytes(data, len, "data")?;
        let model = cityjson_lib::json::merge_feature_stream_slice(input)?;
        write_model_handle(out_model, model)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_feature_stream(
    models: *const *const cj_model_t,
    model_count: usize,
    options: cj_json_write_options_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        if options.pretty {
            return Err(AbiError::from(Error::UnsupportedFeature(
                "pretty output is not supported for JSONL feature streams".into(),
            )));
        }

        if model_count == 0 {
            return write_bytes(out_bytes, Vec::new());
        }

        let models_ptr = NonNull::new(models.cast_mut()).ok_or_else(|| {
            invalid_argument("models must not be null when model_count is non-zero")
        })?;
        let models =
            unsafe { slice::from_raw_parts(models_ptr.as_ptr().cast_const(), model_count) };
        let refs = models
            .iter()
            .map(|handle| required_model_ref(*handle))
            .collect::<Result<Vec<_>, _>>()?;

        if options.validate_default_themes {
            for model in &refs {
                model
                    .validate_default_themes()
                    .map_err(cityjson_lib::Error::from)
                    .map_err(AbiError::from)?;
            }
        }

        let mut buffer = Vec::new();
        for (index, model) in refs.iter().enumerate() {
            match model.type_citymodel() {
                CityModelType::CityJSON => {
                    if index != 0 {
                        return Err(AbiError::from(Error::UnsupportedFeature(
                            "only the first feature-stream item may be CityJSON".into(),
                        )));
                    }
                    cityjson_lib::json::to_writer_with_options(
                        &mut buffer,
                        model,
                        cityjson_lib::json::WriteOptions {
                            pretty: false,
                            validate_default_themes: options.validate_default_themes,
                        },
                    )?;
                }
                CityModelType::CityJSONFeature => {
                    cityjson_lib::json::to_feature_writer(&mut buffer, model)?;
                }
                other => return Err(AbiError::from(Error::UnsupportedType(other.to_string()))),
            }
            buffer.push(b'\n');
        }
        write_bytes(out_bytes, buffer)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_cityjsonseq_with_transform(
    base_root: *const cj_model_t,
    features: *const *const cj_model_t,
    feature_count: usize,
    transform: cj_transform_t,
    options: cj_cityjsonseq_write_options_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let base_root = required_model_ref(base_root)?;
        let feature_refs = required_model_refs(features, feature_count, "features")?;
        let transform = transform_from_abi(transform);
        let _ = options;

        let mut buffer = Vec::new();
        cityjson_lib::json::write_cityjsonseq_refs(
            &mut buffer,
            base_root,
            feature_refs,
            &transform,
        )?;
        write_bytes(out_bytes, buffer)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_serialize_cityjsonseq_auto_transform(
    base_root: *const cj_model_t,
    features: *const *const cj_model_t,
    feature_count: usize,
    options: cj_cityjsonseq_auto_transform_options_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let base_root = required_model_ref(base_root)?;
        let feature_refs = required_model_refs(features, feature_count, "features")?;
        let _ = (
            options.validate_default_themes,
            options.trailing_newline,
            options.update_metadata_geographical_extent,
        );

        let mut buffer = Vec::new();
        cityjson_lib::json::write_cityjsonseq_auto_transform_refs(
            &mut buffer,
            base_root,
            feature_refs,
            [options.scale_x, options.scale_y, options.scale_z],
        )?;
        write_bytes(out_bytes, buffer)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_null(out_value: *mut *mut cj_value_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), value_into_handle(OwnedValue::Null));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_bool(value: bool, out_value: *mut *mut cj_value_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), value_into_handle(OwnedValue::Bool(value)));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_int64(value: i64, out_value: *mut *mut cj_value_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), value_into_handle(OwnedValue::Integer(value)));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_float64(value: f64, out_value: *mut *mut cj_value_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), value_into_handle(OwnedValue::Float(value)));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_string(
    value: cj_string_view_t,
    out_value: *mut *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        let value = view_utf8(value, "value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), value_into_handle(OwnedValue::String(value)));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_array(out_value: *mut *mut cj_value_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), value_into_handle(OwnedValue::Vec(Vec::new())));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_object(out_value: *mut *mut cj_value_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(
                out.as_ptr(),
                value_into_handle(OwnedValue::Map(std::collections::HashMap::new())),
            );
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_new_geometry_ref(
    value: cj_geometry_id_t,
    out_value: *mut *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_value, "out_value")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(
                out.as_ptr(),
                value_into_handle(OwnedValue::Geometry(geometry_from_abi(value))),
            );
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_array_push(
    array_value: *mut cj_value_t,
    element: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let element = take_value_handle(element)?;
        match required_value_mut(array_value)? {
            OwnedValue::Vec(values) => {
                values.push(element);
                Ok(())
            }
            _ => Err(invalid_argument("array_value must be an array")),
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_value_object_insert(
    object_value: *mut cj_value_t,
    key: cj_string_view_t,
    member_value: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let key = view_utf8(key, "key")?;
        let member_value = take_value_handle(member_value)?;
        match required_value_mut(object_value)? {
            OwnedValue::Map(values) => {
                values.insert(key, member_value);
                Ok(())
            }
            _ => Err(invalid_argument("object_value must be an object")),
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_new(out_contact: *mut *mut cj_contact_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_contact, "out_contact")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), contact_into_handle(Contact::new()));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_name(
    contact: *mut cj_contact_t,
    value: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_contact_mut(contact)?.set_contact_name(view_utf8(value, "value")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_email(
    contact: *mut cj_contact_t,
    value: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_contact_mut(contact)?.set_email_address(view_utf8(value, "value")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_role(
    contact: *mut cj_contact_t,
    value: cj_contact_role_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_contact_mut(contact)?.set_role(Some(contact_role_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_website(
    contact: *mut cj_contact_t,
    value: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_contact_mut(contact)?.set_website(Some(view_utf8(value, "value")?));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_type(
    contact: *mut cj_contact_t,
    value: cj_contact_type_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_contact_mut(contact)?.set_contact_type(Some(contact_type_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_phone(
    contact: *mut cj_contact_t,
    value: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_contact_mut(contact)?.set_phone(Some(view_utf8(value, "value")?));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_organization(
    contact: *mut cj_contact_t,
    value: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_contact_mut(contact)?.set_organization(Some(view_utf8(value, "value")?));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_contact_set_address(
    contact: *mut cj_contact_t,
    object_value: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let object_value = take_value_handle(object_value)?;
        match object_value {
            OwnedValue::Map(values) => {
                required_contact_mut(contact)?.set_address(Some(values.into()));
                Ok(())
            }
            _ => Err(invalid_argument("object_value must be an object")),
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_metadata_geographical_extent(
    model: *mut cj_model_t,
    bbox: cj_bbox_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .metadata_mut()
            .set_geographical_extent(bbox_from_abi(bbox));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_metadata_reference_date(
    model: *mut cj_model_t,
    value: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .metadata_mut()
            .set_reference_date(cityjson_lib::cityjson_types::v2_0::Date::new(view_utf8(
                value, "value",
            )?));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_metadata_reference_system(
    model: *mut cj_model_t,
    value: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .metadata_mut()
            .set_reference_system(cityjson_lib::cityjson_types::v2_0::CRS::new(view_utf8(
                value, "value",
            )?));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_metadata_contact(
    model: *mut cj_model_t,
    contact: *mut cj_contact_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .metadata_mut()
            .set_point_of_contact(Some(take_contact_handle(contact)?));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_metadata_extra(
    model: *mut cj_model_t,
    key: cj_string_view_t,
    value: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .metadata_mut()
            .extra_mut()
            .insert(view_utf8(key, "key")?, take_value_handle(value)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_root_extra(
    model: *mut cj_model_t,
    key: cj_string_view_t,
    value: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .extra_mut()
            .insert(view_utf8(key, "key")?, take_value_handle(value)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_extension(
    model: *mut cj_model_t,
    name: cj_string_view_t,
    url: cj_string_view_t,
    version: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?
            .extensions_mut()
            .add(Extension::new(
                view_utf8(name, "name")?,
                view_utf8(url, "url")?,
                view_utf8(version, "version")?,
            ));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_semantic(
    model: *mut cj_model_t,
    semantic_type: cj_string_view_t,
    out_id: *mut cj_semantic_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let handle = required_model_mut(model)?
            .add_semantic(Semantic::new(semantic_type_from_string(view_utf8(
                semantic_type,
                "semantic_type",
            )?)))
            .map_err(AbiError::from)?;
        write_value(out_id, "out_id", handle.into())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_semantic_parent(
    model: *mut cj_model_t,
    semantic: cj_semantic_id_t,
    parent: cj_semantic_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let semantic_handle = semantic_from_abi(semantic);
        let parent_handle = semantic_from_abi(parent);
        let model = required_model_mut(model)?;
        {
            let semantic_mut = model
                .get_semantic_mut(semantic_handle)
                .ok_or_else(|| invalid_argument("semantic id is invalid for this model"))?;
            semantic_mut.set_parent(parent_handle);
        }
        {
            let parent_mut = model
                .get_semantic_mut(parent_handle)
                .ok_or_else(|| invalid_argument("parent id is invalid for this model"))?;
            if !parent_mut
                .children()
                .is_some_and(|children| children.contains(&semantic_handle))
            {
                parent_mut.children_mut().push(semantic_handle);
            }
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_semantic_set_extra(
    model: *mut cj_model_t,
    semantic: cj_semantic_id_t,
    key: cj_string_view_t,
    value: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_semantic_mut(required_model_mut(model)?, semantic)?
            .attributes_mut()
            .insert(view_utf8(key, "key")?, take_value_handle(value)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_material(
    model: *mut cj_model_t,
    name: cj_string_view_t,
    out_id: *mut cj_material_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let handle = required_model_mut(model)?
            .add_material(Material::new(view_utf8(name, "name")?))
            .map_err(AbiError::from)?;
        write_value(out_id, "out_id", handle.into())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_material_set_ambient_intensity(
    model: *mut cj_model_t,
    material: cj_material_id_t,
    value: f32,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_material_mut(required_model_mut(model)?, material)?
            .set_ambient_intensity(Some(value));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_material_set_diffuse_color(
    model: *mut cj_model_t,
    material: cj_material_id_t,
    value: cj_rgb_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_material_mut(required_model_mut(model)?, material)?
            .set_diffuse_color(Some(rgb_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_material_set_emissive_color(
    model: *mut cj_model_t,
    material: cj_material_id_t,
    value: cj_rgb_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_material_mut(required_model_mut(model)?, material)?
            .set_emissive_color(Some(rgb_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_material_set_specular_color(
    model: *mut cj_model_t,
    material: cj_material_id_t,
    value: cj_rgb_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_material_mut(required_model_mut(model)?, material)?
            .set_specular_color(Some(rgb_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_material_set_shininess(
    model: *mut cj_model_t,
    material: cj_material_id_t,
    value: f32,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_material_mut(required_model_mut(model)?, material)?.set_shininess(Some(value));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_material_set_transparency(
    model: *mut cj_model_t,
    material: cj_material_id_t,
    value: f32,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_material_mut(required_model_mut(model)?, material)?.set_transparency(Some(value));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_material_set_is_smooth(
    model: *mut cj_model_t,
    material: cj_material_id_t,
    value: bool,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_material_mut(required_model_mut(model)?, material)?.set_is_smooth(Some(value));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_texture(
    model: *mut cj_model_t,
    image: cj_string_view_t,
    image_type: cj_image_type_t,
    out_id: *mut cj_texture_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let handle = required_model_mut(model)?
            .add_texture(Texture::new(
                view_utf8(image, "image")?,
                image_type_from_abi(image_type),
            ))
            .map_err(AbiError::from)?;
        write_value(out_id, "out_id", handle.into())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_texture_set_wrap_mode(
    model: *mut cj_model_t,
    texture: cj_texture_id_t,
    value: cj_wrap_mode_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_texture_mut(required_model_mut(model)?, texture)?
            .set_wrap_mode(Some(wrap_mode_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_texture_set_texture_type(
    model: *mut cj_model_t,
    texture: cj_texture_id_t,
    value: cj_texture_type_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_texture_mut(required_model_mut(model)?, texture)?
            .set_texture_type(Some(texture_type_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_texture_set_border_color(
    model: *mut cj_model_t,
    texture: cj_texture_id_t,
    value: cj_rgba_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_texture_mut(required_model_mut(model)?, texture)?
            .set_border_color(Some(rgba_from_abi(value)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_default_material_theme(
    model: *mut cj_model_t,
    theme: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?.set_default_material_theme(Some(
            cityjson_lib::cityjson_types::v2_0::ThemeName::new(view_utf8(theme, "theme")?),
        ));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_set_default_texture_theme(
    model: *mut cj_model_t,
    theme: cj_string_view_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_model_mut(model)?.set_default_texture_theme(Some(
            cityjson_lib::cityjson_types::v2_0::ThemeName::new(view_utf8(theme, "theme")?),
        ));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_cityobject_draft_new(
    id: cj_string_view_t,
    cityobject_type: cj_string_view_t,
    out_draft: *mut *mut cj_cityobject_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let id = view_utf8(id, "id")?;
        let cityobject_type = view_utf8(cityobject_type, "cityobject_type")?;
        let cityobject_type =
            CityObjectType::from_str(&cityobject_type).map_err(cityjson_lib::Error::from)?;
        let draft = CityObject::new(CityObjectIdentifier::new(id), cityobject_type);
        let out = required_out(out_draft, "out_draft")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), cityobject_draft_into_handle(draft));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_cityobject_draft_set_geographical_extent(
    draft: *mut cj_cityobject_draft_t,
    bbox: cj_bbox_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_cityobject_draft_mut(draft)?.set_geographical_extent(Some(bbox_from_abi(bbox)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_cityobject_draft_set_attribute(
    draft: *mut cj_cityobject_draft_t,
    key: cj_string_view_t,
    value: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_cityobject_draft_mut(draft)?
            .attributes_mut()
            .insert(view_utf8(key, "key")?, take_value_handle(value)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_cityobject_draft_set_extra(
    draft: *mut cj_cityobject_draft_t,
    key: cj_string_view_t,
    value: *mut cj_value_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_cityobject_draft_mut(draft)?
            .extra_mut()
            .insert(view_utf8(key, "key")?, take_value_handle(value)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_cityobject(
    model: *mut cj_model_t,
    draft: *mut cj_cityobject_draft_t,
    out_id: *mut cj_cityobject_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let handle = required_model_mut(model)?
            .cityobjects_mut()
            .add(take_cityobject_draft_handle(draft)?)
            .map_err(AbiError::from)?;
        write_value(out_id, "out_id", handle.into())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_cityobject_add_geometry(
    model: *mut cj_model_t,
    cityobject: cj_cityobject_id_t,
    geometry: cj_geometry_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let geometry_handle = geometry_from_abi(geometry);
        let model = required_model_mut(model)?;
        if model.get_geometry(geometry_handle).is_none() {
            return Err(invalid_argument("geometry id is invalid for this model"));
        }
        required_cityobject_by_handle_mut(model, cityobject)?.add_geometry(geometry_handle);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_cityobject_add_parent(
    model: *mut cj_model_t,
    child: cj_cityobject_id_t,
    parent: cj_cityobject_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let child_handle = cityobject_from_abi(child);
        let parent_handle = cityobject_from_abi(parent);
        let model = required_model_mut(model)?;
        {
            let child = model
                .cityobjects_mut()
                .get_mut(child_handle)
                .ok_or_else(|| invalid_argument("child cityobject id is invalid for this model"))?;
            child.add_parent(parent_handle);
        }
        {
            let parent = model
                .cityobjects_mut()
                .get_mut(parent_handle)
                .ok_or_else(|| {
                    invalid_argument("parent cityobject id is invalid for this model")
                })?;
            parent.add_child(child_handle);
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_ring_draft_new(out_ring: *mut *mut cj_ring_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_ring, "out_ring")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(
                out.as_ptr(),
                ring_draft_into_handle(RingAuthoring::default()),
            );
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_ring_draft_push_vertex_index(
    ring: *mut cj_ring_draft_t,
    vertex_index: u32,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_ring_draft_mut(ring)?
            .vertices
            .push(VertexAuthoring::Existing(vertex_index.into()));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_ring_draft_push_vertex(
    ring: *mut cj_ring_draft_t,
    vertex: cj_vertex_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_ring_draft_mut(ring)?
            .vertices
            .push(VertexAuthoring::New(vertex.into()));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_ring_draft_add_texture(
    ring: *mut cj_ring_draft_t,
    theme: cj_string_view_t,
    texture: cj_texture_id_t,
    uv_indices: *const u32,
    uv_index_count: usize,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let theme = view_utf8(theme, "theme")?;
        let uvs = if uv_index_count == 0 {
            Vec::new()
        } else {
            let ptr = NonNull::new(uv_indices.cast_mut()).ok_or_else(|| {
                invalid_argument("uv_indices must not be null when uv_index_count is non-zero")
            })?;
            // SAFETY: the caller promises `uv_index_count` readable indices.
            unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), uv_index_count) }
                .iter()
                .copied()
                .map(|index| UvAuthoring::Existing(index.into()))
                .collect()
        };
        required_ring_draft_mut(ring)?
            .textures
            .push(RingTextureAuthoring {
                theme,
                texture: texture_from_abi(texture),
                uvs,
            });
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_ring_draft_add_texture_uvs(
    ring: *mut cj_ring_draft_t,
    theme: cj_string_view_t,
    texture: cj_texture_id_t,
    uvs: *const cj_uv_t,
    uv_count: usize,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let theme = view_utf8(theme, "theme")?;
        let uvs = if uv_count == 0 {
            Vec::new()
        } else {
            let ptr = NonNull::new(uvs.cast_mut()).ok_or_else(|| {
                invalid_argument("uvs must not be null when uv_count is non-zero")
            })?;
            // SAFETY: the caller promises `uv_count` readable uv coordinates.
            unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), uv_count) }
                .iter()
                .copied()
                .map(|uv| UvAuthoring::New(uv.into()))
                .collect()
        };
        required_ring_draft_mut(ring)?
            .textures
            .push(RingTextureAuthoring {
                theme,
                texture: texture_from_abi(texture),
                uvs,
            });
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_surface_draft_new(
    outer: *mut cj_ring_draft_t,
    out_surface: *mut *mut cj_surface_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_surface, "out_surface")?;
        let outer = take_ring_draft_handle(outer)?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(
                out.as_ptr(),
                surface_draft_into_handle(SurfaceAuthoring {
                    outer,
                    inners: Vec::new(),
                    semantic: None,
                    materials: Vec::new(),
                }),
            );
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_surface_draft_add_inner_ring(
    surface: *mut cj_surface_draft_t,
    inner: *mut cj_ring_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_surface_draft_mut(surface)?
            .inners
            .push(take_ring_draft_handle(inner)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_surface_draft_set_semantic(
    surface: *mut cj_surface_draft_t,
    semantic: cj_semantic_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_surface_draft_mut(surface)?.semantic = Some(semantic_from_abi(semantic));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_surface_draft_add_material(
    surface: *mut cj_surface_draft_t,
    theme: cj_string_view_t,
    material: cj_material_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_surface_draft_mut(surface)?
            .materials
            .push((view_utf8(theme, "theme")?, material_from_abi(material)));
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_shell_draft_new(out_shell: *mut *mut cj_shell_draft_t) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_shell, "out_shell")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(
                out.as_ptr(),
                shell_draft_into_handle(ShellAuthoring::default()),
            );
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_shell_draft_add_surface(
    shell: *mut cj_shell_draft_t,
    surface: *mut cj_surface_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_shell_draft_mut(shell)?
            .surfaces
            .push(take_surface_draft_handle(surface)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_solid_draft_new(
    outer: *mut cj_shell_draft_t,
    out_solid: *mut *mut cj_solid_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let out = required_out(out_solid, "out_solid")?;
        let outer = take_shell_draft_handle(outer)?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(
                out.as_ptr(),
                solid_draft_into_handle(SolidAuthoring {
                    outer,
                    inners: Vec::new(),
                }),
            );
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_solid_draft_add_inner_shell(
    solid: *mut cj_solid_draft_t,
    inner: *mut cj_shell_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        required_solid_draft_mut(solid)?
            .inners
            .push(take_shell_draft_handle(inner)?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_draft_new(
    geometry_type: cj_geometry_type_t,
    lod: cj_string_view_t,
    out_draft: *mut *mut cj_geometry_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let lod = optional_view_utf8(lod, "lod")?;
        let draft = GeometryAuthoring::new(geometry_type_from_abi(geometry_type), parse_lod(lod)?);
        let out = required_out(out_draft, "out_draft")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), geometry_draft_into_handle(draft));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_draft_new_instance(
    template_id: cj_geometry_template_id_t,
    reference_vertex_index: u32,
    transform: cj_affine_transform_4x4_t,
    out_draft: *mut *mut cj_geometry_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let draft = GeometryAuthoring::instance(
            geometry_template_from_abi(template_id),
            VertexAuthoring::Existing(reference_vertex_index.into()),
            affine_transform_from_abi(transform),
        );
        let out = required_out(out_draft, "out_draft")?;
        // SAFETY: `out` is validated to be non-null and points to writable storage.
        unsafe {
            ptr::write(out.as_ptr(), geometry_draft_into_handle(draft));
        }
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_draft_add_point_vertex_index(
    draft: *mut cj_geometry_draft_t,
    vertex_index: u32,
    semantic: *const cj_semantic_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let semantic = if semantic.is_null() {
            None
        } else {
            // SAFETY: the caller promises a readable semantic pointer when non-null.
            Some(semantic_from_abi(unsafe { *semantic }))
        };
        match required_geometry_draft_mut(draft)? {
            GeometryAuthoring::MultiPoint { points, .. } => {
                points.push(PointAuthoring {
                    vertex: VertexAuthoring::Existing(vertex_index.into()),
                    semantic,
                });
                Ok(())
            }
            _ => Err(invalid_argument("geometry draft must be MultiPoint")),
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_draft_add_linestring(
    draft: *mut cj_geometry_draft_t,
    vertex_indices: *const u32,
    vertex_index_count: usize,
    semantic: *const cj_semantic_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let vertices = if vertex_index_count == 0 {
            Vec::new()
        } else {
            let ptr = NonNull::new(vertex_indices.cast_mut()).ok_or_else(|| {
                invalid_argument(
                    "vertex_indices must not be null when vertex_index_count is non-zero",
                )
            })?;
            // SAFETY: the caller promises `vertex_index_count` readable indices.
            unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), vertex_index_count) }
                .iter()
                .copied()
                .map(|index| VertexAuthoring::Existing(index.into()))
                .collect()
        };
        let semantic = if semantic.is_null() {
            None
        } else {
            // SAFETY: the caller promises a readable semantic pointer when non-null.
            Some(semantic_from_abi(unsafe { *semantic }))
        };
        match required_geometry_draft_mut(draft)? {
            GeometryAuthoring::MultiLineString { linestrings, .. } => {
                linestrings.push(LineStringAuthoring { vertices, semantic });
                Ok(())
            }
            _ => Err(invalid_argument("geometry draft must be MultiLineString")),
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_draft_add_surface(
    draft: *mut cj_geometry_draft_t,
    surface: *mut cj_surface_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let surface = take_surface_draft_handle(surface)?;
        match required_geometry_draft_mut(draft)? {
            GeometryAuthoring::MultiSurface { surfaces, .. }
            | GeometryAuthoring::CompositeSurface { surfaces, .. } => {
                surfaces.push(surface);
                Ok(())
            }
            _ => Err(invalid_argument(
                "geometry draft must be MultiSurface or CompositeSurface",
            )),
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_geometry_draft_add_solid(
    draft: *mut cj_geometry_draft_t,
    solid: *mut cj_solid_draft_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let solid = take_solid_draft_handle(solid)?;
        match required_geometry_draft_mut(draft)? {
            GeometryAuthoring::Solid { solid: slot, .. } => {
                *slot = Some(solid);
                Ok(())
            }
            GeometryAuthoring::MultiSolid { solids, .. }
            | GeometryAuthoring::CompositeSolid { solids, .. } => {
                solids.push(solid);
                Ok(())
            }
            _ => Err(invalid_argument(
                "geometry draft must be Solid, MultiSolid, or CompositeSolid",
            )),
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_geometry(
    model: *mut cj_model_t,
    draft: *mut cj_geometry_draft_t,
    out_id: *mut cj_geometry_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let draft = take_geometry_draft_handle(draft)?
            .into_draft()
            .ok_or_else(|| invalid_argument("solid draft is incomplete"))?;
        let handle = draft
            .insert_into(required_model_mut(model)?)
            .map_err(AbiError::from)?;
        write_value(out_id, "out_id", handle.into())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_model_add_geometry_template(
    model: *mut cj_model_t,
    draft: *mut cj_geometry_draft_t,
    out_id: *mut cj_geometry_template_id_t,
) -> cj_status_t {
    ffi_status(run_ffi::<(), AbiError, _>(|| {
        let draft = take_geometry_draft_handle(draft)?
            .into_draft()
            .ok_or_else(|| invalid_argument("solid draft is incomplete"))?;
        let handle = draft
            .insert_template_into(required_model_mut(model)?)
            .map_err(AbiError::from)?;
        write_value(out_id, "out_id", handle.into())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_proj_transformer_create(
    source_crs: cj_string_view_t,
    target_crs: cj_string_view_t,
    out_transformer: *mut *mut cj_proj_transformer_t,
) -> cj_status_t {
    ffi_status(run_ffi(|| {
        let source_crs = view_utf8(source_crs, "source_crs")?;
        let target_crs = view_utf8(target_crs, "target_crs")?;
        #[cfg(feature = "proj")]
        {
            let transformer = cityjson_lib::ops::transformer(&source_crs, &target_crs)?;
            let raw = Box::into_raw(Box::new(transformer)).cast::<cj_proj_transformer_t>();
            write_value(out_transformer, "out_transformer", raw)
        }
        #[cfg(not(feature = "proj"))]
        {
            let _ = source_crs;
            let _ = target_crs;
            let _ = out_transformer;
            Err(unsupported(
                "PROJ support is not enabled; rebuild cityjson-lib-ffi-core with the proj feature",
            ))
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_proj_transformer_free(transformer: *mut cj_proj_transformer_t) -> cj_status_t {
    ffi_status(run_ffi(|| {
        if transformer.is_null() {
            return Ok::<(), AbiError>(());
        }
        #[cfg(feature = "proj")]
        {
            // SAFETY: valid transformer handles originate from `cj_proj_transformer_create`.
            unsafe {
                drop(Box::from_raw(
                    transformer.cast::<cityjson_lib::ops::Transformer>(),
                ));
            }
        }
        #[cfg(not(feature = "proj"))]
        {
            let _ = transformer;
        }
        Ok::<(), AbiError>(())
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn cj_proj_transformer_transform(
    transformer: *const cj_proj_transformer_t,
    point: cj_vertex_t,
    out_point: *mut cj_vertex_t,
) -> cj_status_t {
    ffi_status(run_ffi(|| {
        #[cfg(feature = "proj")]
        {
            let transformer = required_proj_transformer(transformer)?;
            let transformed = transformer.transform([point.x, point.y, point.z])?;
            write_value(
                out_point,
                "out_point",
                cj_vertex_t {
                    x: transformed[0],
                    y: transformed[1],
                    z: transformed[2],
                },
            )
        }
        #[cfg(not(feature = "proj"))]
        {
            let _ = transformer;
            let _ = point;
            let _ = out_point;
            Err(unsupported(
                "PROJ support is not enabled; rebuild cityjson-lib-ffi-core with the proj feature",
            ))
        }
    }))
}
