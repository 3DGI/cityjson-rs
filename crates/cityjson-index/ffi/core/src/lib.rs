use std::collections::{BTreeMap, BTreeSet};
use std::ffi::c_char;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};
use std::slice;

use cityjson_lib::json;
use cityjson_lib_ffi_core::{
    AbiError, bytes_free, bytes_from_string, bytes_from_vec, cj_bytes_t, cj_error_kind_t,
    cj_status_t, clear_last_error, copy_last_error_message, last_error_kind,
    last_error_message_len, run_ffi,
};

use cityjson_index::{
    CityIndex, IndexedCityObjectRef, IndexedPackageRef, LodSelection, MissingLodSelection,
    PackageFilter, PackageFilterReport, PackageType, ResolvedDataset, resolve_dataset,
};

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cjx_index_t {
    _private: [u8; 0],
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_index_status_t {
    pub exists: bool,
    pub needs_reindex: bool,
    pub indexed_feature_count: usize,
    pub indexed_source_count: usize,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cjx_bounds3d_t {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum cjx_package_type_t {
    #[default]
    CJX_PACKAGE_TYPE_CITYJSON = 0,
    CJX_PACKAGE_TYPE_CITYJSON_SEQ = 1,
    CJX_PACKAGE_TYPE_FEATURE_FILES = 2,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cjx_cityobject_ref_t {
    pub record_id: i64,
    pub external_id: cj_bytes_t,
    pub cityobject_type: cj_bytes_t,
    pub has_bounds: bool,
    pub bounds: cjx_bounds3d_t,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct cjx_package_ref_t {
    pub record_id: i64,
    pub model_id: cj_bytes_t,
    pub package_type: cjx_package_type_t,
    pub has_bounds: bool,
    pub bounds: cjx_bounds3d_t,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_string_list_t {
    pub data: *mut cj_bytes_t,
    pub len: usize,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum cjx_lod_selection_kind_t {
    #[default]
    CJX_LOD_SELECTION_ALL = 0,
    CJX_LOD_SELECTION_HIGHEST = 1,
    CJX_LOD_SELECTION_EXACT = 2,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_lod_selection_t {
    pub kind: cjx_lod_selection_kind_t,
    pub exact_lod: cj_bytes_t,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_lod_by_type_t {
    pub cityobject_type: cj_bytes_t,
    pub selection: cjx_lod_selection_t,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_feature_filter_t {
    pub has_cityobject_types: bool,
    pub cityobject_types: cjx_string_list_t,
    pub default_lod: cjx_lod_selection_t,
    pub lods_by_type: *mut cjx_lod_by_type_t,
    pub lods_by_type_len: usize,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_lod_map_entry_t {
    pub cityobject_type: cj_bytes_t,
    pub lods: cjx_string_list_t,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_lod_map_t {
    pub data: *mut cjx_lod_map_entry_t,
    pub len: usize,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_missing_lod_selection_t {
    pub cityobject_type: cj_bytes_t,
    pub requested_lod: cj_bytes_t,
    pub available_lods: cjx_string_list_t,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_missing_lod_selection_list_t {
    pub data: *mut cjx_missing_lod_selection_t,
    pub len: usize,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_feature_filter_diagnostics_t {
    pub available_types: cjx_string_list_t,
    pub retained_types: cjx_string_list_t,
    pub ignored_types: cjx_string_list_t,
    pub available_lods: cjx_lod_map_t,
    pub retained_lods: cjx_lod_map_t,
    pub missing_lods: cjx_missing_lod_selection_list_t,
    pub retained_geometry_count: usize,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct cjx_filtered_feature_t {
    pub model_json: cj_bytes_t,
    pub diagnostics: cjx_feature_filter_diagnostics_t,
}

impl From<IndexedCityObjectRef> for cjx_cityobject_ref_t {
    fn from(value: IndexedCityObjectRef) -> Self {
        let (has_bounds, bounds) = value
            .bounds
            .map(|bounds| {
                (
                    true,
                    cjx_bounds3d_t {
                        min_x: bounds.min_x,
                        max_x: bounds.max_x,
                        min_y: bounds.min_y,
                        max_y: bounds.max_y,
                        min_z: bounds.min_z,
                        max_z: bounds.max_z,
                    },
                )
            })
            .unwrap_or_default();
        Self {
            record_id: value.record_id,
            external_id: bytes_from_string(value.external_id),
            cityobject_type: bytes_from_string(value.cityobject_type),
            has_bounds,
            bounds,
        }
    }
}

impl From<IndexedPackageRef> for cjx_package_ref_t {
    fn from(value: IndexedPackageRef) -> Self {
        let package_type = match value.package_type {
            PackageType::CityJson => cjx_package_type_t::CJX_PACKAGE_TYPE_CITYJSON,
            PackageType::CityJsonSeq => cjx_package_type_t::CJX_PACKAGE_TYPE_CITYJSON_SEQ,
            PackageType::FeatureFiles => cjx_package_type_t::CJX_PACKAGE_TYPE_FEATURE_FILES,
        };
        let (has_bounds, bounds) = value
            .bounds
            .map(|bounds| {
                (
                    true,
                    cjx_bounds3d_t {
                        min_x: bounds.min_x,
                        max_x: bounds.max_x,
                        min_y: bounds.min_y,
                        max_y: bounds.max_y,
                        min_z: bounds.min_z,
                        max_z: bounds.max_z,
                    },
                )
            })
            .unwrap_or_default();
        Self {
            record_id: value.record_id,
            model_id: bytes_from_string(value.model_id),
            package_type,
            has_bounds,
            bounds,
        }
    }
}

impl TryFrom<&cjx_cityobject_ref_t> for IndexedCityObjectRef {
    type Error = AbiError;

    fn try_from(value: &cjx_cityobject_ref_t) -> Result<Self, Self::Error> {
        Ok(Self {
            record_id: value.record_id,
            external_id: bytes_to_string(value.external_id, "external_id")?,
            cityobject_type: bytes_to_string(value.cityobject_type, "cityobject_type")?,
            bounds: None,
        })
    }
}

impl TryFrom<&cjx_package_ref_t> for IndexedPackageRef {
    type Error = AbiError;

    fn try_from(value: &cjx_package_ref_t) -> Result<Self, Self::Error> {
        let package_type = match value.package_type {
            cjx_package_type_t::CJX_PACKAGE_TYPE_CITYJSON => PackageType::CityJson,
            cjx_package_type_t::CJX_PACKAGE_TYPE_CITYJSON_SEQ => PackageType::CityJsonSeq,
            cjx_package_type_t::CJX_PACKAGE_TYPE_FEATURE_FILES => PackageType::FeatureFiles,
        };
        Ok(Self {
            record_id: value.record_id,
            model_id: bytes_to_string(value.model_id, "model_id")?,
            package_type,
            bounds: None,
        })
    }
}

struct OpenedIndex {
    resolved: ResolvedDataset,
    index: CityIndex,
}

impl OpenedIndex {
    fn open(dataset_dir: &Path, index_path: Option<PathBuf>) -> Result<Self, AbiError> {
        let resolved = resolve_dataset(dataset_dir, index_path).map_err(AbiError::from)?;
        let index = CityIndex::open(resolved.storage_layout(), resolved.index_path.as_path())
            .map_err(AbiError::from)?;
        Ok(Self { resolved, index })
    }

    fn status(&self) -> Result<cjx_index_status_t, AbiError> {
        let inspection = self.resolved.inspect().map_err(AbiError::from)?;
        Ok(cjx_index_status_t {
            exists: inspection.index.exists,
            needs_reindex: !inspection.index.fresh.unwrap_or(false),
            indexed_feature_count: inspection.index.indexed_feature_count.unwrap_or(0),
            indexed_source_count: inspection.index.indexed_source_count.unwrap_or(0),
        })
    }

    fn reindex(&mut self) -> Result<(), AbiError> {
        self.index.reindex().map_err(AbiError::from)
    }

    fn lookup_cityobject_refs(
        &self,
        external_id: &str,
    ) -> Result<Vec<cjx_cityobject_ref_t>, AbiError> {
        self.index
            .lookup_cityobject_refs(external_id)
            .map(|refs| refs.into_iter().map(Into::into).collect())
            .map_err(AbiError::from)
    }

    fn package_refs_for_cityobject(
        &self,
        cityobject: &cjx_cityobject_ref_t,
    ) -> Result<Vec<cjx_package_ref_t>, AbiError> {
        let cityobject = IndexedCityObjectRef::try_from(cityobject)?;
        self.index
            .package_refs_for_cityobject(&cityobject)
            .map(|refs| refs.into_iter().map(Into::into).collect())
            .map_err(AbiError::from)
    }

    fn read_package_model_bytes(&self, package: &cjx_package_ref_t) -> Result<Vec<u8>, AbiError> {
        let package = IndexedPackageRef::try_from(package)?;
        let model = self.index.read_package(&package).map_err(AbiError::from)?;
        json::to_vec(&model).map_err(AbiError::from)
    }

    fn read_filtered_packages(
        &self,
        packages: &[cjx_package_ref_t],
        filter: &PackageFilter,
    ) -> Result<Vec<cjx_filtered_feature_t>, AbiError> {
        let packages = packages
            .iter()
            .map(IndexedPackageRef::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let filtered = self
            .index
            .read_filtered_packages(&packages, filter)
            .map_err(AbiError::from)?;
        filtered
            .into_iter()
            .map(|outcome| {
                let model_json = outcome
                    .model
                    .map(|model| json::to_vec(&model).map(bytes_from_vec))
                    .transpose()
                    .map_err(AbiError::from)?
                    .unwrap_or_default();
                Ok(cjx_filtered_feature_t {
                    model_json,
                    diagnostics: package_report_from_abi(&outcome.report),
                })
            })
            .collect::<Result<Vec<_>, AbiError>>()
    }
}

fn lod_selection_from_abi(
    selection: cjx_lod_selection_t,
    name: &'static str,
) -> Result<LodSelection, AbiError> {
    match selection.kind {
        cjx_lod_selection_kind_t::CJX_LOD_SELECTION_ALL => Ok(LodSelection::All),
        cjx_lod_selection_kind_t::CJX_LOD_SELECTION_HIGHEST => Ok(LodSelection::Highest),
        cjx_lod_selection_kind_t::CJX_LOD_SELECTION_EXACT => {
            let exact_lod = bytes_to_string(selection.exact_lod, name)?;
            if exact_lod.is_empty() {
                return Err(AbiError::invalid_argument(format!(
                    "{name} must not be empty for exact LoD selections"
                )));
            }
            Ok(LodSelection::Exact(exact_lod))
        }
    }
}

fn package_filter_from_abi(filter: &cjx_feature_filter_t) -> Result<PackageFilter, AbiError> {
    let cityobject_types = if filter.has_cityobject_types {
        Some(string_list_to_set(
            filter.cityobject_types,
            "filter.cityobject_types",
        )?)
    } else {
        None
    };

    let default_lod = lod_selection_from_abi(filter.default_lod, "filter.default_lod.exact_lod")?;
    let lods_by_type = if filter.lods_by_type_len == 0 {
        BTreeMap::new()
    } else {
        let data = NonNull::new(filter.lods_by_type).ok_or_else(|| {
            AbiError::invalid_argument("filter.lods_by_type must not be null when len is non-zero")
        })?;
        // SAFETY: the caller promises `lods_by_type_len` readable entries at `lods_by_type`.
        let entries = unsafe { slice::from_raw_parts(data.as_ptr(), filter.lods_by_type_len) };
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let cityobject_type = bytes_to_string(
                    entry.cityobject_type,
                    &format!("filter.lods_by_type[{index}].cityobject_type"),
                )?;
                if cityobject_type.is_empty() {
                    return Err(AbiError::invalid_argument(format!(
                        "filter.lods_by_type[{index}].cityobject_type must not be empty"
                    )));
                }
                let selection = lod_selection_from_abi(
                    entry.selection,
                    "filter.lods_by_type.selection.exact_lod",
                )?;
                Ok((cityobject_type, selection))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?
    };

    Ok(PackageFilter {
        cityobject_types,
        default_lod,
        lods_by_type,
    })
}

fn string_list_from_set(items: &BTreeSet<String>) -> cjx_string_list_t {
    let items = items
        .iter()
        .cloned()
        .map(bytes_from_string)
        .collect::<Vec<_>>();
    string_list_from_vec(items)
}

fn string_list_from_vec(items: Vec<cj_bytes_t>) -> cjx_string_list_t {
    if items.is_empty() {
        return cjx_string_list_t::default();
    }

    let boxed = items.into_boxed_slice();
    let len = boxed.len();
    let data = Box::into_raw(boxed).cast::<cj_bytes_t>();
    cjx_string_list_t { data, len }
}

fn lod_map_from_abi(map: &BTreeMap<String, BTreeSet<String>>) -> cjx_lod_map_t {
    let entries = map
        .iter()
        .map(|(cityobject_type, lods)| cjx_lod_map_entry_t {
            cityobject_type: bytes_from_string(cityobject_type.clone()),
            lods: string_list_from_set(lods),
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return cjx_lod_map_t::default();
    }

    let boxed = entries.into_boxed_slice();
    let len = boxed.len();
    let data = Box::into_raw(boxed).cast::<cjx_lod_map_entry_t>();
    cjx_lod_map_t { data, len }
}

fn missing_lod_from_abi(missing: &MissingLodSelection) -> cjx_missing_lod_selection_t {
    cjx_missing_lod_selection_t {
        cityobject_type: bytes_from_string(missing.cityobject_type.clone()),
        requested_lod: bytes_from_string(missing.requested_lod.clone()),
        available_lods: string_list_from_set(&missing.available_lods),
    }
}

fn missing_lods_from_abi(items: &[MissingLodSelection]) -> cjx_missing_lod_selection_list_t {
    if items.is_empty() {
        return cjx_missing_lod_selection_list_t::default();
    }

    let boxed = items
        .iter()
        .map(missing_lod_from_abi)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let len = boxed.len();
    let data = Box::into_raw(boxed).cast::<cjx_missing_lod_selection_t>();
    cjx_missing_lod_selection_list_t { data, len }
}

fn package_report_from_abi(report: &PackageFilterReport) -> cjx_feature_filter_diagnostics_t {
    let missing = report.missing_lods.values().cloned().collect::<Vec<_>>();
    cjx_feature_filter_diagnostics_t {
        available_types: string_list_from_set(&report.available_types),
        retained_types: string_list_from_set(&report.retained_types),
        ignored_types: string_list_from_set(&report.ignored_types),
        available_lods: lod_map_from_abi(&report.available_lods),
        retained_lods: lod_map_from_abi(&report.retained_lods),
        missing_lods: missing_lods_from_abi(&missing),
        retained_geometry_count: report.retained_geometry_count,
    }
}

fn bytes_to_string(value: cj_bytes_t, name: &str) -> Result<String, AbiError> {
    if value.len == 0 {
        return Ok(String::new());
    }
    let data = NonNull::new(value.data)
        .ok_or_else(|| AbiError::invalid_argument(format!("{name} must not be null")))?;
    // SAFETY: the ABI caller promises `len` readable bytes at `data`.
    let bytes = unsafe { slice::from_raw_parts(data.as_ptr().cast::<u8>(), value.len) };
    let value = std::str::from_utf8(bytes).map_err(|error| {
        AbiError::invalid_argument(format!("{name} must be valid UTF-8: {error}"))
    })?;
    Ok(value.to_owned())
}

fn string_list_to_set(list: cjx_string_list_t, name: &str) -> Result<BTreeSet<String>, AbiError> {
    if list.len == 0 {
        return Ok(BTreeSet::new());
    }
    let data = NonNull::new(list.data)
        .ok_or_else(|| AbiError::invalid_argument(format!("{name}.data must not be null")))?;
    // SAFETY: the ABI caller promises `len` readable byte entries at `data`.
    let entries = unsafe { slice::from_raw_parts(data.as_ptr(), list.len) };
    entries
        .iter()
        .enumerate()
        .map(|(index, value)| bytes_to_string(*value, &format!("{name}[{index}]")))
        .collect()
}

fn required_string(
    data: *const c_char,
    len: usize,
    name: &'static str,
) -> Result<String, AbiError> {
    if len == 0 {
        return Err(AbiError::invalid_argument(format!(
            "{name} must not be empty"
        )));
    }
    let ptr = NonNull::new(data.cast_mut())
        .ok_or_else(|| AbiError::invalid_argument(format!("{name} must not be null")))?;
    // SAFETY: the caller promises `len` readable bytes when the pointer is non-null.
    let bytes = unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const().cast::<u8>(), len) };
    let value = std::str::from_utf8(bytes).map_err(|error| {
        AbiError::invalid_argument(format!("{name} must be valid UTF-8: {error}"))
    })?;
    Ok(value.to_owned())
}

fn optional_path(
    data: *const c_char,
    len: usize,
    name: &'static str,
) -> Result<Option<PathBuf>, AbiError> {
    if len == 0 {
        return Ok(None);
    }
    required_string(data, len, name)
        .map(PathBuf::from)
        .map(Some)
}

fn write_ref_slice<T>(
    out_items: *mut *mut T,
    out_count: *mut usize,
    items: Vec<T>,
) -> Result<(), AbiError> {
    let count = items.len();
    write_value(out_count, "out_count", count)?;
    if count == 0 {
        let out_items = NonNull::new(out_items)
            .ok_or_else(|| AbiError::invalid_argument("out_items must not be null"))?;
        unsafe {
            ptr::write(out_items.as_ptr(), ptr::null_mut());
        }
        return Ok(());
    }
    let boxed = items.into_boxed_slice();
    let ptr = Box::into_raw(boxed).cast::<T>();
    write_value(out_items, "out_items", ptr)
}

fn write_value<T>(out: *mut T, name: &'static str, value: T) -> Result<(), AbiError> {
    let out = NonNull::new(out)
        .ok_or_else(|| AbiError::invalid_argument(format!("{name} must not be null")))?;
    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), value);
    }
    Ok(())
}

fn write_handle(out_index: *mut *mut cjx_index_t, index: OpenedIndex) -> Result<(), AbiError> {
    let out = NonNull::new(out_index)
        .ok_or_else(|| AbiError::invalid_argument("out_index must not be null"))?;
    let raw = Box::into_raw(Box::new(index)).cast::<cjx_index_t>();
    // SAFETY: `out` is validated to be non-null and points to writable storage.
    unsafe {
        ptr::write(out.as_ptr(), raw);
    }
    Ok(())
}

fn required_handle<'a>(handle: *const cjx_index_t) -> Result<&'a OpenedIndex, AbiError> {
    let ptr = NonNull::new(handle.cast_mut())
        .ok_or_else(|| AbiError::invalid_argument("index must not be null"))?;
    // SAFETY: the pointer originates from `write_handle`, which stores `OpenedIndex` as the
    // concrete allocation behind `cjx_index_t`.
    Ok(unsafe { &*ptr.as_ptr().cast::<OpenedIndex>() })
}

fn required_handle_mut<'a>(handle: *mut cjx_index_t) -> Result<&'a mut OpenedIndex, AbiError> {
    let ptr =
        NonNull::new(handle).ok_or_else(|| AbiError::invalid_argument("index must not be null"))?;
    // SAFETY: the pointer originates from `write_handle`, which stores `OpenedIndex` as the
    // concrete allocation behind `cjx_index_t`.
    Ok(unsafe { &mut *ptr.as_ptr().cast::<OpenedIndex>() })
}

fn free_cityobject_ref(value: cjx_cityobject_ref_t) {
    // SAFETY: each field is an owned byte buffer allocated by this ABI.
    unsafe {
        bytes_free(value.external_id);
        bytes_free(value.cityobject_type);
    }
}

fn free_package_ref(value: cjx_package_ref_t) {
    // SAFETY: each field is an owned byte buffer allocated by this ABI.
    unsafe {
        bytes_free(value.model_id);
    }
}

fn free_string_list(list: cjx_string_list_t) {
    if list.data.is_null() || list.len == 0 {
        return;
    }

    // SAFETY: the list and each nested byte buffer were allocated by this ABI.
    unsafe {
        let slice = slice::from_raw_parts_mut(list.data, list.len);
        for bytes in slice.iter_mut() {
            bytes_free(*bytes);
        }
        let raw = ptr::slice_from_raw_parts_mut(list.data, list.len);
        drop(Box::from_raw(raw));
    }
}

fn free_lod_map(map: cjx_lod_map_t) {
    if map.data.is_null() || map.len == 0 {
        return;
    }

    // SAFETY: the map and each nested buffer/list were allocated by this ABI.
    unsafe {
        let slice = slice::from_raw_parts_mut(map.data, map.len);
        for entry in slice.iter_mut() {
            bytes_free(entry.cityobject_type);
            free_string_list(entry.lods);
        }
        let raw = ptr::slice_from_raw_parts_mut(map.data, map.len);
        drop(Box::from_raw(raw));
    }
}

fn free_missing_lod_selection(missing: cjx_missing_lod_selection_t) {
    // SAFETY: each field is an owned buffer/list allocated by this ABI.
    unsafe {
        bytes_free(missing.cityobject_type);
        bytes_free(missing.requested_lod);
    }
    free_string_list(missing.available_lods);
}

fn free_missing_lod_selection_list(list: cjx_missing_lod_selection_list_t) {
    if list.data.is_null() || list.len == 0 {
        return;
    }

    // SAFETY: the list and each nested item were allocated by this ABI.
    unsafe {
        let slice = slice::from_raw_parts_mut(list.data, list.len);
        for missing in slice.iter_mut() {
            free_missing_lod_selection(*missing);
        }
        let raw = ptr::slice_from_raw_parts_mut(list.data, list.len);
        drop(Box::from_raw(raw));
    }
}

fn free_diagnostics(diagnostics: cjx_feature_filter_diagnostics_t) {
    free_string_list(diagnostics.available_types);
    free_string_list(diagnostics.retained_types);
    free_string_list(diagnostics.ignored_types);
    free_lod_map(diagnostics.available_lods);
    free_lod_map(diagnostics.retained_lods);
    free_missing_lod_selection_list(diagnostics.missing_lods);
}

fn free_filtered_feature(feature: cjx_filtered_feature_t) {
    // SAFETY: the model buffer was allocated by this ABI.
    unsafe {
        bytes_free(feature.model_json);
    }
    free_diagnostics(feature.diagnostics);
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_clear_error() -> cj_status_t {
    clear_last_error();
    cj_status_t::CJ_STATUS_SUCCESS
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_last_error_kind() -> cj_error_kind_t {
    last_error_kind()
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_last_error_message_len() -> usize {
    last_error_message_len()
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `buffer` must point to `capacity` writable bytes and `out_len` must be a
/// valid writable pointer when non-null.
pub unsafe extern "C" fn cjx_last_error_message_copy(
    buffer: *mut c_char,
    capacity: usize,
    out_len: *mut usize,
) -> cj_status_t {
    // SAFETY: the caller upholds the buffer contract.
    unsafe { copy_last_error_message(buffer, capacity, out_len) }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_bytes_free(bytes: cj_bytes_t) -> cj_status_t {
    // SAFETY: `bytes` originated from this ABI.
    unsafe {
        bytes_free(bytes);
    }
    cj_status_t::CJ_STATUS_SUCCESS
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_open(
    dataset_dir: *const c_char,
    dataset_dir_len: usize,
    index_path: *const c_char,
    index_path_len: usize,
    out_index: *mut *mut cjx_index_t,
) -> cj_status_t {
    match run_ffi(|| {
        let dataset_dir = required_string(dataset_dir, dataset_dir_len, "dataset_dir")?;
        let index_path = optional_path(index_path, index_path_len, "index_path")?;
        let opened = OpenedIndex::open(Path::new(&dataset_dir), index_path)?;
        write_handle(out_index, opened)
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_free(handle: *mut cjx_index_t) -> cj_status_t {
    match run_ffi(|| {
        let handle = required_handle_mut(handle)?;
        let raw = std::ptr::from_mut(handle);
        // SAFETY: `raw` originates from `Box::into_raw` in `write_handle`.
        unsafe {
            drop(Box::from_raw(raw));
        }
        Ok::<(), AbiError>(())
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_status(
    handle: *const cjx_index_t,
    out_status: *mut cjx_index_status_t,
) -> cj_status_t {
    match run_ffi(|| {
        let handle = required_handle(handle)?;
        let status = handle.status()?;
        write_value(out_status, "out_status", status)
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_reindex(handle: *mut cjx_index_t) -> cj_status_t {
    match run_ffi(|| {
        let handle = required_handle_mut(handle)?;
        handle.reindex()
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_lookup_cityobject_refs(
    handle: *const cjx_index_t,
    external_id: *const c_char,
    external_id_len: usize,
    out_refs: *mut *mut cjx_cityobject_ref_t,
    out_count: *mut usize,
) -> cj_status_t {
    match run_ffi(|| {
        let handle = required_handle(handle)?;
        let external_id = required_string(external_id, external_id_len, "external_id")?;
        let refs = handle.lookup_cityobject_refs(&external_id)?;
        write_ref_slice(out_refs, out_count, refs)
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_package_refs_for_cityobject(
    handle: *const cjx_index_t,
    cityobject: *const cjx_cityobject_ref_t,
    out_refs: *mut *mut cjx_package_ref_t,
    out_count: *mut usize,
) -> cj_status_t {
    match run_ffi(|| {
        let handle = required_handle(handle)?;
        let cityobject = NonNull::new(cityobject.cast_mut())
            .ok_or_else(|| AbiError::invalid_argument("cityobject must not be null"))?;
        let refs = handle.package_refs_for_cityobject(unsafe { cityobject.as_ref() })?;
        write_ref_slice(out_refs, out_count, refs)
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `refs` must either be null or point to `count` CityObject refs allocated by this ABI.
pub unsafe extern "C" fn cjx_cityobject_refs_free(
    refs: *mut cjx_cityobject_ref_t,
    count: usize,
) -> cj_status_t {
    match run_ffi(|| {
        if refs.is_null() || count == 0 {
            return Ok::<(), AbiError>(());
        }
        let slice = unsafe { slice::from_raw_parts_mut(refs, count) };
        for item in slice.iter_mut() {
            free_cityobject_ref(*item);
        }
        let raw = ptr::slice_from_raw_parts_mut(refs, count);
        unsafe {
            drop(Box::from_raw(raw));
        }
        Ok::<(), AbiError>(())
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `refs` must either be null or point to `count` package refs allocated by this ABI.
pub unsafe extern "C" fn cjx_package_refs_free(
    refs: *mut cjx_package_ref_t,
    count: usize,
) -> cj_status_t {
    match run_ffi(|| {
        if refs.is_null() || count == 0 {
            return Ok::<(), AbiError>(());
        }
        let slice = unsafe { slice::from_raw_parts_mut(refs, count) };
        for item in slice.iter_mut() {
            free_package_ref(*item);
        }
        let raw = ptr::slice_from_raw_parts_mut(refs, count);
        unsafe {
            drop(Box::from_raw(raw));
        }
        Ok::<(), AbiError>(())
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_read_package_model_bytes(
    handle: *const cjx_index_t,
    package: *const cjx_package_ref_t,
    out_bytes: *mut cj_bytes_t,
) -> cj_status_t {
    match run_ffi(|| {
        let handle = required_handle(handle)?;
        let package = NonNull::new(package.cast_mut())
            .ok_or_else(|| AbiError::invalid_argument("package must not be null"))?;
        let bytes = handle.read_package_model_bytes(unsafe { package.as_ref() })?;
        write_value(out_bytes, "out_bytes", bytes_from_vec(bytes))
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cjx_index_read_filtered_packages(
    handle: *const cjx_index_t,
    refs: *const cjx_package_ref_t,
    ref_count: usize,
    filter: *const cjx_feature_filter_t,
    out_features: *mut *mut cjx_filtered_feature_t,
    out_count: *mut usize,
) -> cj_status_t {
    match run_ffi(|| {
        let handle = required_handle(handle)?;
        let filter = NonNull::new(filter.cast_mut())
            .ok_or_else(|| AbiError::invalid_argument("filter must not be null"))?;
        let filter = package_filter_from_abi(unsafe { filter.as_ref() })?;
        let refs = if ref_count == 0 {
            &[]
        } else {
            let refs = NonNull::new(refs.cast_mut()).ok_or_else(|| {
                AbiError::invalid_argument("refs must not be null when ref_count is non-zero")
            })?;
            unsafe { slice::from_raw_parts(refs.as_ptr(), ref_count) }
        };
        let features = handle.read_filtered_packages(refs, &filter)?;
        write_ref_slice(out_features, out_count, features)
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `features` must either be null or point to `count` filtered features
/// allocated by `cjx_index_read_filtered_packages`.
pub unsafe extern "C" fn cjx_filtered_features_free(
    features: *mut cjx_filtered_feature_t,
    count: usize,
) -> cj_status_t {
    match run_ffi(|| {
        if features.is_null() || count == 0 {
            return Ok::<(), AbiError>(());
        }

        // SAFETY: the caller promises `count` valid filtered features starting at `features`.
        let slice = unsafe { slice::from_raw_parts_mut(features, count) };
        for feature in slice.iter_mut() {
            free_filtered_feature(*feature);
        }

        // SAFETY: `features` was allocated as a boxed slice by this ABI.
        let raw = ptr::slice_from_raw_parts_mut(features, count);
        unsafe {
            drop(Box::from_raw(raw));
        }
        Ok::<(), AbiError>(())
    }) {
        Ok(()) => cj_status_t::CJ_STATUS_SUCCESS,
        Err(status) => status,
    }
}
