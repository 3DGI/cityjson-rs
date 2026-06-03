pub mod benchmark;
pub mod profile;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use cityjson_lib::json::staged;
use cityjson_lib::{CityModel, Error, Result};
use globset::GlobMatcher;
use ignore::WalkBuilder;
use lru::LruCache;
use rusqlite::{OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Number, Value};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FeatureBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FeatureBoundsSummary {
    pub bounds: FeatureBounds,
    pub feature_count: usize,
}

impl FeatureBounds {
    #[must_use]
    pub fn bbox_2d(self) -> BBox {
        BBox {
            min_x: self.min_x,
            max_x: self.max_x,
            min_y: self.min_y,
            max_y: self.max_y,
        }
    }
}

pub struct CityIndex {
    index: Index,
    backend: Box<dyn StorageBackend>,
}

pub const WORKER_COUNT_ENV: &str = "CITYJSON_INDEX_WORKERS";
const DEFAULT_SCAN_PAGE_SIZE: usize = 512;
const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedFeatureRef {
    pub row_id: i64,
    pub feature_id: String,
    pub source_id: i64,
    pub source_path: PathBuf,
    pub offset: u64,
    pub length: u64,
    pub vertices_offset: Option<u64>,
    pub vertices_length: Option<u64>,
    pub member_ranges_json: Option<String>,
    pub bounds: FeatureBounds,
}

pub struct IndexedFeature {
    pub reference: IndexedFeatureRef,
    pub model: CityModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds3D {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageType {
    CityJson,
    CityJsonSeq,
    FeatureFiles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedPackageRef {
    pub record_id: i64,
    pub model_id: String,
    pub package_type: PackageType,
    pub bounds: Option<Bounds3D>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedCityObjectRef {
    pub record_id: i64,
    pub external_id: String,
    pub cityobject_type: String,
    pub bounds: Option<Bounds3D>,
}

pub struct IndexedPackage {
    pub reference: IndexedPackageRef,
    pub metadata: Arc<Meta>,
    pub model: CityModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LodSelection {
    #[default]
    All,
    Highest,
    Exact(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFilter {
    pub cityobject_types: Option<BTreeSet<String>>,
    pub default_lod: LodSelection,
    pub lods_by_type: BTreeMap<String, LodSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingLodSelection {
    pub cityobject_type: String,
    pub requested_lod: String,
    pub available_lods: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFilterDiagnostics {
    pub available_types: BTreeSet<String>,
    pub retained_types: BTreeSet<String>,
    pub ignored_types: BTreeSet<String>,
    pub available_lods: BTreeMap<String, BTreeSet<String>>,
    pub retained_lods: BTreeMap<String, BTreeSet<String>>,
    pub missing_lods: Vec<MissingLodSelection>,
    pub retained_geometry_count: usize,
}

#[derive(Debug, Clone)]
pub struct FilteredFeature {
    pub model: CityModel,
    pub diagnostics: FeatureFilterDiagnostics,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFilterSummary {
    pub available_types: BTreeSet<String>,
    pub retained_types: BTreeSet<String>,
    pub ignored_types: BTreeSet<String>,
    pub available_lods: BTreeMap<String, BTreeSet<String>>,
    pub retained_lods: BTreeMap<String, BTreeSet<String>>,
    pub missing_lods: BTreeMap<String, MissingLodSelection>,
    pub retained_feature_count: usize,
    pub ignored_feature_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFilter {
    pub cityobject_types: Option<BTreeSet<String>>,
    pub default_lod: LodSelection,
    pub lods_by_type: BTreeMap<String, LodSelection>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFilterReport {
    pub available_types: BTreeSet<String>,
    pub retained_types: BTreeSet<String>,
    pub ignored_types: BTreeSet<String>,
    pub available_lods: BTreeMap<String, BTreeSet<String>>,
    pub retained_lods: BTreeMap<String, BTreeSet<String>>,
    pub missing_lods: BTreeMap<String, MissingLodSelection>,
    pub retained_geometry_count: usize,
    pub retained_package_count: usize,
    pub ignored_package_count: usize,
}

pub struct PackageFilterResult {
    pub model: Option<CityModel>,
    pub report: PackageFilterReport,
}

impl From<FeatureBounds> for Bounds3D {
    fn from(bounds: FeatureBounds) -> Self {
        Self {
            min_x: bounds.min_x,
            max_x: bounds.max_x,
            min_y: bounds.min_y,
            max_y: bounds.max_y,
            min_z: bounds.min_z,
            max_z: bounds.max_z,
        }
    }
}

impl PackageFilter {
    /// Applies this package filter to one `CityJSONFeature` package.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `CityObject` or `LoD` selection fails.
    pub fn apply(&self, model: &CityModel) -> Result<PackageFilterResult> {
        let feature_filter = FeatureFilter {
            cityobject_types: self.cityobject_types.clone(),
            default_lod: self.default_lod.clone(),
            lods_by_type: self.lods_by_type.clone(),
        };
        let filtered = feature_filter.apply(model)?;
        let mut report = PackageFilterReport::from_diagnostics(&filtered.diagnostics);
        if let LodSelection::Exact(requested_lod) = &self.default_lod
            && filtered.diagnostics.retained_geometry_count == 0
        {
            for cityobject_type in &filtered.diagnostics.available_types {
                report
                    .missing_lods
                    .entry(cityobject_type.clone())
                    .or_insert_with(|| MissingLodSelection {
                        cityobject_type: cityobject_type.clone(),
                        requested_lod: requested_lod.clone(),
                        available_lods: filtered
                            .diagnostics
                            .available_lods
                            .get(cityobject_type)
                            .cloned()
                            .unwrap_or_default(),
                    });
            }
        }
        let model = if filtered.diagnostics.retained_geometry_count == 0 {
            None
        } else {
            Some(filtered.model)
        };
        Ok(PackageFilterResult { model, report })
    }
}

impl PackageFilterReport {
    fn from_diagnostics(diagnostics: &FeatureFilterDiagnostics) -> Self {
        let mut missing_lods = BTreeMap::new();
        for missing in &diagnostics.missing_lods {
            missing_lods
                .entry(missing.cityobject_type.clone())
                .or_insert_with(|| missing.clone());
        }
        Self {
            available_types: diagnostics.available_types.clone(),
            retained_types: diagnostics.retained_types.clone(),
            ignored_types: diagnostics.ignored_types.clone(),
            available_lods: diagnostics.available_lods.clone(),
            retained_lods: diagnostics.retained_lods.clone(),
            missing_lods,
            retained_geometry_count: diagnostics.retained_geometry_count,
            retained_package_count: usize::from(diagnostics.retained_geometry_count > 0),
            ignored_package_count: usize::from(diagnostics.retained_geometry_count == 0),
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.available_types
            .extend(other.available_types.iter().cloned());
        self.retained_types
            .extend(other.retained_types.iter().cloned());
        self.ignored_types
            .extend(other.ignored_types.iter().cloned());
        merge_lod_sets(&mut self.available_lods, &other.available_lods);
        merge_lod_sets(&mut self.retained_lods, &other.retained_lods);
        for (cityobject_type, missing) in &other.missing_lods {
            self.missing_lods
                .entry(cityobject_type.clone())
                .or_insert_with(|| missing.clone());
        }
        self.retained_geometry_count += other.retained_geometry_count;
        self.retained_package_count += other.retained_package_count;
        self.ignored_package_count += other.ignored_package_count;
    }

    /// Ensures every exact `LoD` requested by `filter` was available in the merged report.
    ///
    /// # Errors
    ///
    /// Returns an error when a requested exact `LoD` is absent for a requested `CityObject` type.
    pub fn ensure_requested_lods_available(&self, filter: &PackageFilter) -> Result<()> {
        for (cityobject_type, selection) in &filter.lods_by_type {
            let LodSelection::Exact(requested_lod) = selection else {
                continue;
            };
            let available_lods = self
                .available_lods
                .get(cityobject_type)
                .cloned()
                .unwrap_or_default();
            if !available_lods.contains(requested_lod) {
                return Err(import_error(format!(
                    "requested LoD {requested_lod} is not available for {cityobject_type}"
                )));
            }
        }
        Ok(())
    }
}

impl IndexedFeatureRef {
    fn to_location(&self) -> FeatureLocation {
        FeatureLocation {
            feature_id: self.feature_id.clone(),
            source_id: self.source_id,
            source_path: self.source_path.clone(),
            offset: self.offset,
            length: self.length,
            vertices_offset: self.vertices_offset,
            vertices_length: self.vertices_length,
            member_ranges_json: self.member_ranges_json.clone(),
        }
    }
}

impl FeatureFilter {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.cityobject_types.is_some()
            || self.default_lod != LodSelection::All
            || self
                .lods_by_type
                .values()
                .any(|selection| *selection != LodSelection::All)
    }

    /// Applies this filter to a single decoded `CityJSON` feature.
    ///
    /// Object-type selection keeps descendants of selected `CityObjects`. This
    /// preserves common `CityJSON` features where a selected parent object, such
    /// as `Building`, carries its renderable geometry on child objects.
    ///
    /// # Errors
    ///
    /// Returns an error if the source model contains invalid references or
    /// `cityjson-lib` extraction fails.
    pub fn apply(&self, model: &CityModel) -> Result<FilteredFeature> {
        let retained_handles = retained_cityobject_handles(model, self)?;
        let diagnostics = filter_diagnostics(model, &retained_handles, self);

        if !self.is_active() {
            return Ok(FilteredFeature {
                model: model.clone(),
                diagnostics,
            });
        }

        let type_selection = self
            .cityobject_types
            .as_ref()
            .map(|_| {
                cityjson_lib::ops::select_cityobjects(model, |ctx| {
                    retained_handles.contains(&ctx.handle())
                })
            })
            .transpose()?;

        let lod_selection = lod_selection(model, &retained_handles, self)?;
        let selection = match (type_selection, lod_selection) {
            (Some(types), Some(lods)) => types.intersection(&lods),
            (Some(types), None) => types,
            (None, Some(lods)) => lods,
            (None, None) => {
                return Ok(FilteredFeature {
                    model: model.clone(),
                    diagnostics,
                });
            }
        };

        let filtered = extract_or_empty_feature(model, &selection)?;
        Ok(FilteredFeature {
            model: filtered,
            diagnostics,
        })
    }
}

impl FeatureFilterSummary {
    pub fn add(&mut self, diagnostics: &FeatureFilterDiagnostics) {
        self.available_types
            .extend(diagnostics.available_types.iter().cloned());
        self.retained_types
            .extend(diagnostics.retained_types.iter().cloned());
        self.ignored_types
            .extend(diagnostics.ignored_types.iter().cloned());
        merge_lod_sets(&mut self.available_lods, &diagnostics.available_lods);
        merge_lod_sets(&mut self.retained_lods, &diagnostics.retained_lods);
        for missing in &diagnostics.missing_lods {
            self.missing_lods
                .entry(missing.cityobject_type.clone())
                .or_insert_with(|| missing.clone());
        }
        if diagnostics.retained_geometry_count == 0 {
            self.ignored_feature_count += 1;
        } else {
            self.retained_feature_count += 1;
        }
    }

    #[must_use]
    pub fn requested_lod_failures(&self, filter: &FeatureFilter) -> Vec<MissingLodSelection> {
        filter
            .lods_by_type
            .iter()
            .filter_map(|(cityobject_type, selection)| {
                let LodSelection::Exact(requested_lod) = selection else {
                    return None;
                };
                let eligible = self.available_lods.contains_key(cityobject_type)
                    || self.retained_types.contains(cityobject_type)
                    || filter
                        .cityobject_types
                        .as_ref()
                        .is_none_or(|types| types.contains(cityobject_type));
                if !eligible {
                    return None;
                }
                let available_lods = self
                    .available_lods
                    .get(cityobject_type)
                    .cloned()
                    .unwrap_or_default();
                if available_lods.contains(requested_lod) {
                    return None;
                }
                Some(MissingLodSelection {
                    cityobject_type: cityobject_type.clone(),
                    requested_lod: requested_lod.clone(),
                    available_lods,
                })
            })
            .collect()
    }

    /// # Errors
    ///
    /// Returns an error when any explicit `LoD` selector did not match the
    /// scanned filtered dataset.
    pub fn ensure_requested_lods_available(&self, filter: &FeatureFilter) -> Result<()> {
        let failures = self.requested_lod_failures(filter);
        if failures.is_empty() {
            return Ok(());
        }

        let details = failures
            .iter()
            .map(|missing| {
                let available = if missing.available_lods.is_empty() {
                    "none".to_owned()
                } else {
                    missing
                        .available_lods
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                format!(
                    "{} requested LoD '{}' but available LoDs are: {}",
                    missing.cityobject_type, missing.requested_lod, available
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        Err(import_error(format!(
            "requested LoD selector matched no geometry: {details}"
        )))
    }
}

fn merge_lod_sets(
    target: &mut BTreeMap<String, BTreeSet<String>>,
    source: &BTreeMap<String, BTreeSet<String>>,
) {
    for (cityobject_type, lods) in source {
        target
            .entry(cityobject_type.clone())
            .or_default()
            .extend(lods.iter().cloned());
    }
}

type CityObjectHandle = cityjson_types::prelude::CityObjectHandle;
type GeometryHandle = cityjson_types::prelude::GeometryHandle;

fn retained_cityobject_handles(
    model: &CityModel,
    filter: &FeatureFilter,
) -> Result<BTreeSet<CityObjectHandle>> {
    let Some(selected_types) = filter.cityobject_types.as_ref() else {
        return Ok(model
            .cityobjects()
            .iter()
            .map(|(handle, _)| handle)
            .collect());
    };

    let mut retained = BTreeSet::new();
    for (handle, cityobject) in model.cityobjects().iter() {
        if selected_types.contains(&cityobject.type_cityobject().to_string()) {
            collect_cityobject_descendants(model, handle, &mut retained)?;
        }
    }
    Ok(retained)
}

fn collect_cityobject_descendants(
    model: &CityModel,
    handle: CityObjectHandle,
    retained: &mut BTreeSet<CityObjectHandle>,
) -> Result<()> {
    if !retained.insert(handle) {
        return Ok(());
    }
    let cityobject = model.cityobjects().get(handle).ok_or_else(|| {
        import_error(format!(
            "missing CityObject handle in filter traversal: {handle:?}"
        ))
    })?;
    if let Some(children) = cityobject.children() {
        for child in children {
            collect_cityobject_descendants(model, *child, retained)?;
        }
    }
    Ok(())
}

fn lod_selection(
    model: &CityModel,
    retained_handles: &BTreeSet<CityObjectHandle>,
    filter: &FeatureFilter,
) -> Result<Option<cityjson_lib::ops::ModelSelection>> {
    if filter.default_lod == LodSelection::All
        && filter
            .lods_by_type
            .values()
            .all(|selection| *selection == LodSelection::All)
    {
        return Ok(None);
    }

    let highest_lods = highest_lods_by_cityobject(model, retained_handles);
    cityjson_lib::ops::select_geometries(model, |ctx| {
        if !retained_handles.contains(&ctx.cityobject_handle()) {
            return false;
        }
        let cityobject_type = ctx.cityobject().type_cityobject().to_string();
        let selection = filter
            .lods_by_type
            .get(&cityobject_type)
            .unwrap_or(&filter.default_lod);
        geometry_matches_lod_selection(
            ctx.geometry().lod(),
            highest_lods.get(&ctx.cityobject_handle()),
            selection,
        )
    })
    .map(Some)
}

fn geometry_matches_lod_selection(
    geometry_lod: Option<&cityjson_types::v2_0::LoD>,
    highest_lod: Option<&String>,
    selection: &LodSelection,
) -> bool {
    match selection {
        LodSelection::All => true,
        LodSelection::Highest => geometry_lod
            .is_some_and(|lod| highest_lod.is_some_and(|highest| lod.to_string() == *highest)),
        LodSelection::Exact(selected_lod) => {
            geometry_lod.is_some_and(|lod| lod.to_string() == *selected_lod)
        }
    }
}

fn highest_lods_by_cityobject(
    model: &CityModel,
    retained_handles: &BTreeSet<CityObjectHandle>,
) -> BTreeMap<CityObjectHandle, String> {
    let mut highest = BTreeMap::new();
    for handle in retained_handles {
        let Some(cityobject) = model.cityobjects().get(*handle) else {
            continue;
        };
        let Some(geometry_handles) = cityobject.geometry() else {
            continue;
        };
        if let Some(lod) = highest_lod(model, geometry_handles) {
            highest.insert(*handle, lod);
        }
    }
    highest
}

fn highest_lod(model: &CityModel, geometries: &[GeometryHandle]) -> Option<String> {
    geometries
        .iter()
        .filter_map(|geometry_handle| {
            model
                .get_geometry(*geometry_handle)
                .and_then(|geometry| geometry.lod())
                .map(std::string::ToString::to_string)
        })
        .max_by(|lhs, rhs| compare_lod_strings(lhs, rhs))
}

fn compare_lod_strings(lhs: &str, rhs: &str) -> std::cmp::Ordering {
    match (lhs.parse::<f64>(), rhs.parse::<f64>()) {
        (Ok(lhs), Ok(rhs)) => lhs.partial_cmp(&rhs).unwrap_or(std::cmp::Ordering::Equal),
        _ => lhs.cmp(rhs),
    }
}

fn filter_diagnostics(
    model: &CityModel,
    retained_handles: &BTreeSet<CityObjectHandle>,
    filter: &FeatureFilter,
) -> FeatureFilterDiagnostics {
    let highest_lods = highest_lods_by_cityobject(model, retained_handles);
    let mut diagnostics = FeatureFilterDiagnostics::default();

    for (handle, cityobject) in model.cityobjects().iter() {
        let cityobject_type = cityobject.type_cityobject().to_string();
        diagnostics.available_types.insert(cityobject_type.clone());
        if retained_handles.contains(&handle) {
            diagnostics.retained_types.insert(cityobject_type.clone());
        } else {
            diagnostics.ignored_types.insert(cityobject_type.clone());
            continue;
        }

        let Some(geometry_handles) = cityobject.geometry() else {
            continue;
        };
        let selection = filter
            .lods_by_type
            .get(&cityobject_type)
            .unwrap_or(&filter.default_lod);
        for geometry_handle in geometry_handles {
            let Some(geometry_lod) = model
                .get_geometry(*geometry_handle)
                .and_then(|geometry| geometry.lod())
            else {
                continue;
            };
            let lod = geometry_lod.to_string();
            diagnostics
                .available_lods
                .entry(cityobject_type.clone())
                .or_default()
                .insert(lod.clone());
            if geometry_matches_lod_selection(
                Some(geometry_lod),
                highest_lods.get(&handle),
                selection,
            ) {
                diagnostics
                    .retained_lods
                    .entry(cityobject_type.clone())
                    .or_default()
                    .insert(lod);
                diagnostics.retained_geometry_count += 1;
            }
        }
    }

    for (cityobject_type, selection) in &filter.lods_by_type {
        let LodSelection::Exact(requested_lod) = selection else {
            continue;
        };
        if !diagnostics.retained_types.contains(cityobject_type) {
            continue;
        }
        let available_lods = diagnostics
            .available_lods
            .get(cityobject_type)
            .cloned()
            .unwrap_or_default();
        if !available_lods.contains(requested_lod) {
            diagnostics.missing_lods.push(MissingLodSelection {
                cityobject_type: cityobject_type.clone(),
                requested_lod: requested_lod.clone(),
                available_lods,
            });
        }
    }

    diagnostics
}

fn extract_or_empty_feature(
    model: &CityModel,
    selection: &cityjson_lib::ops::ModelSelection,
) -> Result<CityModel> {
    if !selection.is_empty() {
        return cityjson_lib::ops::extract(model, selection);
    }

    let mut empty = model.clone();
    empty.clear_cityobjects();
    empty.set_id(None);
    Ok(empty)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatasetLayoutKind {
    #[serde(rename = "cityjson-seq")]
    Ndjson,
    #[serde(rename = "cityjson")]
    CityJson,
    #[serde(rename = "feature-files")]
    FeatureFiles,
}

impl DatasetLayoutKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ndjson => "cityjson-seq",
            Self::CityJson => "cityjson",
            Self::FeatureFiles => "feature-files",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSummary {
    pub path: PathBuf,
    pub selected_tile_count: Option<usize>,
    pub total_features: Option<usize>,
    pub total_cityobjects: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ResolvedDataset {
    pub dataset_root: PathBuf,
    pub index_path: PathBuf,
    pub layout: DatasetLayoutKind,
    pub manifest: Option<ManifestSummary>,
    storage_layout: StorageLayout,
    source_paths: Vec<PathBuf>,
    feature_file_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub path: PathBuf,
    pub exists: bool,
    pub index_mtime_ns: Option<i64>,
    pub schema_version: Option<i64>,
    pub indexed_source_count: Option<usize>,
    pub indexed_feature_count: Option<usize>,
    pub indexed_package_count: Option<usize>,
    pub indexed_cityobject_count: Option<usize>,
    pub indexed_cityobject_relationship_count: Option<usize>,
    pub fresh: Option<bool>,
    pub covered: Option<bool>,
    pub needs_reindex: bool,
    pub missing_source_paths: Vec<PathBuf>,
    pub unindexed_source_paths: Vec<PathBuf>,
    pub changed_source_paths: Vec<PathBuf>,
    pub missing_feature_paths: Vec<PathBuf>,
    pub unindexed_feature_paths: Vec<PathBuf>,
    pub changed_feature_paths: Vec<PathBuf>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInspection {
    pub dataset_root: PathBuf,
    pub layout: DatasetLayoutKind,
    pub manifest: Option<ManifestSummary>,
    pub detected_source_count: usize,
    pub detected_feature_file_count: usize,
    pub index: IndexStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub inspection: DatasetInspection,
}

#[derive(Debug, Clone)]
pub enum StorageLayout {
    Ndjson {
        paths: Vec<PathBuf>,
    },
    CityJson {
        paths: Vec<PathBuf>,
    },
    FeatureFiles {
        root: PathBuf,
        metadata_glob: String,
        feature_glob: String,
    },
}

impl StorageLayout {
    #[must_use]
    pub fn layout_kind(&self) -> DatasetLayoutKind {
        match self {
            Self::Ndjson { .. } => DatasetLayoutKind::Ndjson,
            Self::CityJson { .. } => DatasetLayoutKind::CityJson,
            Self::FeatureFiles { .. } => DatasetLayoutKind::FeatureFiles,
        }
    }
}

impl ResolvedDataset {
    #[must_use]
    pub fn storage_layout(&self) -> StorageLayout {
        self.storage_layout.clone()
    }

    #[must_use]
    pub fn source_paths(&self) -> &[PathBuf] {
        &self.source_paths
    }

    #[must_use]
    pub fn feature_file_paths(&self) -> &[PathBuf] {
        &self.feature_file_paths
    }

    /// Inspects the resolved dataset and its current index sidecar.
    ///
    /// # Errors
    ///
    /// Returns an error if the dataset or index cannot be inspected.
    pub fn inspect(&self) -> Result<DatasetInspection> {
        inspect_resolved_dataset(self)
    }

    /// Validates the resolved dataset and returns a structured report.
    ///
    /// # Errors
    ///
    /// Returns an error if the dataset or index cannot be inspected.
    pub fn validate(&self) -> Result<ValidationReport> {
        let inspection = self.inspect()?;
        let ok = inspection.index.issues.is_empty();
        Ok(ValidationReport { ok, inspection })
    }
}

#[allow(clippy::too_many_lines)]
fn inspect_resolved_dataset(resolved: &ResolvedDataset) -> Result<DatasetInspection> {
    let mut status = IndexStatus {
        path: resolved.index_path.clone(),
        exists: resolved.index_path.exists(),
        index_mtime_ns: None,
        schema_version: None,
        indexed_source_count: None,
        indexed_feature_count: None,
        indexed_package_count: None,
        indexed_cityobject_count: None,
        indexed_cityobject_relationship_count: None,
        fresh: None,
        covered: None,
        needs_reindex: false,
        missing_source_paths: Vec::new(),
        unindexed_source_paths: Vec::new(),
        changed_source_paths: Vec::new(),
        missing_feature_paths: Vec::new(),
        unindexed_feature_paths: Vec::new(),
        changed_feature_paths: Vec::new(),
        issues: Vec::new(),
    };

    if status.exists {
        let (_, mtime_ns) = file_status(&resolved.index_path)?;
        status.index_mtime_ns = Some(mtime_ns);

        let index = Index::open(&resolved.index_path)?;
        status.schema_version = Some(index.current_schema_version()?);
        status.indexed_source_count = Some(index.source_count()?);
        status.indexed_feature_count = Some(index.feature_count()?);
        status.indexed_package_count = Some(index.package_count()?);
        status.indexed_cityobject_count = Some(index.normalized_cityobject_count()?);
        status.indexed_cityobject_relationship_count = Some(index.cityobject_relationship_count()?);
        if !index.feature_bounds_complete()? {
            status.needs_reindex = true;
            status
                .issues
                .push("index is missing persisted z bounds; run cjindex reindex".to_owned());
        }

        let indexed_sources = index.indexed_sources()?;
        let current_sources = collect_current_file_statuses(&resolved.source_paths)?;
        compare_path_statuses(
            &current_sources,
            &indexed_sources,
            &mut status.missing_source_paths,
            &mut status.unindexed_source_paths,
            &mut status.changed_source_paths,
            &mut status.needs_reindex,
        );

        if resolved.layout == DatasetLayoutKind::FeatureFiles {
            let indexed_features = index.indexed_feature_paths()?;
            let current_features = collect_current_file_statuses(&resolved.feature_file_paths)?;
            compare_feature_statuses(
                &current_features,
                &indexed_features,
                &mut status.missing_feature_paths,
                &mut status.unindexed_feature_paths,
                &mut status.changed_feature_paths,
                &mut status.needs_reindex,
            );
        }

        if let Some(manifest) = &resolved.manifest {
            if let Some(expected_features) = manifest.total_features
                && status.indexed_feature_count != Some(expected_features)
            {
                status.issues.push(format!(
                    "indexed feature count {} does not match manifest count {}",
                    status.indexed_feature_count.unwrap_or(0),
                    expected_features
                ));
            }
            if let Some(expected_cityobjects) = manifest.total_cityobjects
                && status.indexed_cityobject_count != Some(expected_cityobjects)
            {
                status.issues.push(format!(
                    "indexed CityObject count {} does not match manifest count {}",
                    status.indexed_cityobject_count.unwrap_or(0),
                    expected_cityobjects
                ));
            }
            if let Some(expected_sources) = manifest.selected_tile_count
                && resolved.layout != DatasetLayoutKind::FeatureFiles
                && status.indexed_source_count != Some(expected_sources)
            {
                status.issues.push(format!(
                    "indexed source count {} does not match manifest tile count {}",
                    status.indexed_source_count.unwrap_or(0),
                    expected_sources
                ));
            }
        }

        if let Some(source_count) = status.indexed_source_count
            && source_count != resolved.source_paths.len()
        {
            status.issues.push(format!(
                "indexed source count {} does not match detected source count {}",
                source_count,
                resolved.source_paths.len()
            ));
        }

        if !status.missing_source_paths.is_empty() {
            status.issues.push(format!(
                "{} indexed source files are missing on disk",
                status.missing_source_paths.len()
            ));
        }
        if !status.unindexed_source_paths.is_empty() {
            status.issues.push(format!(
                "{} detected source files are missing from the index",
                status.unindexed_source_paths.len()
            ));
        }
        if !status.changed_source_paths.is_empty() {
            status.issues.push(format!(
                "{} indexed source files changed size or mtime",
                status.changed_source_paths.len()
            ));
        }
        if !status.missing_feature_paths.is_empty() {
            status.issues.push(format!(
                "{} indexed feature files are missing on disk",
                status.missing_feature_paths.len()
            ));
        }
        if !status.unindexed_feature_paths.is_empty() {
            status.issues.push(format!(
                "{} detected feature files are missing from the index",
                status.unindexed_feature_paths.len()
            ));
        }
        if !status.changed_feature_paths.is_empty() {
            status.issues.push(format!(
                "{} indexed feature files changed size or mtime",
                status.changed_feature_paths.len()
            ));
        }
        if status.needs_reindex {
            status.issues.push(
                "index is missing persisted freshness metadata; run cjindex reindex".to_owned(),
            );
        }

        status.covered = Some(
            status.missing_source_paths.is_empty()
                && status.unindexed_source_paths.is_empty()
                && status.missing_feature_paths.is_empty()
                && status.unindexed_feature_paths.is_empty(),
        );
        status.fresh = Some(
            status.covered == Some(true)
                && status.changed_source_paths.is_empty()
                && status.changed_feature_paths.is_empty()
                && !status.needs_reindex,
        );
    } else {
        status.issues.push(format!(
            "index {} does not exist",
            resolved.index_path.display()
        ));
    }

    Ok(DatasetInspection {
        dataset_root: resolved.dataset_root.clone(),
        layout: resolved.layout,
        manifest: resolved.manifest.clone(),
        detected_source_count: resolved.source_paths.len(),
        detected_feature_file_count: resolved.feature_file_paths.len(),
        index: status,
    })
}

fn resolve_manifest_summary(dataset_root: &Path) -> Result<Option<ManifestSummary>> {
    let candidates = [
        dataset_root.join("manifest.json"),
        dataset_root.parent().map_or_else(
            || dataset_root.join("manifest.json"),
            |parent| parent.join("manifest.json"),
        ),
    ];
    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        let manifest: Value = read_json(&candidate)?;
        let selected_tile_count = manifest
            .get("selected_tiles")
            .and_then(Value::as_array)
            .map(Vec::len);
        let total_features = manifest
            .get("total_features")
            .and_then(Value::as_u64)
            .map(usize::try_from)
            .transpose()
            .map_err(|_| import_error("manifest total_features does not fit in usize"))?;
        let total_cityobjects = manifest
            .get("total_cityobjects")
            .and_then(Value::as_u64)
            .map(usize::try_from)
            .transpose()
            .map_err(|_| import_error("manifest total_cityobjects does not fit in usize"))?;
        return Ok(Some(ManifestSummary {
            path: candidate,
            selected_tile_count,
            total_features,
            total_cityobjects,
        }));
    }
    Ok(None)
}

fn collect_current_file_statuses(paths: &[PathBuf]) -> Result<BTreeMap<PathBuf, (u64, i64)>> {
    paths
        .iter()
        .map(|path| file_status(path).map(|status| (path.clone(), status)))
        .collect()
}

fn compare_path_statuses(
    current: &BTreeMap<PathBuf, (u64, i64)>,
    indexed: &[IndexedSourceRecord],
    missing_on_disk: &mut Vec<PathBuf>,
    missing_from_index: &mut Vec<PathBuf>,
    changed: &mut Vec<PathBuf>,
    needs_reindex: &mut bool,
) {
    let indexed_by_path = indexed
        .iter()
        .map(|record| {
            (
                record.path.clone(),
                (record.source_size, record.source_mtime_ns),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for path in current.keys() {
        if !indexed_by_path.contains_key(path) {
            missing_from_index.push(path.clone());
        }
    }

    for (path, (expected_size, expected_mtime_ns)) in indexed_by_path {
        let Some((current_size, current_mtime_ns)) = current.get(&path) else {
            missing_on_disk.push(path);
            continue;
        };
        let Some(expected_size) = expected_size else {
            *needs_reindex = true;
            continue;
        };
        let Some(expected_mtime_ns) = expected_mtime_ns else {
            *needs_reindex = true;
            continue;
        };
        if expected_size != *current_size || expected_mtime_ns != *current_mtime_ns {
            changed.push(path);
        }
    }
}

fn compare_feature_statuses(
    current: &BTreeMap<PathBuf, (u64, i64)>,
    indexed: &[IndexedFeaturePathRecord],
    missing_on_disk: &mut Vec<PathBuf>,
    missing_from_index: &mut Vec<PathBuf>,
    changed: &mut Vec<PathBuf>,
    needs_reindex: &mut bool,
) {
    let indexed_by_path = indexed
        .iter()
        .map(|record| {
            (
                record.path.clone(),
                (record.file_size, record.file_mtime_ns),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for path in current.keys() {
        if !indexed_by_path.contains_key(path) {
            missing_from_index.push(path.clone());
        }
    }

    for (path, (expected_size, expected_mtime_ns)) in indexed_by_path {
        let Some((current_size, current_mtime_ns)) = current.get(&path) else {
            missing_on_disk.push(path);
            continue;
        };
        let Some(expected_size) = expected_size else {
            *needs_reindex = true;
            continue;
        };
        let Some(expected_mtime_ns) = expected_mtime_ns else {
            *needs_reindex = true;
            continue;
        };
        if expected_size != *current_size || expected_mtime_ns != *current_mtime_ns {
            changed.push(path);
        }
    }
}

/// Resolves a dataset directory into one concrete storage layout plus the
/// effective sidecar index location.
///
/// # Errors
///
/// Returns an error if the directory does not exist, no known layout matches,
/// or multiple layouts match.
pub fn resolve_dataset(
    dataset_dir: &Path,
    index_override: Option<PathBuf>,
) -> Result<ResolvedDataset> {
    let dataset_root = fs::canonicalize(dataset_dir).map_err(|error| {
        import_error(format!(
            "failed to resolve dataset directory {}: {error}",
            dataset_dir.display()
        ))
    })?;
    if !dataset_root.is_dir() {
        return Err(import_error(format!(
            "dataset path {} is not a directory",
            dataset_root.display()
        )));
    }

    let roots = vec![dataset_root.clone()];
    let ndjson_paths = collect_layout_files(&roots, ".city.jsonl")?;
    let cityjson_paths = collect_layout_files(&roots, ".city.json")?;
    let metadata_paths = collect_layout_files(&roots, "metadata.json")?;
    let feature_file_paths = if metadata_paths.is_empty() {
        Vec::new()
    } else {
        ndjson_paths.clone()
    };

    let feature_files_match = !metadata_paths.is_empty() && !feature_file_paths.is_empty();
    let ndjson_match = !ndjson_paths.is_empty() && !feature_files_match;
    let cityjson_match = !cityjson_paths.is_empty();

    let mut matches = Vec::new();
    if ndjson_match {
        matches.push(DatasetLayoutKind::Ndjson);
    }
    if cityjson_match {
        matches.push(DatasetLayoutKind::CityJson);
    }
    if feature_files_match {
        matches.push(DatasetLayoutKind::FeatureFiles);
    }

    if matches.is_empty() {
        return Err(import_error(format!(
            "dataset directory {} does not match cityjson-seq, cityjson, or feature-files layouts",
            dataset_root.display()
        )));
    }
    if matches.len() > 1 {
        let matched_layouts = matches
            .into_iter()
            .map(DatasetLayoutKind::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(import_error(format!(
            "dataset directory {} matches multiple layouts ({matched_layouts}); use explicit CLI flags instead",
            dataset_root.display(),
        )));
    }

    let layout = matches[0];
    let storage_layout = match layout {
        DatasetLayoutKind::Ndjson => StorageLayout::Ndjson {
            paths: vec![dataset_root.clone()],
        },
        DatasetLayoutKind::CityJson => StorageLayout::CityJson {
            paths: vec![dataset_root.clone()],
        },
        DatasetLayoutKind::FeatureFiles => StorageLayout::FeatureFiles {
            root: dataset_root.clone(),
            metadata_glob: "**/metadata.json".to_owned(),
            feature_glob: "**/*.city.jsonl".to_owned(),
        },
    };
    let source_paths = match layout {
        DatasetLayoutKind::Ndjson => ndjson_paths,
        DatasetLayoutKind::CityJson => cityjson_paths,
        DatasetLayoutKind::FeatureFiles => metadata_paths,
    };
    let feature_file_paths = match layout {
        DatasetLayoutKind::FeatureFiles => feature_file_paths,
        _ => Vec::new(),
    };

    Ok(ResolvedDataset {
        dataset_root: dataset_root.clone(),
        index_path: index_override.unwrap_or_else(|| dataset_root.join(".cityjson-index.sqlite")),
        layout,
        manifest: resolve_manifest_summary(&dataset_root)?,
        storage_layout,
        source_paths,
        feature_file_paths,
    })
}

fn clone_indexed_package(package: &IndexedPackage) -> IndexedPackage {
    IndexedPackage {
        reference: package.reference.clone(),
        metadata: Arc::clone(&package.metadata),
        model: package.model.clone(),
    }
}

fn package_type_from_str(value: &str) -> rusqlite::Result<PackageType> {
    match value {
        "cityjson" => Ok(PackageType::CityJson),
        "cityjson-seq" => Ok(PackageType::CityJsonSeq),
        "feature-files" => Ok(PackageType::FeatureFiles),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn optional_bounds_from_row(
    row: &rusqlite::Row<'_>,
    col: usize,
) -> rusqlite::Result<Option<Bounds3D>> {
    let min_x = row.get::<_, Option<f64>>(col)?;
    let Some(min_x) = min_x else {
        return Ok(None);
    };
    Ok(Some(Bounds3D {
        min_x,
        max_x: row.get::<_, f64>(col + 1)?,
        min_y: row.get::<_, f64>(col + 2)?,
        max_y: row.get::<_, f64>(col + 3)?,
        min_z: row.get::<_, f64>(col + 4)?,
        max_z: row.get::<_, f64>(col + 5)?,
    }))
}

fn package_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedPackageRef> {
    Ok(IndexedPackageRef {
        record_id: row.get(0)?,
        model_id: row.get(1)?,
        package_type: package_type_from_str(&row.get::<_, String>(2)?)?,
        bounds: optional_bounds_from_row(row, 3)?,
    })
}

fn cityobject_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedCityObjectRef> {
    Ok(IndexedCityObjectRef {
        record_id: row.get(0)?,
        external_id: row.get(1)?,
        cityobject_type: row.get(2)?,
        bounds: optional_bounds_from_row(row, 3)?,
    })
}

impl CityIndex {
    /// Opens an index for the given storage layout.
    ///
    /// # Errors
    ///
    /// Returns an error if the index backend cannot be created or the index
    /// store cannot be opened.
    pub fn open(layout: StorageLayout, index_path: &Path) -> Result<Self> {
        let backend: Box<dyn StorageBackend> = match layout {
            StorageLayout::Ndjson { paths } => Box::new(NdjsonBackend { paths }),
            StorageLayout::CityJson { paths } => Box::new(CityJsonBackend::new(paths)),
            StorageLayout::FeatureFiles {
                root,
                metadata_glob,
                feature_glob,
            } => Box::new(FeatureFilesBackend::new(
                root,
                metadata_glob.as_str(),
                feature_glob.as_str(),
            )),
        };

        Ok(Self {
            index: Index::open(index_path)?,
            backend,
        })
    }

    /// Rebuilds the index from the configured backend.
    ///
    /// # Errors
    ///
    /// Returns an error if backend scanning or index population fails.
    pub fn reindex(&mut self) -> Result<()> {
        let worker_count = configured_worker_count()?;
        let scans = self.backend.scan(worker_count)?;
        self.index.rebuild(&scans)
    }

    /// Returns a `CityJSON` feature by id.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup fails.
    pub fn get(&self, id: &str) -> Result<Option<CityModel>> {
        self.get_with_metadata(id)
            .map(|maybe| maybe.map(|(_, model)| model))
    }

    /// Returns a `CityJSON` feature by id together with the source metadata
    /// used to reconstruct it.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup fails.
    pub fn get_with_metadata(&self, id: &str) -> Result<Option<(Arc<Meta>, CityModel)>> {
        let Some(loc) = self.index.lookup_id(id)? else {
            return Ok(None);
        };
        let metadata = self.index.get_cached_metadata(loc.source_id)?;
        let model = self.backend.read_one(&loc, Arc::clone(&metadata.bytes))?;
        Ok(Some((metadata.value, model)))
    }

    /// Returns every indexed `CityObject` occurrence with the given external id.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` lookup fails.
    pub fn lookup_cityobject_refs(&self, external_id: &str) -> Result<Vec<IndexedCityObjectRef>> {
        let mut stmt = sqlite_result(self.index.conn.prepare(
            r"
            SELECT
                c.id,
                c.external_id,
                c.cityobject_type,
                b.min_x,
                b.max_x,
                b.min_y,
                b.max_y,
                b.min_z,
                b.max_z
            FROM cityobjects AS c
            LEFT JOIN cityobject_bbox AS b ON b.cityobject_id = c.id
            WHERE c.external_id = ?1
            ORDER BY c.id
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(params![external_id], cityobject_ref_from_row))?;
        sqlite_result(rows.collect())
    }

    /// Returns package refs containing the given `CityObject` occurrence.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` lookup fails.
    pub fn package_refs_for_cityobject(
        &self,
        cityobject: &IndexedCityObjectRef,
    ) -> Result<Vec<IndexedPackageRef>> {
        let mut stmt = sqlite_result(self.index.conn.prepare(
            r"
            SELECT DISTINCT
                p.id,
                p.model_id,
                p.package_type,
                b.min_x,
                b.max_x,
                b.min_y,
                b.max_y,
                b.min_z,
                b.max_z
            FROM package_cityobjects AS pc
            JOIN packages AS p ON p.id = pc.package_id
            LEFT JOIN package_bbox AS b ON b.package_id = p.id
            WHERE pc.cityobject_id = ?1
            ORDER BY p.id
            ",
        ))?;
        let rows =
            sqlite_result(stmt.query_map(params![cityobject.record_id], package_ref_from_row))?;
        sqlite_result(rows.collect())
    }

    /// Returns every distinct package containing a `CityObject` with `external_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup or package reconstruction fails.
    pub fn get_packages(&self, external_id: &str) -> Result<Vec<CityModel>> {
        self.get_packages_with_metadata(external_id)
            .map(|items| items.into_iter().map(|(_, model)| model).collect())
    }

    /// Returns every distinct package containing `external_id` with its source metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup, metadata loading, or package reconstruction fails.
    pub fn get_packages_with_metadata(
        &self,
        external_id: &str,
    ) -> Result<Vec<(Arc<Meta>, CityModel)>> {
        let cityobjects = self.lookup_cityobject_refs(external_id)?;
        let mut seen = BTreeSet::new();
        let mut packages = Vec::new();
        for cityobject in &cityobjects {
            for package in self.package_refs_for_cityobject(cityobject)? {
                if seen.insert(package.record_id) {
                    packages.push(package);
                }
            }
        }
        packages.sort_by_key(|package| package.record_id);
        self.read_packages(&packages).map(|items| {
            items
                .into_iter()
                .map(|item| (item.metadata, item.model))
                .collect()
        })
    }

    /// Reads every package containing the given `CityObject` occurrence.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup or package reconstruction fails.
    pub fn read_cityobject_packages(
        &self,
        cityobject: &IndexedCityObjectRef,
    ) -> Result<Vec<IndexedPackage>> {
        let packages = self.package_refs_for_cityobject(cityobject)?;
        self.read_packages(&packages)
    }

    /// Returns a page of package refs ordered by package record id.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` lookup fails.
    pub fn package_ref_page(&self, offset: usize, limit: usize) -> Result<Vec<IndexedPackageRef>> {
        let mut stmt = sqlite_result(self.index.conn.prepare(
            r"
            SELECT
                p.id,
                p.model_id,
                p.package_type,
                b.min_x,
                b.max_x,
                b.min_y,
                b.max_y,
                b.min_z,
                b.max_z
            FROM packages AS p
            LEFT JOIN package_bbox AS b ON b.package_id = p.id
            ORDER BY p.id
            LIMIT ?2 OFFSET ?1
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(params![offset, limit], package_ref_from_row))?;
        sqlite_result(rows.collect())
    }

    /// Returns package refs whose package bbox intersects `bbox`.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` query fails.
    pub fn query_package_refs(&self, bbox: &BBox) -> Result<Vec<IndexedPackageRef>> {
        let mut stmt = sqlite_result(self.index.conn.prepare(
            r"
            SELECT DISTINCT
                p.id,
                p.model_id,
                p.package_type,
                b.min_x,
                b.max_x,
                b.min_y,
                b.max_y,
                b.min_z,
                b.max_z
            FROM package_bbox AS b
            JOIN packages AS p ON p.id = b.package_id
            WHERE b.min_x <= ?2
              AND b.max_x >= ?1
              AND b.min_y <= ?4
              AND b.max_y >= ?3
            ORDER BY p.id
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(
            params![bbox.min_x, bbox.max_x, bbox.min_y, bbox.max_y],
            package_ref_from_row,
        ))?;
        sqlite_result(rows.collect())
    }

    /// Returns `CityObject` refs whose `CityObject` bbox intersects `bbox`.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` query fails.
    pub fn query_cityobject_refs(&self, bbox: &BBox) -> Result<Vec<IndexedCityObjectRef>> {
        let mut stmt = sqlite_result(self.index.conn.prepare(
            r"
            SELECT
                c.id,
                c.external_id,
                c.cityobject_type,
                b.min_x,
                b.max_x,
                b.min_y,
                b.max_y,
                b.min_z,
                b.max_z
            FROM cityobject_bbox AS b
            JOIN cityobjects AS c ON c.id = b.cityobject_id
            WHERE b.min_x <= ?2
              AND b.max_x >= ?1
              AND b.min_y <= ?4
              AND b.max_y >= ?3
            ORDER BY c.id
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(
            params![bbox.min_x, bbox.max_x, bbox.min_y, bbox.max_y],
            cityobject_ref_from_row,
        ))?;
        sqlite_result(rows.collect())
    }

    /// Returns descendants of the given `CityObject` occurrence in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns an error if relationship traversal queries fail.
    pub fn descendant_cityobject_refs(
        &self,
        cityobject: &IndexedCityObjectRef,
    ) -> Result<Vec<IndexedCityObjectRef>> {
        let mut descendants = Vec::new();
        let mut seen = BTreeSet::new();
        let mut frontier = vec![cityobject.record_id];
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for parent_id in frontier {
                let mut stmt = sqlite_result(self.index.conn.prepare(
                    r"
                    SELECT
                        c.id,
                        c.external_id,
                        c.cityobject_type,
                        b.min_x,
                        b.max_x,
                        b.min_y,
                        b.max_y,
                        b.min_z,
                        b.max_z
                    FROM cityobject_relationships AS r
                    JOIN cityobjects AS c ON c.id = r.child_cityobject_id
                    LEFT JOIN cityobject_bbox AS b ON b.cityobject_id = c.id
                    WHERE r.parent_cityobject_id = ?1
                    ORDER BY r.child_cityobject_id
                    ",
                ))?;
                let rows =
                    sqlite_result(stmt.query_map(params![parent_id], cityobject_ref_from_row))?;
                for row in rows {
                    let child = sqlite_result(row)?;
                    if seen.insert(child.record_id) {
                        next.push(child.record_id);
                        descendants.push(child);
                    }
                }
            }
            frontier = next;
        }
        Ok(descendants)
    }

    /// Reads one package as a valid `CityJSONFeature` model.
    ///
    /// # Errors
    ///
    /// Returns an error if the package cannot be found, read, or reconstructed.
    pub fn read_package(&self, package: &IndexedPackageRef) -> Result<CityModel> {
        self.read_indexed_package(package)
            .map(|package| package.model)
    }

    /// Reads packages while preserving input order and duplicate refs.
    ///
    /// # Errors
    ///
    /// Returns an error if any package cannot be found, read, or reconstructed.
    pub fn read_packages(&self, packages: &[IndexedPackageRef]) -> Result<Vec<IndexedPackage>> {
        let mut decoded = BTreeMap::new();
        for package in packages {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                decoded.entry(package.record_id)
            {
                entry.insert(self.read_indexed_package(package)?);
            }
        }
        packages
            .iter()
            .map(|package| {
                decoded
                    .get(&package.record_id)
                    .map(clone_indexed_package)
                    .ok_or_else(|| {
                        import_error(format!("package {} was not decoded", package.record_id))
                    })
            })
            .collect()
    }

    /// Reads and filters packages while preserving input order.
    ///
    /// # Errors
    ///
    /// Returns an error if package reconstruction or filtering fails.
    pub fn read_filtered_packages(
        &self,
        packages: &[IndexedPackageRef],
        filter: &PackageFilter,
    ) -> Result<Vec<PackageFilterResult>> {
        self.read_packages(packages)?
            .into_iter()
            .map(|package| filter.apply(&package.model))
            .collect()
    }

    /// Looks up a package ref by its `SQLite` package record id.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` lookup fails.
    pub fn lookup_package_ref_by_record_id(
        &self,
        record_id: i64,
    ) -> Result<Option<IndexedPackageRef>> {
        self.package_location(record_id)
            .map(|maybe| maybe.map(|location| location.reference))
    }

    /// Reads a package by its `SQLite` package record id.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup or package reconstruction fails.
    pub fn read_package_by_record_id(&self, record_id: i64) -> Result<Option<IndexedPackage>> {
        let Some(reference) = self.lookup_package_ref_by_record_id(record_id)? else {
            return Ok(None);
        };
        self.read_indexed_package(&reference).map(Some)
    }

    fn read_indexed_package(&self, package: &IndexedPackageRef) -> Result<IndexedPackage> {
        let location = self.package_location(package.record_id)?.ok_or_else(|| {
            import_error(format!(
                "package record {} was not found",
                package.record_id
            ))
        })?;
        let metadata = self.index.get_cached_metadata(location.source_id)?;
        let model = match location.reference.package_type {
            PackageType::CityJson => {
                self.read_cityjson_package(&location, Arc::clone(&metadata.bytes))?
            }
            PackageType::CityJsonSeq | PackageType::FeatureFiles => {
                let offset = location
                    .offset
                    .ok_or_else(|| import_error("package offset is missing"))?;
                let length = location
                    .length
                    .ok_or_else(|| import_error("package length is missing"))?;
                let bytes = read_exact_range(&location.path, offset, length)?;
                feature_slice_with_preserved_package_id(&bytes, metadata.bytes.as_ref())?
            }
        };
        Ok(IndexedPackage {
            reference: location.reference,
            metadata: metadata.value,
            model,
        })
    }

    fn read_cityjson_package(
        &self,
        location: &PackageLocation,
        metadata_bytes: Arc<[u8]>,
    ) -> Result<CityModel> {
        let feature = self
            .index
            .lookup_feature_refs(&location.reference.model_id)?
            .into_iter()
            .find(|feature| feature.source_id == location.source_id)
            .ok_or_else(|| {
                import_error(format!(
                    "CityJSON package {} no longer has a feature reference",
                    location.reference.record_id
                ))
            })?;
        self.backend
            .read_one(&feature.to_location(), metadata_bytes)
    }

    fn package_location(&self, record_id: i64) -> Result<Option<PackageLocation>> {
        sqlite_result(
            self.index
                .conn
                .query_row(
                    r"
                    SELECT
                        p.id,
                        p.model_id,
                        p.package_type,
                        b.min_x,
                        b.max_x,
                        b.min_y,
                        b.max_y,
                        b.min_z,
                        b.max_z,
                        p.source_id,
                        p.path,
                        p.offset,
                        p.length
                    FROM packages AS p
                    LEFT JOIN package_bbox AS b ON b.package_id = p.id
                    WHERE p.id = ?1
                    ",
                    params![record_id],
                    |row| {
                        let reference = package_ref_from_row(row)?;
                        let source_id = row.get::<_, i64>(9)?;
                        let path = PathBuf::from(row.get::<_, String>(10)?);
                        let offset = row.get::<_, Option<i64>>(11)?.map(i64_to_u64).transpose()?;
                        let length = row.get::<_, Option<i64>>(12)?.map(i64_to_u64).transpose()?;
                        Ok(PackageLocation {
                            reference,
                            source_id,
                            path,
                            offset,
                            length,
                        })
                    },
                )
                .optional(),
        )
    }

    /// Returns a lightweight feature reference for a feature id.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub fn lookup_feature_ref(&self, id: &str) -> Result<Option<IndexedFeatureRef>> {
        self.index.lookup_feature_ref(id)
    }

    /// Returns all lightweight feature references for a feature id.
    ///
    /// Results are ordered by the internal feature row id.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub fn lookup_feature_refs(&self, id: &str) -> Result<Vec<IndexedFeatureRef>> {
        self.index.lookup_feature_refs(id)
    }

    /// Returns a lightweight feature reference for a feature row id.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub fn lookup_feature_ref_by_rowid(&self, row_id: i64) -> Result<Option<IndexedFeatureRef>> {
        self.index.lookup_feature_ref_by_rowid(row_id)
    }

    /// Returns cached metadata for a source id.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata lookup fails.
    pub fn metadata_for_source(&self, source_id: i64) -> Result<Arc<Meta>> {
        self.index
            .get_cached_metadata(source_id)
            .map(|metadata| metadata.value)
    }

    /// Returns every feature intersecting the given bounding box.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query(&self, bbox: &BBox) -> Result<Vec<CityModel>> {
        self.query_iter(bbox)?
            .collect::<std::result::Result<Vec<_>, _>>()
    }

    /// Returns every feature intersecting the given bounding box together with
    /// the source metadata used to reconstruct it.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query_with_metadata(&self, bbox: &BBox) -> Result<Vec<(Arc<Meta>, CityModel)>> {
        self.query_iter_with_metadata(bbox)?
            .collect::<std::result::Result<Vec<_>, _>>()
    }

    /// Returns an iterator over features intersecting the given bounding box.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed.
    pub fn query_iter(&self, bbox: &BBox) -> Result<impl Iterator<Item = Result<CityModel>> + '_> {
        let iter = self.query_iter_with_metadata(bbox)?;
        Ok(iter.map(|item| item.map(|(_, model)| model)))
    }

    /// Returns an iterator over features intersecting the given bounding box
    /// together with their feature identifiers.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed.
    pub fn query_iter_with_ids(
        &self,
        bbox: &BBox,
    ) -> Result<impl Iterator<Item = Result<(String, CityModel)>> + '_> {
        let locations = self.index.lookup_bbox_iter(*bbox);
        Ok(locations.map(move |loc| {
            let loc = loc?;
            let feature_id = loc.feature_id.clone();
            let metadata = self.index.get_cached_metadata(loc.source_id)?;
            let model = self.backend.read_one(&loc, Arc::clone(&metadata.bytes))?;
            Ok((feature_id, model))
        }))
    }

    /// Returns an iterator over features intersecting the given bounding box
    /// together with the source metadata used to reconstruct them.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed.
    pub fn query_iter_with_metadata(
        &self,
        bbox: &BBox,
    ) -> Result<impl Iterator<Item = Result<(Arc<Meta>, CityModel)>> + '_> {
        let locations = self.index.lookup_bbox_iter(*bbox);
        Ok(locations.map(move |loc| {
            let loc = loc?;
            let metadata = self.index.get_cached_metadata(loc.source_id)?;
            let model = self.backend.read_one(&loc, Arc::clone(&metadata.bytes))?;
            Ok((metadata.value, model))
        }))
    }

    /// Returns every feature in the index.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed.
    pub fn iter_all(&self) -> Result<impl Iterator<Item = Result<CityModel>> + '_> {
        let iter = self.iter_all_with_metadata()?;
        Ok(iter.map(|item| item.map(|(_, model)| model)))
    }

    /// Returns every feature in the index together with its feature identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed.
    pub fn iter_all_with_ids(
        &self,
    ) -> Result<impl Iterator<Item = Result<(String, CityModel)>> + '_> {
        let iter = self.index.lookup_all_iter();
        Ok(iter.map(move |loc| {
            let loc = loc?;
            let feature_id = loc.location.feature_id.clone();
            let metadata = self.index.get_cached_metadata(loc.location.source_id)?;
            let model = self
                .backend
                .read_one(&loc.location, Arc::clone(&metadata.bytes))?;
            Ok((feature_id, model))
        }))
    }

    /// Returns every feature in the index together with the source metadata used
    /// to reconstruct it.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed.
    pub fn iter_all_with_metadata(
        &self,
    ) -> Result<impl Iterator<Item = Result<(Arc<Meta>, CityModel)>> + '_> {
        let iter = self.index.lookup_all_iter();
        Ok(iter.map(move |loc| {
            let loc = loc?;
            let metadata = self.index.get_cached_metadata(loc.location.source_id)?;
            let model = self
                .backend
                .read_one(&loc.location, Arc::clone(&metadata.bytes))?;
            Ok((metadata.value, model))
        }))
    }

    /// Returns every indexed feature as a page of lightweight references.
    ///
    /// Each page is ordered by the internal feature row id and can be used for
    /// caller-managed parallel decoding.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed or `page_size`
    /// is zero.
    pub fn iter_all_feature_ref_pages(
        &self,
        page_size: usize,
    ) -> Result<impl Iterator<Item = Result<Vec<IndexedFeatureRef>>> + '_> {
        self.index.lookup_all_ref_page_iter(page_size)
    }

    /// Returns every indexed feature as a page of lightweight references.
    ///
    /// This is a semantic alias of [`CityIndex::iter_all_feature_ref_pages`]
    /// for callers that care primarily about bbox-oriented processing.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed or `page_size`
    /// is zero.
    pub fn iter_all_bbox_pages(
        &self,
        page_size: usize,
    ) -> Result<impl Iterator<Item = Result<Vec<IndexedFeatureRef>>> + '_> {
        self.index.lookup_all_ref_page_iter(page_size)
    }

    /// Returns every indexed feature together with its decoded payload in
    /// ascending feature row id order.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed.
    pub fn scan_features(&self) -> Result<impl Iterator<Item = Result<IndexedFeature>> + '_> {
        let ref_pages = self
            .index
            .lookup_all_ref_page_iter(DEFAULT_SCAN_PAGE_SIZE)?;
        Ok(AllIndexedFeatureIter::new(AllIndexedFeaturePageIter {
            city_index: self,
            ref_pages,
        }))
    }

    /// Returns every indexed feature as decoded pages in ascending feature row
    /// id order.
    ///
    /// Each page preserves the row order from
    /// [`CityIndex::iter_all_feature_ref_pages`] and reconstructs its payloads
    /// with the grouped batch path.
    ///
    /// # Errors
    ///
    /// Returns an error if the iterator cannot be constructed or `page_size`
    /// is zero.
    pub fn scan_feature_pages(
        &self,
        page_size: usize,
    ) -> Result<impl Iterator<Item = Result<Vec<IndexedFeature>>> + '_> {
        let ref_pages = self.index.lookup_all_ref_page_iter(page_size)?;
        Ok(AllIndexedFeaturePageIter {
            city_index: self,
            ref_pages,
        })
    }

    /// Returns aggregate bounds and feature count for the whole index.
    ///
    /// Returns `Ok(None)` when the index contains no features.
    ///
    /// # Errors
    ///
    /// Returns an error if the aggregate query fails.
    pub fn feature_bounds_summary(&self) -> Result<Option<FeatureBoundsSummary>> {
        self.index.feature_bounds_summary()
    }

    /// Reconstructs a single indexed feature from a lightweight reference.
    ///
    /// # Errors
    ///
    /// Returns an error if the feature cannot be reconstructed.
    pub fn read_feature(&self, feature: &IndexedFeatureRef) -> Result<CityModel> {
        let metadata = self.index.get_cached_metadata(feature.source_id)?;
        self.backend
            .read_one(&feature.to_location(), Arc::clone(&metadata.bytes))
    }

    /// Reconstructs multiple indexed features from lightweight references.
    ///
    /// # Errors
    ///
    /// Returns an error if any feature cannot be reconstructed.
    pub fn read_features(&self, features: &[IndexedFeatureRef]) -> Result<Vec<CityModel>> {
        self.read_feature_models(features)
    }

    /// Reconstructs and filters multiple indexed features while preserving the
    /// input order.
    ///
    /// # Errors
    ///
    /// Returns an error if any feature cannot be reconstructed or filtered.
    pub fn read_filtered_features(
        &self,
        features: &[IndexedFeatureRef],
        filter: &FeatureFilter,
    ) -> Result<Vec<FilteredFeature>> {
        self.read_feature_models(features)?
            .iter()
            .map(|model| filter.apply(model))
            .collect()
    }

    /// Reconstructs multiple indexed features with their references.
    ///
    /// The returned features preserve the input order.
    ///
    /// # Errors
    ///
    /// Returns an error if any feature cannot be reconstructed.
    pub fn read_indexed_features(
        &self,
        features: &[IndexedFeatureRef],
    ) -> Result<Vec<IndexedFeature>> {
        let models = self.read_feature_models(features)?;
        Ok(features
            .iter()
            .cloned()
            .zip(models)
            .map(|(reference, model)| IndexedFeature { reference, model })
            .collect())
    }

    /// Reconstructs a feature by row id.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup or reconstruction fails.
    pub fn read_feature_by_rowid(&self, row_id: i64) -> Result<Option<IndexedFeature>> {
        let Some(reference) = self.lookup_feature_ref_by_rowid(row_id)? else {
            return Ok(None);
        };
        let model = self.read_feature(&reference)?;
        Ok(Some(IndexedFeature { reference, model }))
    }

    /// Reconstructs features by row id while preserving the input order.
    ///
    /// Missing row ids are returned as `None`.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup or reconstruction fails.
    pub fn read_features_by_rowids(&self, row_ids: &[i64]) -> Result<Vec<Option<IndexedFeature>>> {
        let references = self.index.lookup_feature_refs_by_rowids(row_ids)?;
        let present_references = references.iter().flatten().cloned().collect::<Vec<_>>();
        let mut present_features = self.read_indexed_features(&present_references)?.into_iter();
        let mut features = Vec::with_capacity(row_ids.len());
        for reference in references {
            if reference.is_some() {
                let feature = present_features.next().ok_or_else(|| {
                    import_error("feature reconstruction returned fewer models than references")
                })?;
                features.push(Some(feature));
            } else {
                features.push(None);
            }
        }
        Ok(features)
    }

    /// Reconstructs a page of features after a row id.
    ///
    /// Passing `None` starts from the first row. Results are ordered by row id.
    ///
    /// # Errors
    ///
    /// Returns an error if lookup or reconstruction fails, or `limit` is zero.
    pub fn read_feature_range_after_rowid(
        &self,
        after_row_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<IndexedFeature>> {
        if limit == 0 {
            return Err(import_error("limit must be greater than zero"));
        }
        let refs = self
            .index
            .lookup_all_ref_page(after_row_id, limit)?
            .into_iter()
            .map(|record| record.feature)
            .collect::<Vec<_>>();
        self.read_indexed_features(&refs)
    }

    /// Returns the total number of indexed packages.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be read from the index.
    pub fn package_count(&self) -> Result<usize> {
        self.index.package_count()
    }

    /// Returns the total number of indexed feature aliases kept for legacy sidecars.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be read from the index.
    pub fn feature_ref_count(&self) -> Result<usize> {
        self.index.feature_count()
    }

    /// Returns the total number of indexed sources.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be read from the index.
    pub fn source_count(&self) -> Result<usize> {
        self.index.source_count()
    }

    /// Returns the total number of indexed `CityObjects`.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be read from the index.
    pub fn cityobject_count(&self) -> Result<usize> {
        self.index.cityobject_count()
    }

    /// Returns a contiguous page of indexed feature references.
    ///
    /// The page is ordered by the underlying feature row identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the page cannot be read from the index.
    pub fn feature_ref_page(&self, offset: usize, limit: usize) -> Result<Vec<IndexedFeatureRef>> {
        self.index.lookup_all_ref_page_window(offset, limit)
    }

    /// Returns the raw indexed feature bytes for the given feature identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the index lookup or byte-range read fails.
    pub fn get_bytes(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let Some(loc) = self.index.lookup_id(id)? else {
            return Ok(None);
        };
        read_exact_range(&loc.source_path, loc.offset, loc.length).map(Some)
    }

    /// Returns the raw bytes for a feature reference.
    ///
    /// # Errors
    ///
    /// Returns an error if the feature bytes cannot be read from disk.
    pub fn read_feature_bytes(&self, feature: &IndexedFeatureRef) -> Result<Vec<u8>> {
        read_exact_range(&feature.source_path, feature.offset, feature.length)
    }

    /// Returns cached metadata entries.
    ///
    /// # Errors
    ///
    /// Returns an error if metadata lookup fails.
    pub fn metadata(&self) -> Result<Vec<Arc<Meta>>> {
        self.index.metadata()
    }

    fn read_feature_models(&self, features: &[IndexedFeatureRef]) -> Result<Vec<CityModel>> {
        let mut features_by_source: BTreeMap<i64, Vec<(usize, FeatureLocation)>> = BTreeMap::new();
        for (index, feature) in features.iter().enumerate() {
            features_by_source
                .entry(feature.source_id)
                .or_default()
                .push((index, feature.to_location()));
        }

        let mut models = std::iter::repeat_with(|| None)
            .take(features.len())
            .collect::<Vec<Option<CityModel>>>();
        for (source_id, indexed_locations) in features_by_source {
            let metadata = self.index.get_cached_metadata(source_id)?;
            let locations = indexed_locations
                .iter()
                .map(|(_, location)| location)
                .collect::<Vec<_>>();
            let source_models = self
                .backend
                .read_many(&locations, Arc::clone(&metadata.bytes))?;
            for ((index, _), model) in indexed_locations.into_iter().zip(source_models) {
                models[index] = Some(model);
            }
        }

        Ok(models
            .into_iter()
            .map(|model| model.expect("every input feature should have a decoded model"))
            .collect())
    }
}

type Meta = serde_json::Value;

#[derive(Clone)]
struct CachedMetadata {
    value: Arc<Meta>,
    bytes: Arc<[u8]>,
}

struct Index {
    conn: rusqlite::Connection,
    metadata_cache: Mutex<HashMap<i64, CachedMetadata>>,
}

struct FeatureLocation {
    feature_id: String,
    source_id: i64,
    source_path: PathBuf,
    offset: u64,
    length: u64,
    vertices_offset: Option<u64>,
    vertices_length: Option<u64>,
    member_ranges_json: Option<String>,
}

struct PackageLocation {
    reference: IndexedPackageRef,
    source_id: i64,
    path: PathBuf,
    offset: Option<u64>,
    length: Option<u64>,
}

struct IndexedFeatureLocation {
    row_id: i64,
    location: FeatureLocation,
}

struct IndexedFeatureRefLocation {
    row_id: i64,
    feature: IndexedFeatureRef,
}

struct FeatureIndexEntry {
    id: String,
    source_id: i64,
    path: PathBuf,
    file_size: u64,
    file_mtime_ns: i64,
    offset: u64,
    length: u64,
    bounds: FeatureBounds,
    spatial: bool,
    cityobject_count: u64,
    member_ranges_json: Option<String>,
}

struct IndexedSourceRecord {
    path: PathBuf,
    source_size: Option<u64>,
    source_mtime_ns: Option<i64>,
}

struct IndexedFeaturePathRecord {
    path: PathBuf,
    file_size: Option<u64>,
    file_mtime_ns: Option<i64>,
}

struct BBoxLocationIter<'a> {
    index: &'a Index,
    bbox: BBox,
    last_row_id: Option<i64>,
    page: std::vec::IntoIter<IndexedFeatureLocation>,
    finished: bool,
}

struct AllLocationIter<'a> {
    index: &'a Index,
    last_row_id: Option<i64>,
    page: std::vec::IntoIter<IndexedFeatureLocation>,
    finished: bool,
}

struct AllFeatureRefPageIter<'a> {
    index: &'a Index,
    page_size: usize,
    last_row_id: Option<i64>,
    finished: bool,
}

struct AllIndexedFeaturePageIter<'a> {
    city_index: &'a CityIndex,
    ref_pages: AllFeatureRefPageIter<'a>,
}

struct AllIndexedFeatureIter<'a> {
    pages: AllIndexedFeaturePageIter<'a>,
    page: std::vec::IntoIter<IndexedFeature>,
    finished: bool,
}

impl<'a> BBoxLocationIter<'a> {
    const PAGE_SIZE: usize = 512;

    fn new(index: &'a Index, bbox: BBox) -> Self {
        Self {
            index,
            bbox,
            last_row_id: None,
            page: Vec::new().into_iter(),
            finished: false,
        }
    }

    fn next_location(&mut self) -> Result<Option<FeatureLocation>> {
        if self.finished {
            return Ok(None);
        }

        if let Some(feature) = self.page.next() {
            self.last_row_id = Some(feature.row_id);
            return Ok(Some(feature.location));
        }

        let page = self
            .index
            .lookup_bbox_page(&self.bbox, self.last_row_id, Self::PAGE_SIZE)?;
        if page.is_empty() {
            self.finished = true;
            return Ok(None);
        }

        self.page = page.into_iter();
        let feature = self
            .page
            .next()
            .expect("non-empty page should yield at least one feature");
        self.last_row_id = Some(feature.row_id);
        Ok(Some(feature.location))
    }
}

impl<'a> AllLocationIter<'a> {
    const PAGE_SIZE: usize = 512;

    fn new(index: &'a Index) -> Self {
        Self {
            index,
            last_row_id: None,
            page: Vec::new().into_iter(),
            finished: false,
        }
    }

    fn next_location(&mut self) -> Result<Option<IndexedFeatureLocation>> {
        if self.finished {
            return Ok(None);
        }

        if let Some(feature) = self.page.next() {
            self.last_row_id = Some(feature.row_id);
            return Ok(Some(feature));
        }

        let page = self
            .index
            .lookup_all_page(self.last_row_id, Self::PAGE_SIZE)?;
        if page.is_empty() {
            self.finished = true;
            return Ok(None);
        }

        self.page = page.into_iter();
        let feature = self
            .page
            .next()
            .expect("non-empty page should yield at least one feature");
        self.last_row_id = Some(feature.row_id);
        Ok(Some(feature))
    }
}

impl<'a> AllFeatureRefPageIter<'a> {
    fn new(index: &'a Index, page_size: usize) -> Result<Self> {
        if page_size == 0 {
            return Err(import_error("page_size must be greater than zero"));
        }
        Ok(Self {
            index,
            page_size,
            last_row_id: None,
            finished: false,
        })
    }

    fn next_page(&mut self) -> Result<Option<Vec<IndexedFeatureRef>>> {
        if self.finished {
            return Ok(None);
        }

        let page = self
            .index
            .lookup_all_ref_page(self.last_row_id, self.page_size)?;
        if page.is_empty() {
            self.finished = true;
            return Ok(None);
        }

        self.last_row_id = Some(
            page.last()
                .expect("non-empty page should yield at least one feature")
                .row_id,
        );
        Ok(Some(
            page.into_iter().map(|record| record.feature).collect(),
        ))
    }
}

impl Iterator for BBoxLocationIter<'_> {
    type Item = Result<FeatureLocation>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_location() {
            Ok(Some(feature)) => Some(Ok(feature)),
            Ok(None) => None,
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

impl Iterator for AllLocationIter<'_> {
    type Item = Result<IndexedFeatureLocation>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_location() {
            Ok(Some(feature)) => Some(Ok(feature)),
            Ok(None) => None,
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

impl Iterator for AllFeatureRefPageIter<'_> {
    type Item = Result<Vec<IndexedFeatureRef>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_page() {
            Ok(Some(page)) => Some(Ok(page)),
            Ok(None) => None,
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

impl Iterator for AllIndexedFeaturePageIter<'_> {
    type Item = Result<Vec<IndexedFeature>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ref_pages
            .next()
            .map(|page| page.and_then(|refs| self.city_index.read_indexed_features(&refs)))
    }
}

impl<'a> AllIndexedFeatureIter<'a> {
    fn new(pages: AllIndexedFeaturePageIter<'a>) -> Self {
        Self {
            pages,
            page: Vec::new().into_iter(),
            finished: false,
        }
    }
}

impl Iterator for AllIndexedFeatureIter<'_> {
    type Item = Result<IndexedFeature>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        loop {
            if let Some(feature) = self.page.next() {
                return Some(Ok(feature));
            }
            match self.pages.next() {
                Some(Ok(page)) => {
                    self.page = page.into_iter();
                }
                Some(Err(error)) => {
                    self.finished = true;
                    return Some(Err(error));
                }
                None => {
                    self.finished = true;
                    return None;
                }
            }
        }
    }
}

impl Index {
    fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let conn = sqlite_result(rusqlite::Connection::open(path))?;
        sqlite_result(conn.execute_batch("PRAGMA foreign_keys = ON;"))?;

        let has_schema_state = Self::table_exists(&conn, "schema_state")?;
        let has_legacy_features = Self::table_exists(&conn, "features")?;
        if has_schema_state {
            let schema_version = Self::schema_version(&conn)?;
            if schema_version > SCHEMA_VERSION {
                return Err(import_error(format!(
                    "unsupported cityjson-index schema version {schema_version}; rebuild with a newer cjindex"
                )));
            }
        }

        Self::create_schema(&conn)?;
        if has_schema_state {
            Self::ensure_schema_state(&conn, 0)?;
        } else if has_legacy_features {
            Self::ensure_schema_state(&conn, 1)?;
        } else {
            Self::ensure_schema_state(&conn, 0)?;
        }

        Self::ensure_duplicate_feature_ids_allowed(&conn)?;
        Self::ensure_member_ranges_column(&conn)?;
        Self::ensure_source_status_columns(&conn)?;
        Self::ensure_feature_status_columns(&conn)?;
        Self::ensure_feature_bounds_columns(&conn)?;

        Ok(Self {
            conn,
            metadata_cache: Mutex::new(HashMap::new()),
        })
    }

    fn create_schema(conn: &rusqlite::Connection) -> Result<()> {
        sqlite_result(conn.execute_batch(
            r"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS schema_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                schema_version INTEGER NOT NULL,
                needs_reindex INTEGER NOT NULL CHECK (needs_reindex IN (0, 1))
            );

            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                metadata TEXT NOT NULL,
                vertices_offset INTEGER,
                vertices_length INTEGER,
                source_size INTEGER NOT NULL DEFAULT 0,
                source_mtime_ns INTEGER NOT NULL DEFAULT 0,
                CHECK ((vertices_offset IS NULL) = (vertices_length IS NULL))
            );

            CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                package_type TEXT NOT NULL CHECK (package_type IN ('cityjson', 'cityjson-seq', 'feature-files')),
                model_id TEXT NOT NULL,
                path TEXT NOT NULL,
                offset INTEGER,
                length INTEGER,
                file_size INTEGER NOT NULL,
                file_mtime_ns INTEGER NOT NULL,
                CHECK ((offset IS NULL) = (length IS NULL))
            );

            CREATE TABLE IF NOT EXISTS cityobjects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                external_id TEXT NOT NULL,
                cityobject_type TEXT NOT NULL,
                path TEXT NOT NULL,
                offset INTEGER NOT NULL,
                length INTEGER NOT NULL,
                CHECK (offset >= 0),
                CHECK (length > 0),
                UNIQUE (path, offset, length)
            );

            CREATE TABLE IF NOT EXISTS package_cityobjects (
                package_id INTEGER NOT NULL REFERENCES packages(id) ON DELETE CASCADE,
                cityobject_id INTEGER NOT NULL REFERENCES cityobjects(id) ON DELETE CASCADE,
                ordinal INTEGER NOT NULL,
                is_root INTEGER NOT NULL CHECK (is_root IN (0, 1)),
                PRIMARY KEY (package_id, cityobject_id),
                UNIQUE (package_id, ordinal)
            );

            CREATE TABLE IF NOT EXISTS cityobject_relationships (
                parent_cityobject_id INTEGER NOT NULL REFERENCES cityobjects(id) ON DELETE CASCADE,
                child_cityobject_id INTEGER NOT NULL REFERENCES cityobjects(id) ON DELETE CASCADE,
                PRIMARY KEY (parent_cityobject_id, child_cityobject_id),
                CHECK (parent_cityobject_id <> child_cityobject_id)
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS package_bbox USING rtree(
                package_id, min_x, max_x, min_y, max_y, min_z, max_z
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS cityobject_bbox USING rtree(
                cityobject_id, min_x, max_x, min_y, max_y, min_z, max_z
            );

            CREATE INDEX IF NOT EXISTS packages_source_id_idx ON packages(source_id);
            CREATE INDEX IF NOT EXISTS packages_model_id_idx ON packages(model_id);
            CREATE INDEX IF NOT EXISTS cityobjects_external_id_idx ON cityobjects(external_id);
            CREATE INDEX IF NOT EXISTS cityobjects_type_idx ON cityobjects(cityobject_type);
            CREATE INDEX IF NOT EXISTS cityobjects_source_id_idx ON cityobjects(source_id);
            CREATE INDEX IF NOT EXISTS package_cityobjects_cityobject_id_idx
                ON package_cityobjects(cityobject_id);
            CREATE INDEX IF NOT EXISTS cityobject_relationships_child_idx
                ON cityobject_relationships(child_cityobject_id);

            CREATE TABLE IF NOT EXISTS features (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                feature_id TEXT NOT NULL,
                source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                file_size INTEGER,
                file_mtime_ns INTEGER,
                offset INTEGER NOT NULL,
                length INTEGER NOT NULL,
                min_z REAL,
                max_z REAL,
                cityobject_count INTEGER,
                member_ranges TEXT
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS feature_bbox
            USING rtree(feature_rowid, min_x, max_x, min_y, max_y);

            CREATE TABLE IF NOT EXISTS bbox_map (
                feature_rowid INTEGER PRIMARY KEY,
                feature_id TEXT NOT NULL
            );
            ",
        ))?;
        Ok(())
    }

    fn table_exists(conn: &rusqlite::Connection, table: &str) -> Result<bool> {
        let exists = sqlite_result(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1)",
            params![table],
            |row| row.get::<_, i64>(0),
        ))?;
        Ok(exists != 0)
    }

    fn schema_version(conn: &rusqlite::Connection) -> Result<i64> {
        sqlite_result(conn.query_row(
            "SELECT schema_version FROM schema_state WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        ))
    }

    fn ensure_schema_state(conn: &rusqlite::Connection, needs_reindex: i64) -> Result<()> {
        sqlite_result(conn.execute(
            r"
            INSERT INTO schema_state (id, schema_version, needs_reindex)
            VALUES (1, ?1, ?2)
            ON CONFLICT(id) DO UPDATE SET
                schema_version = excluded.schema_version,
                needs_reindex = CASE
                    WHEN schema_state.needs_reindex = 1 THEN 1
                    ELSE excluded.needs_reindex
                END
            ",
            params![SCHEMA_VERSION, needs_reindex],
        ))?;
        Ok(())
    }

    fn rebuild(&mut self, scans: &[SourceScan]) -> Result<()> {
        let tx = sqlite_result(self.conn.transaction())?;
        Self::clear_tables(&tx)?;

        let mut feature_entries = Vec::new();
        for scan in scans {
            let source_id = Self::insert_source_in_tx(
                &tx,
                scan.path.as_path(),
                &scan.metadata,
                scan.vertices_offset,
                scan.vertices_length,
                scan.source_size,
                scan.source_mtime_ns,
            )?;
            for feature in &scan.features {
                feature_entries.push(FeatureIndexEntry {
                    id: feature.id.clone(),
                    source_id,
                    path: feature.path.clone(),
                    file_size: feature.file_size,
                    file_mtime_ns: feature.file_mtime_ns,
                    offset: feature.offset,
                    length: feature.length,
                    bounds: feature.bounds,
                    spatial: feature.spatial,
                    cityobject_count: feature.cityobject_count,
                    member_ranges_json: feature
                        .member_ranges
                        .as_ref()
                        .map(json_string)
                        .transpose()?,
                });
            }
        }
        Self::insert_features_in_tx(&tx, &feature_entries)?;
        Self::insert_normalized_features_in_tx(&tx, &feature_entries)?;
        sqlite_result(tx.execute(
            "UPDATE schema_state SET schema_version = ?1, needs_reindex = 0 WHERE id = 1",
            params![SCHEMA_VERSION],
        ))?;
        sqlite_result(tx.commit())?;

        self.metadata_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }

    fn lookup_id(&self, id: &str) -> Result<Option<FeatureLocation>> {
        sqlite_result(
            self.conn
                .query_row(
                    r"
                SELECT
                    f.feature_id,
                    s.id,
                    f.path,
                    f.offset,
                    f.length,
                    s.vertices_offset,
                    s.vertices_length,
                    f.member_ranges
                FROM features AS f
                JOIN sources AS s ON s.id = f.source_id
                WHERE f.feature_id = ?1
                ORDER BY f.id
                LIMIT 1
                ",
                    params![id],
                    Self::feature_location_from_row,
                )
                .optional(),
        )
    }

    fn lookup_feature_ref(&self, id: &str) -> Result<Option<IndexedFeatureRef>> {
        sqlite_result(
            self.conn
                .query_row(
                    r"
                SELECT
                    f.id,
                    f.feature_id,
                    s.id,
                    f.path,
                    f.offset,
                    f.length,
                    s.vertices_offset,
                    s.vertices_length,
                    f.member_ranges,
                    fb.min_x,
                    fb.max_x,
                    fb.min_y,
                    fb.max_y,
                    f.min_z,
                    f.max_z
                FROM features AS f
                JOIN sources AS s ON s.id = f.source_id
                JOIN feature_bbox AS fb ON fb.feature_rowid = f.id
                WHERE f.feature_id = ?1
                ORDER BY f.id
                LIMIT 1
                ",
                    params![id],
                    Self::indexed_feature_ref_location_from_row,
                )
                .optional()
                .map(|maybe| maybe.map(|record| record.feature)),
        )
    }

    fn lookup_feature_refs(&self, id: &str) -> Result<Vec<IndexedFeatureRef>> {
        let mut stmt = sqlite_result(self.conn.prepare(
            r"
            SELECT
                f.id,
                f.feature_id,
                s.id,
                f.path,
                f.offset,
                f.length,
                s.vertices_offset,
                s.vertices_length,
                f.member_ranges,
                fb.min_x,
                fb.max_x,
                fb.min_y,
                fb.max_y,
                f.min_z,
                f.max_z
            FROM features AS f
            JOIN sources AS s ON s.id = f.source_id
            JOIN feature_bbox AS fb ON fb.feature_rowid = f.id
            WHERE f.feature_id = ?1
            ORDER BY f.id
            ",
        ))?;
        let rows = sqlite_result(
            stmt.query_map(params![id], Self::indexed_feature_ref_location_from_row),
        )?;
        sqlite_result(rows.map(|row| row.map(|record| record.feature)).collect())
    }

    fn lookup_feature_ref_by_rowid(&self, row_id: i64) -> Result<Option<IndexedFeatureRef>> {
        sqlite_result(
            self.conn
                .query_row(
                    r"
                SELECT
                    f.id,
                    f.feature_id,
                    s.id,
                    f.path,
                    f.offset,
                    f.length,
                    s.vertices_offset,
                    s.vertices_length,
                    f.member_ranges,
                    fb.min_x,
                    fb.max_x,
                    fb.min_y,
                    fb.max_y,
                    f.min_z,
                    f.max_z
                FROM features AS f
                JOIN sources AS s ON s.id = f.source_id
                JOIN feature_bbox AS fb ON fb.feature_rowid = f.id
                WHERE f.id = ?1
                ",
                    params![row_id],
                    Self::indexed_feature_ref_location_from_row,
                )
                .optional()
                .map(|maybe| maybe.map(|record| record.feature)),
        )
    }

    fn lookup_feature_refs_by_rowids(
        &self,
        row_ids: &[i64],
    ) -> Result<Vec<Option<IndexedFeatureRef>>> {
        row_ids
            .iter()
            .map(|row_id| self.lookup_feature_ref_by_rowid(*row_id))
            .collect()
    }

    fn lookup_bbox_iter(&self, bbox: BBox) -> BBoxLocationIter<'_> {
        BBoxLocationIter::new(self, bbox)
    }

    fn lookup_all_iter(&self) -> AllLocationIter<'_> {
        AllLocationIter::new(self)
    }

    fn lookup_all_ref_page_iter(&self, page_size: usize) -> Result<AllFeatureRefPageIter<'_>> {
        AllFeatureRefPageIter::new(self, page_size)
    }

    fn lookup_all_ref_page_window(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<IndexedFeatureRef>> {
        let mut stmt = sqlite_result(self.conn.prepare(
            r"
            SELECT
                f.id,
                f.feature_id,
                s.id,
                f.path,
                f.offset,
                f.length,
                s.vertices_offset,
                s.vertices_length,
                f.member_ranges,
                fb.min_x,
                fb.max_x,
                fb.min_y,
                fb.max_y,
                f.min_z,
                f.max_z
            FROM features AS f
            JOIN sources AS s ON s.id = f.source_id
            JOIN feature_bbox AS fb ON fb.feature_rowid = f.id
            ORDER BY f.id
            LIMIT ?2 OFFSET ?1
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(
            params![offset, limit],
            Self::indexed_feature_ref_location_from_row,
        ))?;
        sqlite_result(rows.map(|row| row.map(|record| record.feature)).collect())
    }

    fn lookup_bbox_page(
        &self,
        bbox: &BBox,
        after_row_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<IndexedFeatureLocation>> {
        let mut stmt = sqlite_result(self.conn.prepare(
            r"
            SELECT
                f.id,
                f.feature_id,
                s.id,
                f.path,
                f.offset,
                f.length,
                s.vertices_offset,
                s.vertices_length,
                f.member_ranges
            FROM feature_bbox AS fb
            JOIN bbox_map AS bm ON bm.feature_rowid = fb.feature_rowid
            JOIN features AS f ON f.id = bm.feature_rowid
            JOIN sources AS s ON s.id = f.source_id
            WHERE fb.min_x <= ?2
              AND fb.max_x >= ?1
              AND fb.min_y <= ?4
              AND fb.max_y >= ?3
              AND (?5 IS NULL OR f.id > ?5)
            ORDER BY f.id
            LIMIT ?6
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(
            params![
                bbox.min_x,
                bbox.max_x,
                bbox.min_y,
                bbox.max_y,
                after_row_id,
                limit
            ],
            Self::indexed_feature_location_from_row,
        ))?;
        sqlite_result(rows.collect())
    }

    fn lookup_all_page(
        &self,
        after_row_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<IndexedFeatureLocation>> {
        let sql = match after_row_id {
            Some(_) => {
                r"
                SELECT
                    f.id,
                    f.feature_id,
                    s.id,
                    f.path,
                    f.offset,
                    f.length,
                    s.vertices_offset,
                    s.vertices_length,
                    f.member_ranges
                FROM features AS f
                JOIN sources AS s ON s.id = f.source_id
                WHERE f.id > ?1
                ORDER BY f.id
                LIMIT ?2
                "
            }
            None => {
                r"
                SELECT
                    f.id,
                    f.feature_id,
                    s.id,
                    f.path,
                    f.offset,
                    f.length,
                    s.vertices_offset,
                    s.vertices_length,
                    f.member_ranges
                FROM features AS f
                JOIN sources AS s ON s.id = f.source_id
                ORDER BY f.id
                LIMIT ?1
                "
            }
        };
        let mut stmt = sqlite_result(self.conn.prepare(sql))?;
        let rows = if let Some(after_row_id) = after_row_id {
            sqlite_result(stmt.query_map(
                params![after_row_id, limit],
                Self::indexed_feature_location_from_row,
            ))?
        } else {
            sqlite_result(stmt.query_map(params![limit], Self::indexed_feature_location_from_row))?
        };
        sqlite_result(rows.collect())
    }

    fn lookup_all_ref_page(
        &self,
        after_row_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<IndexedFeatureRefLocation>> {
        let sql = match after_row_id {
            Some(_) => {
                r"
                SELECT
                    f.id,
                    f.feature_id,
                    s.id,
                    f.path,
                    f.offset,
                    f.length,
                    s.vertices_offset,
                    s.vertices_length,
                    f.member_ranges,
                    fb.min_x,
                    fb.max_x,
                    fb.min_y,
                    fb.max_y,
                    f.min_z,
                    f.max_z
                FROM features AS f
                JOIN sources AS s ON s.id = f.source_id
                JOIN feature_bbox AS fb ON fb.feature_rowid = f.id
                WHERE f.id > ?1
                ORDER BY f.id
                LIMIT ?2
                "
            }
            None => {
                r"
                SELECT
                    f.id,
                    f.feature_id,
                    s.id,
                    f.path,
                    f.offset,
                    f.length,
                    s.vertices_offset,
                    s.vertices_length,
                    f.member_ranges,
                    fb.min_x,
                    fb.max_x,
                    fb.min_y,
                    fb.max_y,
                    f.min_z,
                    f.max_z
                FROM features AS f
                JOIN sources AS s ON s.id = f.source_id
                JOIN feature_bbox AS fb ON fb.feature_rowid = f.id
                ORDER BY f.id
                LIMIT ?1
                "
            }
        };
        let mut stmt = sqlite_result(self.conn.prepare(sql))?;
        let rows = if let Some(after_row_id) = after_row_id {
            sqlite_result(stmt.query_map(
                params![after_row_id, limit],
                Self::indexed_feature_ref_location_from_row,
            ))?
        } else {
            sqlite_result(
                stmt.query_map(params![limit], Self::indexed_feature_ref_location_from_row),
            )?
        };
        sqlite_result(rows.collect())
    }

    fn get_cached_metadata(&self, source_id: i64) -> Result<CachedMetadata> {
        if let Some(metadata) = self
            .metadata_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&source_id)
            .cloned()
        {
            return Ok(metadata);
        }

        let metadata_json: String = sqlite_result(self.conn.query_row(
            "SELECT metadata FROM sources WHERE id = ?1",
            params![source_id],
            |row| row.get(0),
        ))?;
        let metadata: Meta = parse_json_str(&metadata_json)?;
        let metadata = CachedMetadata {
            value: Arc::new(metadata),
            bytes: Arc::from(metadata_json.into_bytes()),
        };

        self.metadata_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(source_id, metadata.clone());

        Ok(metadata)
    }

    fn get_metadata(&self, source_id: i64) -> Result<Arc<Meta>> {
        self.get_cached_metadata(source_id)
            .map(|metadata| metadata.value)
    }

    fn metadata(&self) -> Result<Vec<Arc<Meta>>> {
        let mut stmt = sqlite_result(self.conn.prepare("SELECT id FROM sources ORDER BY id"))?;
        let rows = sqlite_result(stmt.query_map([], |row| row.get::<_, i64>(0)))?;
        let source_ids = sqlite_result(rows.collect::<rusqlite::Result<Vec<_>>>())?;
        source_ids
            .into_iter()
            .map(|source_id| self.get_metadata(source_id))
            .collect()
    }

    fn source_count(&self) -> Result<usize> {
        self.query_count("SELECT COUNT(*) FROM sources")
    }

    fn feature_count(&self) -> Result<usize> {
        self.query_count("SELECT COUNT(*) FROM features")
    }

    fn cityobject_count(&self) -> Result<usize> {
        let total = sqlite_result(self.conn.query_row(
            "SELECT COALESCE(SUM(cityobject_count), 0) FROM features",
            [],
            |row| row.get::<_, i64>(0),
        ))?;
        usize::try_from(total)
            .map_err(|_| import_error("indexed CityObject count does not fit in usize"))
    }

    fn current_schema_version(&self) -> Result<i64> {
        Self::schema_version(&self.conn)
    }

    fn package_count(&self) -> Result<usize> {
        self.query_count("SELECT COUNT(*) FROM packages")
    }

    fn normalized_cityobject_count(&self) -> Result<usize> {
        self.query_count("SELECT COUNT(*) FROM cityobjects")
    }

    fn cityobject_relationship_count(&self) -> Result<usize> {
        self.query_count("SELECT COUNT(*) FROM cityobject_relationships")
    }

    fn query_count(&self, sql: &str) -> Result<usize> {
        let count = sqlite_result(self.conn.query_row(sql, [], |row| row.get::<_, i64>(0)))?;
        usize::try_from(count).map_err(|_| import_error("count does not fit in usize"))
    }

    fn feature_bounds_summary(&self) -> Result<Option<FeatureBoundsSummary>> {
        let summary = sqlite_result(self.conn.query_row(
            r"
            SELECT
                COUNT(*),
                MIN(fb.min_x),
                MAX(fb.max_x),
                MIN(fb.min_y),
                MAX(fb.max_y),
                MIN(f.min_z),
                MAX(f.max_z)
            FROM features AS f
            JOIN feature_bbox AS fb ON fb.feature_rowid = f.id
            ",
            [],
            |row| {
                let count = row.get::<_, i64>(0)?;
                if count == 0 {
                    return Ok(None);
                }
                let feature_count = usize::try_from(count).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?;
                Ok(Some(FeatureBoundsSummary {
                    bounds: FeatureBounds {
                        min_x: row.get::<_, f64>(1)?,
                        max_x: row.get::<_, f64>(2)?,
                        min_y: row.get::<_, f64>(3)?,
                        max_y: row.get::<_, f64>(4)?,
                        min_z: row.get::<_, f64>(5)?,
                        max_z: row.get::<_, f64>(6)?,
                    },
                    feature_count,
                }))
            },
        ))?;
        Ok(summary)
    }

    fn indexed_sources(&self) -> Result<Vec<IndexedSourceRecord>> {
        let mut stmt = sqlite_result(self.conn.prepare(
            r"
            SELECT path, source_size, source_mtime_ns
            FROM sources
            ORDER BY path
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map([], |row| {
            Ok(IndexedSourceRecord {
                path: PathBuf::from(row.get::<_, String>(0)?),
                source_size: row.get::<_, Option<i64>>(1)?.map(i64_to_u64).transpose()?,
                source_mtime_ns: row.get::<_, Option<i64>>(2)?,
            })
        }))?;
        sqlite_result(rows.collect())
    }

    fn indexed_feature_paths(&self) -> Result<Vec<IndexedFeaturePathRecord>> {
        let mut stmt = sqlite_result(self.conn.prepare(
            r"
            SELECT DISTINCT path, file_size, file_mtime_ns
            FROM features
            ORDER BY path
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map([], |row| {
            Ok(IndexedFeaturePathRecord {
                path: PathBuf::from(row.get::<_, String>(0)?),
                file_size: row.get::<_, Option<i64>>(1)?.map(i64_to_u64).transpose()?,
                file_mtime_ns: row.get::<_, Option<i64>>(2)?,
            })
        }))?;
        sqlite_result(rows.collect())
    }

    fn ensure_member_ranges_column(conn: &rusqlite::Connection) -> Result<()> {
        let mut stmt = sqlite_result(conn.prepare("PRAGMA table_info(features)"))?;
        let rows = sqlite_result(stmt.query_map([], |row| row.get::<_, String>(1)))?;
        let columns = sqlite_result(rows.collect::<rusqlite::Result<Vec<_>>>())?;
        if !columns.iter().any(|column| column == "member_ranges") {
            sqlite_result(conn.execute("ALTER TABLE features ADD COLUMN member_ranges TEXT", []))?;
        }
        Ok(())
    }

    fn ensure_source_status_columns(conn: &rusqlite::Connection) -> Result<()> {
        let mut stmt = sqlite_result(conn.prepare("PRAGMA table_info(sources)"))?;
        let rows = sqlite_result(stmt.query_map([], |row| row.get::<_, String>(1)))?;
        let columns = sqlite_result(rows.collect::<rusqlite::Result<Vec<_>>>())?;
        if !columns.iter().any(|column| column == "source_size") {
            sqlite_result(conn.execute("ALTER TABLE sources ADD COLUMN source_size INTEGER", []))?;
        }
        if !columns.iter().any(|column| column == "source_mtime_ns") {
            sqlite_result(
                conn.execute("ALTER TABLE sources ADD COLUMN source_mtime_ns INTEGER", []),
            )?;
        }
        Ok(())
    }

    fn ensure_feature_status_columns(conn: &rusqlite::Connection) -> Result<()> {
        let mut stmt = sqlite_result(conn.prepare("PRAGMA table_info(features)"))?;
        let rows = sqlite_result(stmt.query_map([], |row| row.get::<_, String>(1)))?;
        let columns = sqlite_result(rows.collect::<rusqlite::Result<Vec<_>>>())?;
        if !columns.iter().any(|column| column == "file_size") {
            sqlite_result(conn.execute("ALTER TABLE features ADD COLUMN file_size INTEGER", []))?;
        }
        if !columns.iter().any(|column| column == "file_mtime_ns") {
            sqlite_result(
                conn.execute("ALTER TABLE features ADD COLUMN file_mtime_ns INTEGER", []),
            )?;
        }
        if !columns.iter().any(|column| column == "cityobject_count") {
            sqlite_result(conn.execute(
                "ALTER TABLE features ADD COLUMN cityobject_count INTEGER",
                [],
            ))?;
        }
        Ok(())
    }

    fn ensure_feature_bounds_columns(conn: &rusqlite::Connection) -> Result<()> {
        let mut stmt = sqlite_result(conn.prepare("PRAGMA table_info(features)"))?;
        let rows = sqlite_result(stmt.query_map([], |row| row.get::<_, String>(1)))?;
        let columns = sqlite_result(rows.collect::<rusqlite::Result<Vec<_>>>())?;
        if !columns.iter().any(|column| column == "min_z") {
            sqlite_result(conn.execute("ALTER TABLE features ADD COLUMN min_z REAL", []))?;
        }
        if !columns.iter().any(|column| column == "max_z") {
            sqlite_result(conn.execute("ALTER TABLE features ADD COLUMN max_z REAL", []))?;
        }
        Ok(())
    }

    fn feature_bounds_complete(&self) -> Result<bool> {
        let missing = sqlite_result(self.conn.query_row(
            "SELECT COUNT(*) FROM features WHERE min_z IS NULL OR max_z IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        ))?;
        Ok(missing == 0)
    }

    fn ensure_duplicate_feature_ids_allowed(conn: &rusqlite::Connection) -> Result<()> {
        if table_sql_contains(conn, "features", "feature_id TEXT NOT NULL UNIQUE")?
            || table_sql_contains(conn, "bbox_map", "feature_id TEXT NOT NULL UNIQUE")?
        {
            sqlite_result(conn.execute_batch(
                r"
                PRAGMA foreign_keys = OFF;

                DROP TABLE IF EXISTS bbox_map_new;
                CREATE TABLE bbox_map_new (
                    feature_rowid INTEGER PRIMARY KEY,
                    feature_id TEXT NOT NULL
                );
                INSERT INTO bbox_map_new (feature_rowid, feature_id)
                SELECT feature_rowid, feature_id FROM bbox_map;
                DROP TABLE bbox_map;
                ALTER TABLE bbox_map_new RENAME TO bbox_map;

                DROP TABLE IF EXISTS features_new;
                CREATE TABLE features_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    feature_id TEXT NOT NULL,
                    source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                    path TEXT NOT NULL,
                    file_size INTEGER,
                    file_mtime_ns INTEGER,
                    offset INTEGER NOT NULL,
                    length INTEGER NOT NULL,
                    min_z REAL,
                    max_z REAL,
                    cityobject_count INTEGER,
                    member_ranges TEXT
                );
                INSERT INTO features_new (
                    id,
                    feature_id,
                    source_id,
                    path,
                    file_size,
                    file_mtime_ns,
                    offset,
                    length,
                    min_z,
                    max_z,
                    cityobject_count,
                    member_ranges
                )
                SELECT
                    id,
                    feature_id,
                    source_id,
                    path,
                    file_size,
                    file_mtime_ns,
                    offset,
                    length,
                    min_z,
                    max_z,
                    cityobject_count,
                    member_ranges
                FROM features;
                DROP TABLE features;
                ALTER TABLE features_new RENAME TO features;

                PRAGMA foreign_keys = ON;
                ",
            ))?;
        }
        Ok(())
    }

    fn clear_tables(tx: &rusqlite::Transaction<'_>) -> Result<()> {
        sqlite_result(tx.execute_batch(
            r"
            DELETE FROM cityobject_relationships;
            DELETE FROM package_cityobjects;
            DELETE FROM cityobject_bbox;
            DELETE FROM package_bbox;
            DELETE FROM cityobjects;
            DELETE FROM packages;
            DELETE FROM bbox_map;
            DELETE FROM feature_bbox;
            DELETE FROM features;
            DELETE FROM sources;
            ",
        ))?;
        Ok(())
    }

    fn insert_source_in_tx(
        tx: &rusqlite::Transaction<'_>,
        path: &Path,
        meta: &Meta,
        vertices_offset: Option<u64>,
        vertices_length: Option<u64>,
        source_size: u64,
        source_mtime_ns: i64,
    ) -> Result<i64> {
        let metadata_json = json_string(meta)?;
        let vertices_offset = sqlite_result(vertices_offset.map(u64_to_i64).transpose())?;
        let vertices_length = sqlite_result(vertices_length.map(u64_to_i64).transpose())?;
        let source_size = sqlite_result(u64_to_i64(source_size))?;
        sqlite_result(tx.execute(
            r"
            INSERT INTO sources (
                path,
                metadata,
                vertices_offset,
                vertices_length,
                source_size,
                source_mtime_ns
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                path.to_string_lossy(),
                metadata_json,
                vertices_offset,
                vertices_length,
                source_size,
                source_mtime_ns,
            ],
        ))?;
        Ok(tx.last_insert_rowid())
    }

    fn insert_features_in_tx(
        tx: &rusqlite::Transaction<'_>,
        entries: &[FeatureIndexEntry],
    ) -> Result<()> {
        let mut feature_stmt = sqlite_result(tx.prepare(
            r"
            INSERT INTO features (
                feature_id,
                source_id,
                path,
                file_size,
                file_mtime_ns,
                offset,
                length,
                min_z,
                max_z,
                cityobject_count,
                member_ranges
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
        ))?;
        let mut bbox_stmt = sqlite_result(tx.prepare(
            r"
            INSERT INTO feature_bbox (feature_rowid, min_x, max_x, min_y, max_y)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
        ))?;
        let mut map_stmt = sqlite_result(tx.prepare(
            r"
            INSERT INTO bbox_map (feature_rowid, feature_id)
            VALUES (?1, ?2)
            ",
        ))?;
        for entry in entries {
            let file_size = sqlite_result(u64_to_i64(entry.file_size))?;
            let offset = sqlite_result(u64_to_i64(entry.offset))?;
            let length = sqlite_result(u64_to_i64(entry.length))?;
            let cityobject_count = sqlite_result(u64_to_i64(entry.cityobject_count))?;
            sqlite_result(feature_stmt.execute(params![
                &entry.id,
                entry.source_id,
                entry.path.to_string_lossy(),
                file_size,
                entry.file_mtime_ns,
                offset,
                length,
                entry.bounds.min_z,
                entry.bounds.max_z,
                cityobject_count,
                &entry.member_ranges_json,
            ]))?;
            let feature_rowid = tx.last_insert_rowid();
            sqlite_result(bbox_stmt.execute(params![
                feature_rowid,
                entry.bounds.min_x,
                entry.bounds.max_x,
                entry.bounds.min_y,
                entry.bounds.max_y,
            ]))?;
            sqlite_result(map_stmt.execute(params![feature_rowid, &entry.id]))?;
        }

        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "transactional normalized schema writes are kept together"
    )]
    fn insert_normalized_features_in_tx(
        tx: &rusqlite::Transaction<'_>,
        entries: &[FeatureIndexEntry],
    ) -> Result<()> {
        let mut inserted_physical_packages = BTreeSet::new();
        for entry in entries {
            let package_type = normalized_package_type(entry);
            if package_type != "cityjson"
                && !inserted_physical_packages.insert((
                    entry.path.clone(),
                    entry.offset,
                    entry.length,
                ))
            {
                continue;
            }
            let model_id = normalized_package_model_id(entry, package_type)?;
            let file_size = sqlite_result(u64_to_i64(entry.file_size))?;
            let file_mtime_ns = entry.file_mtime_ns;
            let (package_offset, package_length) = if package_type == "cityjson" {
                (None, None)
            } else {
                (
                    Some(sqlite_result(u64_to_i64(entry.offset))?),
                    Some(sqlite_result(u64_to_i64(entry.length))?),
                )
            };
            sqlite_result(tx.execute(
                r"
                INSERT INTO packages (
                    source_id,
                    package_type,
                    model_id,
                    path,
                    offset,
                    length,
                    file_size,
                    file_mtime_ns
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    entry.source_id,
                    package_type,
                    &model_id,
                    entry.path.to_string_lossy(),
                    package_offset,
                    package_length,
                    file_size,
                    file_mtime_ns,
                ],
            ))?;
            let package_id = tx.last_insert_rowid();
            if entry.spatial {
                insert_package_bbox(tx, package_id, entry.bounds)?;
            }

            let objects = normalized_cityobjects_for_entry(entry)?;
            validate_normalized_relationships(&objects)?;
            let root_external_id = entry.id.as_str();
            let mut object_ids_by_external_id = BTreeMap::new();

            for (ordinal, object) in objects.iter().enumerate() {
                let cityobject_id = insert_or_lookup_cityobject(tx, entry.source_id, object)?;
                object_ids_by_external_id.insert(object.external_id.clone(), cityobject_id);
                sqlite_result(tx.execute(
                    r"
                    INSERT OR IGNORE INTO package_cityobjects (
                        package_id,
                        cityobject_id,
                        ordinal,
                        is_root
                    )
                    VALUES (?1, ?2, ?3, ?4)
                    ",
                    params![
                        package_id,
                        cityobject_id,
                        i64::try_from(ordinal).map_err(|_| {
                            import_error("package CityObject ordinal does not fit in i64")
                        })?,
                        i64::from(object.external_id == root_external_id),
                    ],
                ))?;
                if entry.spatial {
                    insert_cityobject_bbox(tx, cityobject_id, entry.bounds)?;
                }
            }

            for object in &objects {
                let parent_id = object_ids_by_external_id
                    .get(&object.external_id)
                    .copied()
                    .ok_or_else(|| {
                        import_error(format!(
                            "CityObject {} was not inserted",
                            object.external_id
                        ))
                    })?;
                for child_external_id in &object.children {
                    let child_id = object_ids_by_external_id
                        .get(child_external_id)
                        .copied()
                        .ok_or_else(|| {
                            import_error(format!("missing relationship target {child_external_id}"))
                        })?;
                    if parent_id == child_id {
                        return Err(import_error(format!(
                            "relationship cycle includes {}",
                            object.external_id
                        )));
                    }
                    sqlite_result(tx.execute(
                        r"
                        INSERT OR IGNORE INTO cityobject_relationships (
                            parent_cityobject_id,
                            child_cityobject_id
                        )
                        VALUES (?1, ?2)
                        ",
                        params![parent_id, child_id],
                    ))?;
                }
            }
        }
        Ok(())
    }

    fn feature_location_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FeatureLocation> {
        Self::feature_location_from_row_offset(row, 0)
    }

    fn feature_location_from_row_offset(
        row: &rusqlite::Row<'_>,
        col: usize,
    ) -> rusqlite::Result<FeatureLocation> {
        let feature_id = row.get::<_, String>(col)?;
        let source_id = row.get::<_, i64>(col + 1)?;
        let source_path = PathBuf::from(row.get::<_, String>(col + 2)?);
        let offset = i64_to_u64(row.get::<_, i64>(col + 3)?)?;
        let length = i64_to_u64(row.get::<_, i64>(col + 4)?)?;
        let vertices_offset = match row.get::<_, Option<i64>>(col + 5)? {
            Some(value) => Some(i64_to_u64(value)?),
            None => None,
        };
        let vertices_length = match row.get::<_, Option<i64>>(col + 6)? {
            Some(value) => Some(i64_to_u64(value)?),
            None => None,
        };
        let member_ranges_json = row.get::<_, Option<String>>(col + 7)?;

        Ok(FeatureLocation {
            feature_id,
            source_id,
            source_path,
            offset,
            length,
            vertices_offset,
            vertices_length,
            member_ranges_json,
        })
    }

    fn indexed_feature_location_from_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<IndexedFeatureLocation> {
        let row_id = row.get::<_, i64>(0)?;
        let location = Self::feature_location_from_row_offset(row, 1)?;
        Ok(IndexedFeatureLocation { row_id, location })
    }

    fn indexed_feature_ref_location_from_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<IndexedFeatureRefLocation> {
        let row_id = row.get::<_, i64>(0)?;
        let feature_id = row.get::<_, String>(1)?;
        let source_id = row.get::<_, i64>(2)?;
        let source_path = PathBuf::from(row.get::<_, String>(3)?);
        let offset = i64_to_u64(row.get::<_, i64>(4)?)?;
        let length = i64_to_u64(row.get::<_, i64>(5)?)?;
        let vertices_offset = match row.get::<_, Option<i64>>(6)? {
            Some(value) => Some(i64_to_u64(value)?),
            None => None,
        };
        let vertices_length = match row.get::<_, Option<i64>>(7)? {
            Some(value) => Some(i64_to_u64(value)?),
            None => None,
        };
        let member_ranges_json = row.get::<_, Option<String>>(8)?;
        let bounds = FeatureBounds {
            min_x: row.get::<_, f64>(9)?,
            max_x: row.get::<_, f64>(10)?,
            min_y: row.get::<_, f64>(11)?,
            max_y: row.get::<_, f64>(12)?,
            min_z: row.get::<_, f64>(13)?,
            max_z: row.get::<_, f64>(14)?,
        };

        Ok(IndexedFeatureRefLocation {
            row_id,
            feature: IndexedFeatureRef {
                row_id,
                feature_id,
                source_id,
                source_path,
                offset,
                length,
                vertices_offset,
                vertices_length,
                member_ranges_json,
                bounds,
            },
        })
    }
}

#[derive(Debug)]
struct NormalizedCityObjectScan {
    external_id: String,
    cityobject_type: String,
    path: PathBuf,
    offset: u64,
    length: u64,
    children: BTreeSet<String>,
}

fn normalized_package_type(entry: &FeatureIndexEntry) -> &'static str {
    if entry.member_ranges_json.is_some() {
        "cityjson"
    } else if entry.offset == 0 && entry.length == entry.file_size {
        "feature-files"
    } else {
        "cityjson-seq"
    }
}

fn normalized_package_model_id(entry: &FeatureIndexEntry, package_type: &str) -> Result<String> {
    if package_type == "cityjson" {
        return Ok(entry.id.clone());
    }
    let bytes = read_exact_range(&entry.path, entry.offset, entry.length)?;
    let feature: Value = parse_json_slice(&bytes)?;
    feature
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            import_error(format!(
                "package {} is missing CityJSONFeature id",
                entry.path.display()
            ))
        })
}

fn normalized_cityobjects_for_entry(
    entry: &FeatureIndexEntry,
) -> Result<Vec<NormalizedCityObjectScan>> {
    if let Some(member_ranges_json) = &entry.member_ranges_json {
        let member_ranges: Vec<IndexedObjectRange> = parse_json_str(member_ranges_json)?;
        return member_ranges
            .into_iter()
            .map(|member| {
                let fragment = read_exact_range(&entry.path, member.offset, member.length)?;
                let (external_id, object) = parse_cityobject_entry(&fragment)?;
                let cityobject_type = cityobject_type(&object, &external_id)?;
                let children = normalized_children(&external_id, &object)?;
                Ok(NormalizedCityObjectScan {
                    external_id,
                    cityobject_type,
                    path: entry.path.clone(),
                    offset: member.offset,
                    length: member.length,
                    children,
                })
            })
            .collect();
    }

    let feature_bytes = read_exact_range(&entry.path, entry.offset, entry.length)?;
    let feature: Value = parse_json_slice(&feature_bytes)?;
    let cityobjects = feature
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| import_error(format!("feature {} is missing CityObjects", entry.id)))?;
    let mut objects = Vec::with_capacity(cityobjects.len());
    for (ordinal, (external_id, object)) in cityobjects.iter().enumerate() {
        let cityobject_type = cityobject_type(object, external_id)?;
        let children = normalized_children(external_id, object)?;
        objects.push(NormalizedCityObjectScan {
            external_id: external_id.clone(),
            cityobject_type,
            path: entry.path.clone(),
            offset: entry.offset
                + u64::try_from(ordinal)
                    .map_err(|_| import_error("CityObject ordinal does not fit in u64"))?,
            length: 1,
            children,
        });
    }
    objects.sort_by(|left, right| left.external_id.cmp(&right.external_id));
    Ok(objects)
}

fn cityobject_type(object: &Value, external_id: &str) -> Result<String> {
    object
        .get("type")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| import_error(format!("CityObject {external_id} is missing type")))
}

fn normalized_children(external_id: &str, object: &Value) -> Result<BTreeSet<String>> {
    let mut children = BTreeSet::new();
    if let Some(values) = object.get("children").and_then(Value::as_array) {
        for value in values {
            let child = value.as_str().ok_or_else(|| {
                import_error(format!(
                    "CityObject {external_id} child reference is not a string"
                ))
            })?;
            children.insert(child.to_owned());
        }
    }
    Ok(children)
}

fn validate_normalized_relationships(objects: &[NormalizedCityObjectScan]) -> Result<()> {
    let ids = objects
        .iter()
        .map(|object| object.external_id.as_str())
        .collect::<BTreeSet<_>>();
    let graph = objects
        .iter()
        .map(|object| {
            for child in &object.children {
                if !ids.contains(child.as_str()) {
                    return Err(import_error(format!("missing relationship target {child}")));
                }
            }
            Ok((
                object.external_id.as_str(),
                object
                    .children
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure_relationship_graph_acyclic(&graph)
}

fn ensure_relationship_graph_acyclic(graph: &BTreeMap<&str, Vec<&str>>) -> Result<()> {
    fn visit<'a>(
        node: &'a str,
        graph: &BTreeMap<&'a str, Vec<&'a str>>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> Result<()> {
        if visited.contains(node) {
            return Ok(());
        }
        if !visiting.insert(node) {
            return Err(import_error(format!("relationship cycle includes {node}")));
        }
        if let Some(children) = graph.get(node) {
            for child in children {
                visit(child, graph, visiting, visited)?;
            }
        }
        visiting.remove(node);
        visited.insert(node);
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for node in graph.keys() {
        visit(node, graph, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn insert_or_lookup_cityobject(
    tx: &rusqlite::Transaction<'_>,
    source_id: i64,
    object: &NormalizedCityObjectScan,
) -> Result<i64> {
    sqlite_result(tx.execute(
        r"
        INSERT OR IGNORE INTO cityobjects (
            source_id,
            external_id,
            cityobject_type,
            path,
            offset,
            length
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            source_id,
            &object.external_id,
            &object.cityobject_type,
            object.path.to_string_lossy(),
            sqlite_result(u64_to_i64(object.offset))?,
            sqlite_result(u64_to_i64(object.length))?,
        ],
    ))?;
    sqlite_result(tx.query_row(
        "SELECT id FROM cityobjects WHERE path = ?1 AND offset = ?2 AND length = ?3",
        params![
            object.path.to_string_lossy(),
            sqlite_result(u64_to_i64(object.offset))?,
            sqlite_result(u64_to_i64(object.length))?,
        ],
        |row| row.get::<_, i64>(0),
    ))
}

fn insert_package_bbox(
    tx: &rusqlite::Transaction<'_>,
    package_id: i64,
    bounds: FeatureBounds,
) -> Result<()> {
    sqlite_result(tx.execute(
        r"
        INSERT OR REPLACE INTO package_bbox (
            package_id, min_x, max_x, min_y, max_y, min_z, max_z
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            package_id,
            bounds.min_x,
            bounds.max_x,
            bounds.min_y,
            bounds.max_y,
            bounds.min_z,
            bounds.max_z,
        ],
    ))?;
    Ok(())
}

fn insert_cityobject_bbox(
    tx: &rusqlite::Transaction<'_>,
    cityobject_id: i64,
    bounds: FeatureBounds,
) -> Result<()> {
    sqlite_result(tx.execute(
        r"
        INSERT OR REPLACE INTO cityobject_bbox (
            cityobject_id, min_x, max_x, min_y, max_y, min_z, max_z
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            cityobject_id,
            bounds.min_x,
            bounds.max_x,
            bounds.min_y,
            bounds.max_y,
            bounds.min_z,
            bounds.max_z,
        ],
    ))?;
    Ok(())
}

trait StorageBackend: Send + Sync {
    fn scan(&self, worker_count: usize) -> Result<Vec<SourceScan>>;
    fn read_one(&self, loc: &FeatureLocation, metadata_bytes: Arc<[u8]>) -> Result<CityModel>;
    fn read_many(
        &self,
        locations: &[&FeatureLocation],
        metadata_bytes: Arc<[u8]>,
    ) -> Result<Vec<CityModel>> {
        locations
            .iter()
            .map(|loc| self.read_one(loc, Arc::clone(&metadata_bytes)))
            .collect()
    }
}

struct SourceScan {
    path: PathBuf,
    metadata: Meta,
    vertices_offset: Option<u64>,
    vertices_length: Option<u64>,
    source_size: u64,
    source_mtime_ns: i64,
    features: Vec<ScannedFeature>,
}

struct ScannedFeature {
    id: String,
    path: PathBuf,
    file_size: u64,
    file_mtime_ns: i64,
    offset: u64,
    length: u64,
    bounds: FeatureBounds,
    spatial: bool,
    cityobject_count: u64,
    member_ranges: Option<Vec<IndexedObjectRange>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IndexedObjectRange {
    id: String,
    offset: u64,
    length: u64,
}

struct LocalizedFeatureParts {
    feature_id: String,
    cityobjects: Vec<LocalizedFeatureObject>,
    vertices: Vec<[i64; 3]>,
}

struct LocalizedFeatureObject {
    id: String,
    object_json: Box<RawValue>,
}

struct NdjsonBackend {
    paths: Vec<PathBuf>,
}

impl StorageBackend for NdjsonBackend {
    fn scan(&self, worker_count: usize) -> Result<Vec<SourceScan>> {
        let paths = collect_layout_files(&self.paths, ".jsonl")?;
        parallel_scan_items(&paths, worker_count, |path| {
            scan_ndjson_source(path.as_path())
        })
    }

    fn read_one(&self, loc: &FeatureLocation, metadata_bytes: Arc<[u8]>) -> Result<CityModel> {
        let bytes = read_exact_range(&loc.source_path, loc.offset, loc.length)?;
        feature_slice_with_indexed_id(&bytes, loc, metadata_bytes.as_ref())
    }

    fn read_many(
        &self,
        locations: &[&FeatureLocation],
        metadata_bytes: Arc<[u8]>,
    ) -> Result<Vec<CityModel>> {
        read_feature_slices_with_base(locations, metadata_bytes.as_ref())
    }
}

struct CityJsonBackend {
    paths: Vec<PathBuf>,
    vertices_cache: Mutex<LruCache<PathBuf, Arc<Vec<[i64; 3]>>>>,
}

impl CityJsonBackend {
    fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            paths,
            vertices_cache: Mutex::new(LruCache::unbounded()),
        }
    }

    fn load_shared_vertices(
        &self,
        source_path: &Path,
        source_file: &mut fs::File,
        offset: u64,
        length: u64,
    ) -> Result<Arc<Vec<[i64; 3]>>> {
        let mut cache = self
            .vertices_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(vertices) = cache.get(source_path) {
            return Ok(Arc::clone(vertices));
        }

        let vertices_bytes = read_exact_range_from_file(source_file, source_path, offset, length)?;
        let vertices = Arc::new(parse_vertices_fragment(&vertices_bytes)?);
        cache.put(source_path.to_path_buf(), Arc::clone(&vertices));
        Ok(vertices)
    }
}

impl StorageBackend for CityJsonBackend {
    fn scan(&self, worker_count: usize) -> Result<Vec<SourceScan>> {
        let _ = &self.vertices_cache;
        let paths = collect_layout_files(&self.paths, ".city.json")?;
        parallel_scan_items(&paths, worker_count, |path| {
            scan_cityjson_source(path.as_path())
        })
    }

    fn read_one(&self, loc: &FeatureLocation, metadata_bytes: Arc<[u8]>) -> Result<CityModel> {
        let mut source_file = fs::File::open(&loc.source_path)?;
        self.read_one_from_file(loc, metadata_bytes.as_ref(), &mut source_file)
    }

    fn read_many(
        &self,
        locations: &[&FeatureLocation],
        metadata_bytes: Arc<[u8]>,
    ) -> Result<Vec<CityModel>> {
        let mut locations_by_path: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
        for (index, location) in locations.iter().enumerate() {
            locations_by_path
                .entry(location.source_path.clone())
                .or_default()
                .push(index);
        }

        let mut models = std::iter::repeat_with(|| None)
            .take(locations.len())
            .collect::<Vec<Option<CityModel>>>();
        for (path, mut indexes) in locations_by_path {
            indexes.sort_by_key(|index| locations[*index].offset);
            let mut source_file = fs::File::open(&path)?;
            for index in indexes {
                let model = self.read_one_from_file(
                    locations[index],
                    metadata_bytes.as_ref(),
                    &mut source_file,
                )?;
                models[index] = Some(model);
            }
        }

        Ok(models
            .into_iter()
            .map(|model| model.expect("every input feature should have a decoded model"))
            .collect())
    }
}

impl CityJsonBackend {
    fn read_one_from_file(
        &self,
        loc: &FeatureLocation,
        metadata_bytes: &[u8],
        source_file: &mut fs::File,
    ) -> Result<CityModel> {
        let vertices_offset = loc.vertices_offset.ok_or_else(|| {
            Error::UnsupportedFeature(
                "regular CityJSON reads require an indexed shared vertices range".into(),
            )
        })?;
        let vertices_length = loc.vertices_length.ok_or_else(|| {
            Error::UnsupportedFeature(
                "regular CityJSON reads require an indexed shared vertices range".into(),
            )
        })?;

        let member_ranges = loc
            .member_ranges_json
            .as_deref()
            .map(parse_json_str::<Vec<IndexedObjectRange>>)
            .transpose()?
            .unwrap_or_else(|| {
                vec![IndexedObjectRange {
                    id: loc.feature_id.clone(),
                    offset: loc.offset,
                    length: loc.length,
                }]
            });
        let mut object_entries = Vec::with_capacity(member_ranges.len());
        for member_range in &member_ranges {
            let object_fragment = read_exact_range_from_file(
                source_file,
                &loc.source_path,
                member_range.offset,
                member_range.length,
            )?;
            let (object_id, object_value) = parse_cityobject_entry(&object_fragment)?;
            if object_id != member_range.id {
                return Err(import_error(format!(
                    "indexed CityJSON member {} resolved to fragment for {}",
                    member_range.id, object_id
                )));
            }
            object_entries.push((object_id, object_value));
        }
        let shared_vertices = self.load_shared_vertices(
            &loc.source_path,
            source_file,
            vertices_offset,
            vertices_length,
        )?;
        let feature_parts =
            build_feature_parts(&loc.feature_id, object_entries, shared_vertices.as_ref())?;
        let cityobjects = feature_parts
            .cityobjects
            .iter()
            .map(|cityobject| staged::FeatureObjectFragment {
                id: cityobject.id.as_str(),
                object: cityobject.object_json.as_ref(),
            })
            .collect::<Vec<_>>();
        let assembly = staged::FeatureAssembly {
            id: feature_parts.feature_id.as_str(),
            cityobjects: &cityobjects,
            vertices: &feature_parts.vertices,
        };

        staged::from_feature_assembly_with_base(assembly, metadata_bytes)
    }
}

struct FeatureFilesBackend {
    root: PathBuf,
    metadata_glob: GlobMatcher,
    feature_glob: GlobMatcher,
}

struct FeatureFileSourcePlan {
    path: PathBuf,
    metadata: Meta,
    source_size: u64,
    source_mtime_ns: i64,
    feature_paths: Vec<PathBuf>,
}

struct FeatureFileScanItem<'a> {
    source_index: usize,
    metadata: &'a Meta,
    path: &'a Path,
}

impl FeatureFilesBackend {
    fn new(root: PathBuf, metadata_glob: &str, feature_glob: &str) -> Self {
        let metadata_glob = globset::Glob::new(metadata_glob)
            .expect("metadata glob must be valid")
            .compile_matcher();
        let feature_glob = globset::Glob::new(feature_glob)
            .expect("feature glob must be valid")
            .compile_matcher();
        Self {
            root,
            metadata_glob,
            feature_glob,
        }
    }
}

impl StorageBackend for FeatureFilesBackend {
    fn scan(&self, worker_count: usize) -> Result<Vec<SourceScan>> {
        scan_feature_files_root(
            &self.root,
            &self.metadata_glob,
            &self.feature_glob,
            worker_count,
        )
    }

    fn read_one(&self, loc: &FeatureLocation, metadata_bytes: Arc<[u8]>) -> Result<CityModel> {
        let feature_bytes = read_exact_range(&loc.source_path, loc.offset, loc.length)?;
        feature_slice_with_indexed_id(&feature_bytes, loc, metadata_bytes.as_ref())
    }

    fn read_many(
        &self,
        locations: &[&FeatureLocation],
        metadata_bytes: Arc<[u8]>,
    ) -> Result<Vec<CityModel>> {
        read_feature_slices_with_base(locations, metadata_bytes.as_ref())
    }
}

fn scan_feature_files_root(
    root: &Path,
    metadata_glob: &GlobMatcher,
    feature_glob: &GlobMatcher,
    worker_count: usize,
) -> Result<Vec<SourceScan>> {
    let plans = discover_feature_file_sources(root, metadata_glob, feature_glob)?;
    let mut sources = plans
        .iter()
        .map(|plan| SourceScan {
            path: plan.path.clone(),
            metadata: plan.metadata.clone(),
            vertices_offset: None,
            vertices_length: None,
            source_size: plan.source_size,
            source_mtime_ns: plan.source_mtime_ns,
            features: Vec::with_capacity(plan.feature_paths.len()),
        })
        .collect::<Vec<_>>();
    let scan_items = plans
        .iter()
        .enumerate()
        .flat_map(|(source_index, plan)| {
            plan.feature_paths
                .iter()
                .map(move |path| FeatureFileScanItem {
                    source_index,
                    metadata: &plan.metadata,
                    path: path.as_path(),
                })
        })
        .collect::<Vec<_>>();
    let features = parallel_scan_items(&scan_items, worker_count, scan_feature_file)?;
    for (source_index, features) in features {
        sources[source_index].features.extend(features);
    }
    Ok(sources)
}

fn discover_feature_file_sources(
    root: &Path,
    metadata_glob: &GlobMatcher,
    feature_glob: &GlobMatcher,
) -> Result<Vec<FeatureFileSourcePlan>> {
    let mut metadata_files = Vec::new();
    let mut feature_files = Vec::new();

    for entry in WalkBuilder::new(root)
        .hidden(false)
        .follow_links(true)
        .build()
    {
        let entry = entry.map_err(|error| import_error(error.to_string()))?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        if entry.metadata().is_ok_and(|meta| meta.len() == 0) {
            continue;
        }
        let path = entry.into_path();
        let rel = path.strip_prefix(root).unwrap_or(path.as_path());
        if metadata_glob.is_match(rel) {
            metadata_files.push(path);
        } else if feature_glob.is_match(rel) {
            feature_files.push(path);
        }
    }

    metadata_files.sort();
    feature_files.sort();

    if metadata_files.is_empty() {
        return Err(import_error(format!(
            "feature-files root {} does not contain any metadata files",
            root.display()
        )));
    }

    let mut metadata_by_dir = BTreeMap::new();
    let mut sources = BTreeMap::new();

    for metadata_path in metadata_files {
        let metadata: Meta = read_json(&metadata_path)?;
        let (source_size, source_mtime_ns) = file_status(&metadata_path)?;
        let parent = metadata_path.parent().unwrap_or(root).to_path_buf();
        metadata_by_dir.insert(parent, metadata_path.clone());
        sources.insert(
            metadata_path.clone(),
            FeatureFileSourcePlan {
                path: metadata_path,
                metadata,
                source_size,
                source_mtime_ns,
                feature_paths: Vec::new(),
            },
        );
    }

    for feature_path in feature_files {
        let metadata_path = resolve_feature_metadata_path(root, &feature_path, &metadata_by_dir)
            .ok_or_else(|| {
                import_error(format!(
                    "no ancestor metadata file found for feature {}",
                    feature_path.display()
                ))
            })?;
        let source = sources.get_mut(&metadata_path).ok_or_else(|| {
            import_error(format!(
                "feature {} resolved to missing metadata source {}",
                feature_path.display(),
                metadata_path.display()
            ))
        })?;
        source.feature_paths.push(feature_path);
    }

    Ok(sources.into_values().collect())
}

fn scan_feature_file(item: &FeatureFileScanItem<'_>) -> Result<(usize, Vec<ScannedFeature>)> {
    let feature: Value = read_json(item.path)?;
    let (ids, bounds, spatial, cityobject_count) =
        parse_feature_file_bounds(&feature, item.metadata)?;
    let (file_size, file_mtime_ns) = file_status(item.path)?;
    let features = ids
        .into_iter()
        .map(|id| ScannedFeature {
            id,
            path: item.path.to_path_buf(),
            file_size,
            file_mtime_ns,
            offset: 0,
            length: file_size,
            bounds,
            spatial,
            cityobject_count,
            member_ranges: None,
        })
        .collect();
    Ok((item.source_index, features))
}

fn resolve_feature_metadata_path(
    root: &Path,
    feature_path: &Path,
    metadata_by_dir: &BTreeMap<PathBuf, PathBuf>,
) -> Option<PathBuf> {
    let mut current = feature_path.parent();
    while let Some(dir) = current {
        if let Some(metadata_path) = metadata_by_dir.get(dir) {
            return Some(metadata_path.clone());
        }
        if dir == root {
            break;
        }
        current = dir.parent();
    }
    None
}

fn parse_feature_file_bounds(
    feature: &Value,
    metadata: &Meta,
) -> Result<(Vec<String>, FeatureBounds, bool, u64)> {
    let ids = feature_cityobject_keys(feature, "feature file")?;
    let vertices = feature
        .get("vertices")
        .cloned()
        .ok_or_else(|| import_error("feature file is missing vertices"))?;
    let vertices: Vec<[i64; 3]> = parse_json_value(vertices)?;

    let referenced_vertices = collect_feature_vertex_indices(feature, vertices.len())?;
    let (scale, translate) = parse_ndjson_transform(metadata)?;
    let (bounds, spatial) = if referenced_vertices.is_empty() {
        (empty_feature_bounds(), false)
    } else {
        (
            feature_bounds_from_vertices(&vertices, &referenced_vertices, scale, translate)?,
            true,
        )
    };
    let cityobject_count = feature_cityobject_count(feature, "feature file")?;
    Ok((ids, bounds, spatial, cityobject_count))
}

fn trim_fragment_delimiters(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();

    while start < end && (bytes[start].is_ascii_whitespace() || bytes[start] == b',') {
        start += 1;
    }
    while end > start && (bytes[end - 1].is_ascii_whitespace() || bytes[end - 1] == b',') {
        end -= 1;
    }

    &bytes[start..end]
}

fn parse_cityobject_entry(fragment: &[u8]) -> Result<(String, Value)> {
    let fragment = trim_fragment_delimiters(fragment);
    if fragment.is_empty() {
        return Err(import_error("CityObject entry fragment is empty"));
    }

    let mut wrapped = Vec::with_capacity(fragment.len() + 2);
    wrapped.push(b'{');
    wrapped.extend_from_slice(fragment);
    wrapped.push(b'}');

    let entry: Map<String, Value> = parse_json_slice(&wrapped)?;
    if entry.len() != 1 {
        return Err(import_error(
            "CityObject entry fragment must contain exactly one object entry",
        ));
    }

    let (object_id, object_value) = entry
        .into_iter()
        .next()
        .ok_or_else(|| import_error("CityObject entry fragment is empty"))?;
    if !object_value.is_object() {
        return Err(import_error("CityObject entry value must be a JSON object"));
    }

    Ok((object_id, object_value))
}

fn parse_vertices_fragment(fragment: &[u8]) -> Result<Vec<[i64; 3]>> {
    let fragment = trim_fragment_delimiters(fragment);
    if fragment.is_empty() {
        return Err(import_error("shared vertices fragment is empty"));
    }
    parse_json_slice(fragment)
}

fn build_feature_parts(
    feature_id: &str,
    mut object_entries: Vec<(String, Value)>,
    shared_vertices: &[[i64; 3]],
) -> Result<LocalizedFeatureParts> {
    let retained_ids = object_entries
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();

    for (_, object_value) in &mut object_entries {
        filter_local_relationships(object_value, &retained_ids)?;
    }

    let mut referenced_vertices = BTreeSet::new();
    for (_, object_value) in &object_entries {
        collect_object_vertex_indices(object_value, &mut referenced_vertices)?;
    }

    let local_vertices = build_local_vertices(shared_vertices, &referenced_vertices)?;
    let remap = referenced_vertices
        .iter()
        .enumerate()
        .map(|(new_index, old_index)| (*old_index, new_index))
        .collect::<HashMap<_, _>>();

    for (_, object_value) in &mut object_entries {
        if let Some(geometries) = object_value
            .as_object_mut()
            .and_then(|object| object.get_mut("geometry"))
            .and_then(Value::as_array_mut)
        {
            for geometry in geometries {
                if let Some(boundaries) = geometry.get_mut("boundaries") {
                    remap_vertex_indices(boundaries, &remap)?;
                }
            }
        }
    }

    let cityobjects = object_entries
        .into_iter()
        .map(|(id, object_value)| {
            let object_json = RawValue::from_string(json_string(&object_value)?)
                .map_err(|error| import_error(error.to_string()))?;
            Ok(LocalizedFeatureObject { id, object_json })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(LocalizedFeatureParts {
        feature_id: feature_id.to_owned(),
        cityobjects,
        vertices: local_vertices,
    })
}

fn filter_local_relationships(
    object_value: &mut Value,
    retained_ids: &BTreeSet<String>,
) -> Result<()> {
    let object = object_value
        .as_object_mut()
        .ok_or_else(|| import_error("CityObject value must be a JSON object"))?;

    for key in ["children", "parents"] {
        let remove_key = match object.get_mut(key) {
            Some(value) => {
                let refs = value
                    .as_array_mut()
                    .ok_or_else(|| import_error(format!("{key} must be an array")))?;
                refs.retain(|entry| {
                    entry
                        .as_str()
                        .is_some_and(|object_id| retained_ids.contains(object_id))
                });
                refs.is_empty()
            }
            None => false,
        };

        if remove_key {
            object.remove(key);
        }
    }

    Ok(())
}

fn collect_vertex_indices(value: &Value, indices: &mut BTreeSet<usize>) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_vertex_indices(item, indices)?;
            }
            Ok(())
        }
        Value::Number(number) => {
            indices.insert(number_to_index(number)?);
            Ok(())
        }
        Value::Null => Ok(()),
        other => Err(import_error(format!(
            "boundary values must be arrays or non-negative integers, found {}",
            value_kind(other)
        ))),
    }
}

fn remap_vertex_indices(value: &mut Value, remap: &HashMap<usize, usize>) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                remap_vertex_indices(item, remap)?;
            }
            Ok(())
        }
        Value::Number(number) => {
            let old_index = number_to_index(number)?;
            let new_index = remap.get(&old_index).copied().ok_or_else(|| {
                import_error(format!(
                    "missing remap entry for referenced vertex index {old_index}"
                ))
            })?;
            *value =
                Value::Number(Number::from(u64::try_from(new_index).map_err(|_| {
                    import_error("localized vertex index does not fit in u64")
                })?));
            Ok(())
        }
        Value::Null => Ok(()),
        other => Err(import_error(format!(
            "boundary values must be arrays or non-negative integers, found {}",
            value_kind(other)
        ))),
    }
}

fn build_local_vertices(
    shared_vertices: &[[i64; 3]],
    referenced_vertices: &BTreeSet<usize>,
) -> Result<Vec<[i64; 3]>> {
    let mut vertices = Vec::with_capacity(referenced_vertices.len());

    for &index in referenced_vertices {
        let vertex = shared_vertices.get(index).copied().ok_or_else(|| {
            import_error(format!(
                "vertex index {index} is outside the shared vertices array"
            ))
        })?;
        vertices.push(vertex);
    }

    Ok(vertices)
}

fn number_to_index(number: &Number) -> Result<usize> {
    let index = number
        .as_u64()
        .ok_or_else(|| import_error("boundary vertex indices must be non-negative integers"))?;
    usize::try_from(index)
        .map_err(|_| import_error(format!("vertex index {index} does not fit in usize")))
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn import_error(message: impl Into<String>) -> Error {
    Error::Import(message.into())
}

/// Returns the configured index worker count.
///
/// # Errors
///
/// Returns an error if `CJINDEX_WORKERS` is set to an invalid value.
pub fn configured_worker_count() -> Result<usize> {
    match std::env::var(WORKER_COUNT_ENV) {
        Ok(value) => {
            let worker_count = value.parse::<usize>().map_err(|error| {
                import_error(format!(
                    "{WORKER_COUNT_ENV} must be a positive integer: {error}"
                ))
            })?;
            if worker_count == 0 {
                return Err(import_error(format!(
                    "{WORKER_COUNT_ENV} must be greater than zero"
                )));
            }
            Ok(worker_count)
        }
        Err(std::env::VarError::NotPresent) => {
            Ok(std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get))
        }
        Err(std::env::VarError::NotUnicode(_)) => Err(import_error(format!(
            "{WORKER_COUNT_ENV} must contain valid UTF-8"
        ))),
    }
}

fn parallel_scan_items<T, U, F>(items: &[T], worker_count: usize, scan: F) -> Result<Vec<U>>
where
    T: Sync,
    U: Send,
    F: Fn(&T) -> Result<U> + Sync,
{
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let shard_count = worker_count.max(1).min(items.len());
    if shard_count == 1 {
        return items.iter().map(scan).collect();
    }

    let chunk_size = items.len().div_ceil(shard_count);
    std::thread::scope(|scope| -> Result<Vec<U>> {
        let mut handles = Vec::with_capacity(shard_count);
        let scan = &scan;
        for shard in items.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                let mut shard_results = Vec::with_capacity(shard.len());
                for item in shard {
                    shard_results.push(scan(item)?);
                }
                Ok::<Vec<U>, Error>(shard_results)
            }));
        }

        let mut results = Vec::with_capacity(items.len());
        for handle in handles {
            let shard_results = handle
                .join()
                .map_err(|_| import_error("parallel scan worker panicked"))??;
            results.extend(shard_results);
        }
        Ok(results)
    })
}

fn serde_json_error(error: &serde_json::Error) -> Error {
    import_error(error.to_string())
}

fn parse_json_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|error| serde_json_error(&error))
}

fn parse_json_str<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|error| serde_json_error(&error))
}

fn parse_json_value<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|error| serde_json_error(&error))
}

fn json_string<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|error| serde_json_error(&error))
}

fn table_sql_contains(conn: &rusqlite::Connection, table: &str, needle: &str) -> Result<bool> {
    let sql = sqlite_result(
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional(),
    )?
    .flatten()
    .unwrap_or_default();
    Ok(sql.contains(needle))
}

fn read_exact_range(path: &Path, offset: u64, length: u64) -> Result<Vec<u8>> {
    let mut file = fs::File::open(path)
        .map_err(|error| import_error(format!("failed to open {}: {error}", path.display())))?;
    read_exact_range_from_file(&mut file, path, offset, length)
}

fn feature_slice_with_preserved_package_id(
    feature_bytes: &[u8],
    metadata_bytes: &[u8],
) -> Result<CityModel> {
    let mut feature: Value = parse_json_slice(feature_bytes)?;
    let feature_id = feature
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| import_error("CityJSONFeature package is missing id"))?
        .to_owned();
    let cityobjects = feature
        .get_mut("CityObjects")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| import_error("CityJSONFeature package is missing CityObjects"))?;
    if !cityobjects.contains_key(&feature_id) {
        let children = cityobjects
            .keys()
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>();
        let wrapper_type = cityobjects
            .values()
            .find_map(|object| object.get("type").and_then(Value::as_str))
            .unwrap_or("Building");
        cityobjects.insert(
            feature_id.clone(),
            serde_json::json!({
                "type": wrapper_type,
                "children": children,
            }),
        );
    }
    let bytes = serde_json::to_vec(&feature).map_err(|error| serde_json_error(&error))?;
    staged::from_feature_slice_with_base(&bytes, metadata_bytes)
}

fn read_feature_slices_with_base(
    locations: &[&FeatureLocation],
    metadata_bytes: &[u8],
) -> Result<Vec<CityModel>> {
    let mut locations_by_path: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (index, location) in locations.iter().enumerate() {
        locations_by_path
            .entry(location.source_path.clone())
            .or_default()
            .push(index);
    }

    let mut models = std::iter::repeat_with(|| None)
        .take(locations.len())
        .collect::<Vec<Option<CityModel>>>();
    for (path, mut indexes) in locations_by_path {
        indexes.sort_by_key(|index| locations[*index].offset);
        let mut file = fs::File::open(&path)
            .map_err(|error| import_error(format!("failed to open {}: {error}", path.display())))?;
        for index in indexes {
            let location = locations[index];
            let feature_bytes =
                read_exact_range_from_file(&mut file, &path, location.offset, location.length)?;
            let model = feature_slice_with_indexed_id(&feature_bytes, location, metadata_bytes)?;
            models[index] = Some(model);
        }
    }

    Ok(models
        .into_iter()
        .map(|model| model.expect("every input feature should have a decoded model"))
        .collect())
}

fn feature_slice_with_indexed_id(
    feature_bytes: &[u8],
    loc: &FeatureLocation,
    metadata_bytes: &[u8],
) -> Result<CityModel> {
    staged::from_feature_slice_with_indexed_id_and_base(
        feature_bytes,
        loc.feature_id.as_str(),
        metadata_bytes,
    )
}

fn read_exact_range_from_file(
    file: &mut fs::File,
    path: &Path,
    offset: u64,
    length: u64,
) -> Result<Vec<u8>> {
    let length = usize::try_from(length).map_err(|_| {
        import_error(format!(
            "requested read of {length} bytes from {} exceeds the supported buffer size",
            path.display()
        ))
    })?;
    if length > isize::MAX as usize {
        return Err(import_error(format!(
            "requested read of {length} bytes from {} exceeds the supported buffer size",
            path.display()
        )));
    }

    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|error| {
        import_error(format!(
            "failed to allocate buffer for {} bytes from {}: {error}",
            length,
            path.display()
        ))
    })?;
    bytes.resize(length, 0);

    file.seek(SeekFrom::Start(offset)).map_err(|error| {
        import_error(format!(
            "failed to seek to byte offset {offset} in {}: {error}",
            path.display()
        ))
    })?;
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == ErrorKind::UnexpectedEof {
            import_error(format!(
                "short read while reading {length} bytes at offset {offset} from {}",
                path.display()
            ))
        } else {
            import_error(format!(
                "failed to read {length} bytes at offset {offset} from {}: {error}",
                path.display()
            ))
        }
    })?;

    Ok(bytes)
}

fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    let bytes = fs::read(path.as_ref())?;
    parse_json_slice(&bytes)
}

fn file_status(path: &Path) -> Result<(u64, i64)> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified().map_err(|error| {
        import_error(format!(
            "failed to read modified time for {}: {error}",
            path.display()
        ))
    })?;
    let since_epoch = modified.duration_since(UNIX_EPOCH).map_err(|error| {
        import_error(format!(
            "modified time for {} is before the unix epoch: {error}",
            path.display()
        ))
    })?;
    let nanos = i64::try_from(since_epoch.as_nanos())
        .map_err(|_| import_error("modified time does not fit in i64 nanoseconds"))?;
    Ok((metadata.len(), nanos))
}

fn feature_cityobject_count(feature: &Value, context: &str) -> Result<u64> {
    let cityobjects = feature
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| import_error(format!("{context} is missing CityObjects")))?;
    u64::try_from(cityobjects.len())
        .map_err(|_| import_error("CityObject count does not fit in u64"))
}

fn scan_ndjson_source(path: &Path) -> Result<SourceScan> {
    let bytes = fs::read(path)?;
    let (source_size, source_mtime_ns) = file_status(path)?;
    let line_spans = line_spans(&bytes);
    let Some((_, metadata_bytes)) = line_spans.first() else {
        return Err(import_error(format!(
            "CityJSONSeq source {} is empty",
            path.display()
        )));
    };

    let metadata: Meta = parse_json_slice(metadata_bytes)?;
    let (scale, translate) = parse_ndjson_transform(&metadata)?;
    let mut features = Vec::new();

    for (offset, line_bytes) in line_spans.into_iter().skip(1) {
        if line_bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }

        let feature: Value = parse_json_slice(line_bytes)?;
        let (ids, bounds, spatial) = parse_ndjson_feature_bounds(&feature, scale, translate)?;
        let cityobject_count = feature_cityobject_count(&feature, "CityJSONSeq feature")?;
        let length = u64::try_from(line_bytes.len())
            .map_err(|_| import_error("CityJSONSeq feature line length does not fit in u64"))?;
        features.extend(ids.into_iter().map(|id| ScannedFeature {
            id,
            path: path.to_path_buf(),
            file_size: source_size,
            file_mtime_ns: source_mtime_ns,
            offset,
            length,
            bounds,
            spatial,
            cityobject_count,
            member_ranges: None,
        }));
    }

    Ok(SourceScan {
        path: path.to_path_buf(),
        metadata,
        vertices_offset: None,
        vertices_length: None,
        source_size,
        source_mtime_ns,
        features,
    })
}

fn collect_layout_files(paths: &[PathBuf], suffix: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for root in paths {
        if root.is_file() {
            if root.to_string_lossy().ends_with(suffix) {
                files.push(root.clone());
            }
            continue;
        }

        for entry in WalkBuilder::new(root)
            .hidden(false)
            .follow_links(true)
            .build()
        {
            let entry = entry.map_err(|error| import_error(error.to_string()))?;
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            let path = entry.into_path();
            if path.to_string_lossy().ends_with(suffix) {
                files.push(path);
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn scan_cityjson_source(path: &Path) -> Result<SourceScan> {
    let bytes = fs::read(path)?;
    let (source_size, source_mtime_ns) = file_status(path)?;
    let document: Value = parse_json_slice(&bytes)?;
    let metadata = cityjson_base_metadata(&document)?;
    let (scale, translate) = parse_ndjson_transform(&metadata)?;

    let cityobjects = document
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            import_error(format!(
                "CityJSON source {} is missing CityObjects",
                path.display()
            ))
        })?;
    validate_cityjson_relationship_graph(cityobjects)?;
    let vertices_value = document.get("vertices").ok_or_else(|| {
        import_error(format!(
            "CityJSON source {} is missing vertices",
            path.display()
        ))
    })?;
    let vertices: Vec<[i64; 3]> = parse_json_value(vertices_value.clone())?;
    let (vertices_offset, vertices_length) = top_level_value_range(&bytes, "vertices")?;
    let cityobject_ranges = cityobject_entry_ranges(&bytes)?
        .into_iter()
        .map(|(id, offset, length)| (id, (offset, length)))
        .collect::<HashMap<_, _>>();

    let root_ids = root_cityobject_ids(cityobjects);
    let mut features = Vec::with_capacity(root_ids.len());
    for id in root_ids {
        let (offset, length) = cityobject_ranges.get(id).copied().ok_or_else(|| {
            import_error(format!(
                "CityObject fragment for {id} could not be located in {}",
                path.display()
            ))
        })?;
        let member_ids = collect_cityjson_feature_members(id, cityobjects)?;
        let member_ranges = member_ids
            .iter()
            .map(|member_id| {
                let (member_offset, member_length) =
                    cityobject_ranges.get(member_id).copied().ok_or_else(|| {
                        import_error(format!(
                            "CityObject fragment for {member_id} could not be located in {}",
                            path.display()
                        ))
                    })?;
                Ok(IndexedObjectRange {
                    id: member_id.clone(),
                    offset: member_offset,
                    length: member_length,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut referenced_vertices = BTreeSet::new();
        let mut visited = BTreeSet::new();
        collect_cityjson_object_vertex_indices(
            id,
            cityobjects,
            &mut referenced_vertices,
            &mut visited,
        )?;
        let (bounds, spatial) = if referenced_vertices.is_empty() {
            (empty_feature_bounds(), false)
        } else {
            (
                feature_bounds_from_vertices(&vertices, &referenced_vertices, scale, translate)?,
                true,
            )
        };
        features.push(ScannedFeature {
            id: id.clone(),
            path: path.to_path_buf(),
            file_size: source_size,
            file_mtime_ns: source_mtime_ns,
            offset,
            length,
            bounds,
            spatial,
            cityobject_count: u64::try_from(member_ranges.len())
                .map_err(|_| import_error("CityObject count does not fit in u64"))?,
            member_ranges: Some(member_ranges),
        });
    }

    Ok(SourceScan {
        path: path.to_path_buf(),
        metadata,
        vertices_offset: Some(vertices_offset),
        vertices_length: Some(vertices_length),
        source_size,
        source_mtime_ns,
        features,
    })
}

fn validate_cityjson_relationship_graph(cityobjects: &Map<String, Value>) -> Result<()> {
    let mut graph = BTreeMap::new();
    for (object_id, object) in cityobjects {
        let mut children = Vec::new();
        if let Some(values) = object.get("children").and_then(Value::as_array) {
            for value in values {
                let child = value.as_str().ok_or_else(|| {
                    import_error(format!(
                        "CityObject {object_id} child reference is not a string"
                    ))
                })?;
                if !cityobjects.contains_key(child) {
                    return Err(import_error(format!("missing relationship target {child}")));
                }
                children.push(child);
            }
        }
        graph.insert(object_id.as_str(), children);
    }
    ensure_relationship_graph_acyclic(&graph)
}

fn empty_feature_bounds() -> FeatureBounds {
    FeatureBounds {
        min_x: 0.0,
        max_x: 0.0,
        min_y: 0.0,
        max_y: 0.0,
        min_z: 0.0,
        max_z: 0.0,
    }
}

fn cityjson_base_metadata(document: &Value) -> Result<Meta> {
    let mut metadata = document.clone();
    let root = metadata
        .as_object_mut()
        .ok_or_else(|| import_error("CityJSON document root must be a JSON object"))?;
    root.insert("CityObjects".to_owned(), Value::Object(Map::new()));
    root.insert("vertices".to_owned(), Value::Array(Vec::new()));
    Ok(metadata)
}

fn root_cityobject_ids(cityobjects: &Map<String, Value>) -> Vec<&String> {
    let mut child_ids = BTreeSet::new();
    let mut ids = cityobjects.keys().collect::<Vec<_>>();

    for object in cityobjects.values() {
        if let Some(children) = object.get("children").and_then(Value::as_array) {
            for child in children {
                if let Some(child_id) = child.as_str() {
                    child_ids.insert(child_id.to_owned());
                }
            }
        }
    }

    ids.sort();
    ids.into_iter()
        .filter(|id| {
            cityobjects
                .get(*id)
                .and_then(|object| object.get("parents"))
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
                && !child_ids.contains(id.as_str())
        })
        .collect()
}

fn collect_cityjson_feature_members(
    root_id: &str,
    cityobjects: &Map<String, Value>,
) -> Result<Vec<String>> {
    let mut members = Vec::new();
    let mut visited = BTreeSet::new();
    collect_cityjson_feature_members_recursive(root_id, cityobjects, &mut members, &mut visited)?;
    Ok(members)
}

fn collect_cityjson_feature_members_recursive(
    object_id: &str,
    cityobjects: &Map<String, Value>,
    members: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    if !visited.insert(object_id.to_owned()) {
        return Ok(());
    }

    let object = cityobjects.get(object_id).ok_or_else(|| {
        import_error(format!(
            "CityJSON source is missing referenced CityObject {object_id}"
        ))
    })?;
    members.push(object_id.to_owned());

    if let Some(children) = object.get("children").and_then(Value::as_array) {
        for child in children {
            let Some(child_id) = child.as_str() else {
                return Err(import_error(
                    "CityObject children must be string identifiers",
                ));
            };
            if cityobjects.contains_key(child_id) {
                collect_cityjson_feature_members_recursive(
                    child_id,
                    cityobjects,
                    members,
                    visited,
                )?;
            }
        }
    }

    Ok(())
}

fn collect_cityjson_object_vertex_indices(
    object_id: &str,
    cityobjects: &Map<String, Value>,
    indices: &mut BTreeSet<usize>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    if !visited.insert(object_id.to_owned()) {
        return Ok(());
    }

    let object = cityobjects.get(object_id).ok_or_else(|| {
        import_error(format!(
            "CityJSON source is missing referenced CityObject {object_id}"
        ))
    })?;
    collect_object_vertex_indices(object, indices)?;

    if let Some(children) = object.get("children").and_then(Value::as_array) {
        for child in children {
            let Some(child_id) = child.as_str() else {
                return Err(import_error(
                    "CityObject children must be string identifiers",
                ));
            };
            if cityobjects.contains_key(child_id) {
                collect_cityjson_object_vertex_indices(child_id, cityobjects, indices, visited)?;
            }
        }
    }

    Ok(())
}

fn collect_object_vertex_indices(object: &Value, indices: &mut BTreeSet<usize>) -> Result<()> {
    if let Some(geometries) = object.get("geometry").and_then(Value::as_array) {
        for geometry in geometries {
            if let Some(boundaries) = geometry.get("boundaries") {
                collect_vertex_indices(boundaries, indices)?;
            }
        }
    }
    Ok(())
}

fn top_level_value_range(bytes: &[u8], key: &str) -> Result<(u64, u64)> {
    let key_start = find_json_key(bytes, key)
        .ok_or_else(|| import_error(format!("top-level key {key} could not be located")))?;
    let mut cursor = skip_json_whitespace(bytes, key_start + key.len() + 2);
    if bytes.get(cursor) != Some(&b':') {
        return Err(import_error(format!(
            "top-level key {key} is missing a value separator"
        )));
    }
    cursor = skip_json_whitespace(bytes, cursor + 1);
    let value_end = json_value_end(bytes, cursor)?;
    Ok((
        u64::try_from(cursor).map_err(|_| import_error("value offset does not fit in u64"))?,
        u64::try_from(value_end - cursor)
            .map_err(|_| import_error("value length does not fit in u64"))?,
    ))
}

fn cityobject_entry_ranges(bytes: &[u8]) -> Result<Vec<(String, u64, u64)>> {
    let key_start = find_json_key(bytes, "CityObjects")
        .ok_or_else(|| import_error("top-level key CityObjects could not be located"))?;
    let mut cursor = skip_json_whitespace(bytes, key_start + "\"CityObjects\"".len());
    if bytes.get(cursor) != Some(&b':') {
        return Err(import_error("CityObjects key is missing a value separator"));
    }
    cursor = skip_json_whitespace(bytes, cursor + 1);
    if bytes.get(cursor) != Some(&b'{') {
        return Err(import_error("CityObjects must be a JSON object"));
    }
    cursor += 1;

    let mut entries = Vec::new();
    loop {
        cursor = skip_json_whitespace(bytes, cursor);
        match bytes.get(cursor) {
            Some(b'}') => break,
            Some(b'"') => {
                let entry_start = cursor;
                let (id, after_key) = parse_json_string(bytes, cursor)?;
                cursor = skip_json_whitespace(bytes, after_key);
                if bytes.get(cursor) != Some(&b':') {
                    return Err(import_error(
                        "CityObject entry is missing a value separator",
                    ));
                }
                cursor = skip_json_whitespace(bytes, cursor + 1);
                let value_end = json_value_end(bytes, cursor)?;
                let offset = u64::try_from(entry_start)
                    .map_err(|_| import_error("CityObject entry offset does not fit in u64"))?;
                let length = u64::try_from(value_end - entry_start)
                    .map_err(|_| import_error("CityObject entry length does not fit in u64"))?;
                entries.push((id, offset, length));
                cursor = skip_json_whitespace(bytes, value_end);
                match bytes.get(cursor) {
                    Some(b',') => cursor += 1,
                    Some(b'}') => break,
                    _ => {
                        return Err(import_error(
                            "CityObjects entries must be separated by commas",
                        ));
                    }
                }
            }
            _ => return Err(import_error("unexpected token inside CityObjects object")),
        }
    }

    Ok(entries)
}

fn find_json_key(bytes: &[u8], key: &str) -> Option<usize> {
    let needle = format!("\"{key}\"");
    bytes
        .windows(needle.len())
        .position(|window| window == needle.as_bytes())
}

fn skip_json_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn parse_json_string(bytes: &[u8], start: usize) -> Result<(String, usize)> {
    let mut index = start + 1;
    let mut escaped = false;

    while let Some(byte) = bytes.get(index) {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'"' {
            let end = index + 1;
            return Ok((parse_json_slice(&bytes[start..end])?, end));
        }
        index += 1;
    }

    Err(import_error("unterminated JSON string"))
}

fn json_value_end(bytes: &[u8], start: usize) -> Result<usize> {
    match bytes.get(start) {
        Some(b'{') => nested_json_end(bytes, start, b'{', b'}'),
        Some(b'[') => nested_json_end(bytes, start, b'[', b']'),
        Some(b'"') => parse_json_string(bytes, start).map(|(_, end)| end),
        Some(_) => {
            let mut end = start;
            while let Some(byte) = bytes.get(end) {
                if byte.is_ascii_whitespace() || matches!(*byte, b',' | b'}' | b']') {
                    break;
                }
                end += 1;
            }
            Ok(end)
        }
        None => Err(import_error("unexpected end of JSON input")),
    }
}

fn nested_json_end(bytes: &[u8], start: usize, open: u8, close: u8) -> Result<usize> {
    let mut depth = 0usize;
    let mut index = start;
    let mut in_string = false;
    let mut escaped = false;

    while let Some(byte) = bytes.get(index) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
        } else if *byte == b'"' {
            in_string = true;
        } else if *byte == open {
            depth += 1;
        } else if *byte == close {
            depth -= 1;
            if depth == 0 {
                return Ok(index + 1);
            }
        }
        index += 1;
    }

    Err(import_error("unterminated JSON value"))
}

fn parse_ndjson_transform(metadata: &Value) -> Result<([f64; 3], [f64; 3])> {
    let transform = metadata
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| import_error("CityJSONSeq metadata is missing transform"))?;

    let scale = parse_vector3_f64(transform, "scale")?;
    let translate = parse_vector3_f64(transform, "translate")?;
    Ok((scale, translate))
}

fn feature_cityobject_keys(feature: &Value, label: &str) -> Result<Vec<String>> {
    let cityobjects = feature
        .get("CityObjects")
        .ok_or_else(|| import_error(format!("{label} is missing CityObjects")))?
        .as_object()
        .ok_or_else(|| import_error(format!("{label} CityObjects must be an object")))?;
    if cityobjects.is_empty() {
        return Err(import_error(format!(
            "{label} CityObjects must contain at least one CityObject"
        )));
    }
    Ok(cityobjects.keys().cloned().collect())
}

fn collect_feature_vertex_indices(feature: &Value, vertex_count: usize) -> Result<BTreeSet<usize>> {
    let mut indices = BTreeSet::new();
    let cityobjects = feature
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| import_error("feature package is missing CityObjects"))?;

    for object in cityobjects.values() {
        collect_object_vertex_indices(object, &mut indices)?;
    }

    if indices.is_empty() {
        indices.extend(0..vertex_count);
    }

    Ok(indices)
}

fn parse_vector3_f64(object: &Map<String, Value>, key: &str) -> Result<[f64; 3]> {
    let array = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| import_error(format!("transform is missing {key}")))?;
    if array.len() != 3 {
        return Err(import_error(format!(
            "transform {key} must contain three values"
        )));
    }

    Ok([
        array[0]
            .as_f64()
            .ok_or_else(|| import_error(format!("transform {key}[0] must be numeric")))?,
        array[1]
            .as_f64()
            .ok_or_else(|| import_error(format!("transform {key}[1] must be numeric")))?,
        array[2]
            .as_f64()
            .ok_or_else(|| import_error(format!("transform {key}[2] must be numeric")))?,
    ])
}

fn parse_ndjson_feature_bounds(
    feature: &Value,
    scale: [f64; 3],
    translate: [f64; 3],
) -> Result<(Vec<String>, FeatureBounds, bool)> {
    let ids = feature_cityobject_keys(feature, "CityJSONSeq feature")?;
    let vertices = feature
        .get("vertices")
        .ok_or_else(|| import_error("CityJSONSeq feature is missing vertices"))?;
    let vertices: Vec<[i64; 3]> = parse_json_value(vertices.clone())?;
    let referenced_vertices = collect_feature_vertex_indices(feature, vertices.len())?;
    let (bounds, spatial) = if referenced_vertices.is_empty() {
        (empty_feature_bounds(), false)
    } else {
        (
            feature_bounds_from_vertices(&vertices, &referenced_vertices, scale, translate)?,
            true,
        )
    };
    Ok((ids, bounds, spatial))
}

#[allow(clippy::cast_precision_loss)]
fn feature_bounds_from_vertices(
    vertices: &[[i64; 3]],
    referenced_vertices: &BTreeSet<usize>,
    scale: [f64; 3],
    translate: [f64; 3],
) -> Result<FeatureBounds> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_z = f64::NEG_INFINITY;

    for &index in referenced_vertices {
        let vertex = vertices.get(index).copied().ok_or_else(|| {
            import_error(format!(
                "vertex index {index} is outside the CityJSONSeq feature vertex array"
            ))
        })?;
        let x = translate[0] + scale[0] * vertex[0] as f64;
        let y = translate[1] + scale[1] * vertex[1] as f64;
        let z = translate[2] + scale[2] * vertex[2] as f64;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }

    if !min_x.is_finite()
        || !min_y.is_finite()
        || !min_z.is_finite()
        || !max_x.is_finite()
        || !max_y.is_finite()
        || !max_z.is_finite()
    {
        return Err(import_error(
            "CityJSONSeq feature bbox could not be computed",
        ));
    }

    Ok(FeatureBounds {
        min_x,
        max_x,
        min_y,
        max_y,
        min_z,
        max_z,
    })
}

fn line_spans(bytes: &[u8]) -> Vec<(u64, &[u8])> {
    let mut spans = Vec::new();
    let mut offset = 0u64;

    for chunk in bytes.split_inclusive(|byte| *byte == b'\n') {
        spans.push((offset, trim_line_ending(chunk)));
        offset += u64::try_from(chunk.len()).expect("line chunk length fits in u64");
    }

    if bytes.is_empty() {
        spans.clear();
    }

    spans
}

fn trim_line_ending(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && (bytes[end - 1] == b'\n' || bytes[end - 1] == b'\r') {
        end -= 1;
    }
    &bytes[..end]
}

fn sqlite_result<T>(result: rusqlite::Result<T>) -> Result<T> {
    result.map_err(|value| Error::Import(value.to_string()))
}

fn u64_to_i64(value: u64) -> rusqlite::Result<i64> {
    i64::try_from(value).map_err(|_| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(import_error(format!(
            "value {value} does not fit in SQLite integer storage"
        ))))
    })
}

fn i64_to_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(import_error(format!(
            "value {value} is not representable as u64"
        ))))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn parent_child_lod_fixture() -> CityModel {
        cityjson_lib::json::from_feature_slice(
            br#"{
                "type":"CityJSONFeature",
                "id":"building",
                "CityObjects":{
                    "building":{
                        "type":"Building",
                        "children":["building-part"]
                    },
                    "building-part":{
                        "type":"BuildingPart",
                        "parents":["building"],
                        "geometry":[
                            {"type":"MultiSurface","lod":"1","boundaries":[[[0,1,2]]]},
                            {"type":"MultiSurface","lod":"2","boundaries":[[[0,2,3]]]}
                        ]
                    },
                    "road":{
                        "type":"Road",
                        "geometry":[{"type":"MultiSurface","lod":"1","boundaries":[[[4,5,6]]]}]
                    }
                },
                "vertices":[[0,0,0],[1,0,0],[1,1,0],[0,1,0],[10,0,0],[11,0,0],[10,1,0]]
            }"#,
        )
        .expect("parent-child fixture should parse")
    }

    #[test]
    fn feature_filter_selecting_parent_type_retains_child_geometry() {
        let filter = FeatureFilter {
            cityobject_types: Some(BTreeSet::from(["Building".to_owned()])),
            default_lod: LodSelection::Highest,
            lods_by_type: BTreeMap::new(),
        };

        let filtered = filter
            .apply(&parent_child_lod_fixture())
            .expect("filter should succeed");

        assert_eq!(
            filtered.diagnostics.retained_types,
            BTreeSet::from(["Building".to_owned(), "BuildingPart".to_owned()])
        );
        assert_eq!(
            filtered.diagnostics.retained_lods.get("BuildingPart"),
            Some(&BTreeSet::from(["2".to_owned()]))
        );
        assert!(filtered.model.cityobjects().iter().any(|(_, cityobject)| {
            cityobject.id() == "building-part"
                && cityobject
                    .geometry()
                    .is_some_and(|geometries| !geometries.is_empty())
        }));
        assert!(
            !filtered
                .model
                .cityobjects()
                .iter()
                .any(|(_, cityobject)| cityobject.id() == "road")
        );
    }

    #[test]
    fn feature_filter_summary_reports_missing_explicit_lod() {
        let filter = FeatureFilter {
            cityobject_types: None,
            default_lod: LodSelection::Highest,
            lods_by_type: BTreeMap::from([(
                "BuildingPart".to_owned(),
                LodSelection::Exact("3".to_owned()),
            )]),
        };
        let filtered = filter
            .apply(&parent_child_lod_fixture())
            .expect("filter should succeed");
        let mut summary = FeatureFilterSummary::default();
        summary.add(&filtered.diagnostics);

        let failures = summary.requested_lod_failures(&filter);

        assert_eq!(
            failures,
            vec![MissingLodSelection {
                cityobject_type: "BuildingPart".to_owned(),
                requested_lod: "3".to_owned(),
                available_lods: BTreeSet::from(["1".to_owned(), "2".to_owned()]),
            }]
        );
    }

    #[test]
    fn cityjson_read_one_localizes_vertices_and_preserves_base_root_members() {
        let selected_id = "building-1";
        let selected_object = serde_json::json!({
            "type": "Building",
            "children": ["building-1-part"],
            "geometry": [{
                "type": "MultiSurface",
                "lod": "0",
                "boundaries": [[[2, 7, 5]]]
            }]
        });
        let other_object = serde_json::json!({
            "type": "Building",
            "geometry": [{
                "type": "MultiSurface",
                "lod": "0",
                "boundaries": [[[0, 1, 3]]]
            }]
        });
        let vertices = serde_json::json!([
            [100, 0, 0],
            [101, 0, 0],
            [0, 0, 0],
            [102, 0, 0],
            [103, 0, 0],
            [2, 0, 0],
            [104, 0, 0],
            [1, 0, 0]
        ]);
        let document = serde_json::json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [0.5, 0.5, 0.5],
                "translate": [10.0, 20.0, 30.0]
            },
            "metadata": {
                "title": "unit-test-fixture"
            },
            "CityObjects": {
                selected_id: selected_object.clone(),
                "other-object": other_object
            },
            "vertices": vertices.clone()
        });
        let document_bytes = serde_json::to_vec(&document).expect("fixture JSON");
        let base_document = cityjson_base_metadata(&document).expect("base CityJSON metadata");
        let base_document_bytes: Arc<[u8]> =
            Arc::from(serde_json::to_vec(&base_document).expect("base CityJSON metadata bytes"));
        let object_fragment = object_entry_fragment(selected_id, &selected_object);
        let vertices_fragment = serde_json::to_vec(&vertices).expect("vertices fragment");
        let loc = FeatureLocation {
            feature_id: selected_id.to_owned(),
            source_id: 0,
            source_path: write_temp_cityjson(&document_bytes),
            offset: find_subslice(&document_bytes, &object_fragment)
                .expect("selected object offset") as u64,
            length: object_fragment.len() as u64,
            vertices_offset: Some(
                find_subslice(&document_bytes, &vertices_fragment).expect("vertices offset") as u64,
            ),
            vertices_length: Some(vertices_fragment.len() as u64),
            member_ranges_json: None,
        };

        let backend = CityJsonBackend::new(vec![loc.source_path.clone()]);
        let model = backend
            .read_one(&loc, base_document_bytes)
            .expect("CityJSON read should succeed");
        let output: Value =
            serde_json::from_str(&cityjson_lib::json::to_string(&model).expect("serialize result"))
                .expect("valid output JSON");

        let cityobjects = output["CityObjects"]
            .as_object()
            .expect("result CityObjects must be an object");
        assert_eq!(cityobjects.len(), 1);
        assert!(cityobjects.contains_key(selected_id));
        assert_eq!(output["transform"], document["transform"]);
        assert_eq!(output["metadata"], document["metadata"]);
        assert!(cityobjects[selected_id].get("children").is_none());
        assert_eq!(
            output["vertices"],
            serde_json::json!([[0, 0, 0], [2, 0, 0], [1, 0, 0]])
        );
        assert_eq!(
            cityobjects[selected_id]["geometry"][0]["boundaries"],
            serde_json::json!([[[0, 2, 1]]])
        );
    }

    #[test]
    fn cityjson_scan_and_read_one_group_root_objects_with_children() {
        let document = serde_json::json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [1.0, 1.0, 1.0],
                "translate": [0.0, 0.0, 0.0]
            },
            "CityObjects": {
                "building-1": {
                    "type": "Building",
                    "children": ["building-1-part"],
                    "geometry": [{
                        "type": "MultiSurface",
                        "lod": "1.0",
                        "boundaries": [[[0, 1, 2]]]
                    }]
                },
                "building-1-part": {
                    "type": "BuildingPart",
                    "parents": ["building-1"],
                    "geometry": [{
                        "type": "MultiSurface",
                        "lod": "1.0",
                        "boundaries": [[[3, 4, 5]]]
                    }]
                }
            },
            "vertices": [
                [0, 0, 0],
                [1, 0, 0],
                [0, 1, 0],
                [2, 0, 0],
                [3, 0, 0],
                [2, 1, 0]
            ]
        });
        let bytes = serde_json::to_vec(&document).expect("fixture JSON");
        let path = write_temp_cityjson(&bytes);
        let scan = scan_cityjson_source(&path).expect("scan should succeed");

        assert_eq!(scan.features.len(), 1);
        assert_eq!(scan.features[0].id, "building-1");
        let member_ranges = scan.features[0]
            .member_ranges
            .as_ref()
            .expect("root feature should carry member ranges");
        assert_eq!(member_ranges.len(), 2);
        assert_eq!(member_ranges[0].id, "building-1");
        assert_eq!(member_ranges[1].id, "building-1-part");

        let loc = FeatureLocation {
            feature_id: scan.features[0].id.clone(),
            source_id: 0,
            source_path: path,
            offset: scan.features[0].offset,
            length: scan.features[0].length,
            vertices_offset: scan.vertices_offset,
            vertices_length: scan.vertices_length,
            member_ranges_json: Some(
                serde_json::to_string(member_ranges).expect("member ranges JSON"),
            ),
        };
        let backend = CityJsonBackend::new(vec![loc.source_path.clone()]);
        let metadata_bytes: Arc<[u8]> =
            Arc::from(serde_json::to_vec(&scan.metadata).expect("metadata JSON"));
        let model = backend
            .read_one(&loc, metadata_bytes)
            .expect("CityJSON read should succeed");
        let output: Value =
            serde_json::from_str(&cityjson_lib::json::to_string(&model).expect("serialize result"))
                .expect("valid output JSON");
        let cityobjects = output["CityObjects"]
            .as_object()
            .expect("result CityObjects must be an object");

        assert_eq!(cityobjects.len(), 2);
        assert!(cityobjects.contains_key("building-1"));
        assert!(cityobjects.contains_key("building-1-part"));
        assert_eq!(
            cityobjects["building-1"]["children"],
            serde_json::json!(["building-1-part"])
        );
        assert_eq!(
            cityobjects["building-1-part"]["parents"],
            serde_json::json!(["building-1"])
        );
    }

    #[test]
    fn feature_parts_builder_drops_dangling_parent_links() {
        let parts = build_feature_parts(
            "building-1-part",
            vec![(
                "building-1-part".to_owned(),
                serde_json::json!({
                    "type": "BuildingPart",
                    "parents": ["building-1"],
                    "geometry": [{
                        "type": "MultiSurface",
                        "lod": "0",
                        "boundaries": [[[5, 9, 7]]]
                    }]
                }),
            )],
            &[
                [100, 0, 0],
                [101, 0, 0],
                [102, 0, 0],
                [103, 0, 0],
                [104, 0, 0],
                [0, 0, 0],
                [105, 0, 0],
                [2, 0, 0],
                [106, 0, 0],
                [1, 0, 0],
            ],
        )
        .expect("feature parts should build");
        let object: Value = serde_json::from_str(parts.cityobjects[0].object_json.get())
            .expect("valid object JSON");

        assert_eq!(parts.feature_id, "building-1-part");
        assert!(object.get("parents").is_none());
        assert_eq!(parts.vertices, vec![[0, 0, 0], [2, 0, 0], [1, 0, 0]]);
        assert_eq!(
            object["geometry"][0]["boundaries"],
            serde_json::json!([[[0, 2, 1]]])
        );
    }

    #[test]
    fn cityjsonseq_backend_scan_and_index_lookup_roundtrip() {
        let metadata = serde_json::json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [1.0, 1.0, 1.0],
                "translate": [0.0, 0.0, 0.0]
            }
        });
        let feature = serde_json::json!({
            "type": "CityJSONFeature",
            "id": "ndjson-test-feature",
            "CityObjects": {
                "ndjson-test-feature": {
                    "type": "Building",
                    "geometry": [{
                        "type": "MultiSurface",
                        "lod": "1.0",
                        "boundaries": [[[0, 1, 2]]]
                    }]
                }
            },
            "vertices": [
                [0, 0, 0],
                [1, 0, 0],
                [0, 1, 0]
            ]
        });
        let ndjson_path = write_temp_ndjson(&metadata, &feature);
        let backend = NdjsonBackend {
            paths: vec![ndjson_path.clone()],
        };
        let scans = backend.scan(1).expect("CityJSONSeq scan should succeed");
        assert_eq!(scans.len(), 1);
        assert_eq!(scans[0].features.len(), 1);
        assert_eq!(scans[0].features[0].id, "ndjson-test-feature");

        let index_path = write_temp_index_path();
        let mut index = Index::open(&index_path).expect("SQLite index should open");
        index
            .rebuild(&scans)
            .expect("CityJSONSeq scan should index");

        let by_id = index
            .lookup_id("ndjson-test-feature")
            .expect("id lookup should succeed");
        assert!(
            by_id.is_some(),
            "indexed feature should be addressable by id"
        );

        let hits = index
            .lookup_bbox_iter(BBox {
                min_x: -1.0,
                max_x: 1.0,
                min_y: -1.0,
                max_y: 1.0,
            })
            .collect::<Result<Vec<_>>>()
            .expect("bbox lookup should collect");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source_path, ndjson_path);
    }

    #[test]
    fn opening_old_unique_schema_removes_feature_id_uniqueness() {
        let index_path = write_temp_index_path_with_prefix("old-unique-schema");
        {
            let conn = rusqlite::Connection::open(&index_path).expect("old index should open");
            conn.execute_batch(
                r"
                CREATE TABLE sources (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT NOT NULL UNIQUE,
                    metadata TEXT NOT NULL,
                    vertices_offset INTEGER,
                    vertices_length INTEGER,
                    source_size INTEGER,
                    source_mtime_ns INTEGER
                );
                CREATE TABLE features (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    feature_id TEXT NOT NULL UNIQUE,
                    source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                    path TEXT NOT NULL,
                    file_size INTEGER,
                    file_mtime_ns INTEGER,
                    offset INTEGER NOT NULL,
                    length INTEGER NOT NULL,
                    min_z REAL,
                    max_z REAL,
                    cityobject_count INTEGER,
                    member_ranges TEXT
                );
                CREATE VIRTUAL TABLE feature_bbox
                USING rtree(feature_rowid, min_x, max_x, min_y, max_y);
                CREATE TABLE bbox_map (
                    feature_rowid INTEGER PRIMARY KEY,
                    feature_id TEXT NOT NULL UNIQUE REFERENCES features(feature_id) ON DELETE CASCADE
                );
                INSERT INTO sources (path, metadata, source_size, source_mtime_ns)
                VALUES ('metadata.json', '{}', 0, 0);
                INSERT INTO features (
                    feature_id,
                    source_id,
                    path,
                    file_size,
                    file_mtime_ns,
                    offset,
                    length,
                    min_z,
                    max_z,
                    cityobject_count,
                    member_ranges
                )
                VALUES ('duplicate', 1, 'feature-a.city.jsonl', 0, 0, 0, 1, 0, 0, 1, NULL);
                INSERT INTO feature_bbox (feature_rowid, min_x, max_x, min_y, max_y)
                VALUES (1, 0, 1, 0, 1);
                INSERT INTO bbox_map (feature_rowid, feature_id) VALUES (1, 'duplicate');
                ",
            )
            .expect("old schema should initialize");
        }

        let index = Index::open(&index_path).expect("index migration should succeed");

        assert!(
            !table_sql_contains(&index.conn, "features", "feature_id TEXT NOT NULL UNIQUE",)
                .expect("features schema should load")
        );
        assert!(
            !table_sql_contains(&index.conn, "bbox_map", "feature_id TEXT NOT NULL UNIQUE",)
                .expect("bbox_map schema should load")
        );

        index
            .conn
            .execute(
                r"
                INSERT INTO features (
                    feature_id,
                    source_id,
                    path,
                    file_size,
                    file_mtime_ns,
                    offset,
                    length,
                    min_z,
                    max_z,
                    cityobject_count,
                    member_ranges
                )
                VALUES ('duplicate', 1, 'feature-b.city.jsonl', 0, 0, 0, 1, 0, 0, 1, NULL)
                ",
                [],
            )
            .expect("duplicate feature_id should insert after migration");
        let row_id = index.conn.last_insert_rowid();
        index
            .conn
            .execute(
                "INSERT INTO bbox_map (feature_rowid, feature_id) VALUES (?1, 'duplicate')",
                params![row_id],
            )
            .expect("duplicate bbox_map feature_id should insert after migration");
    }

    #[test]
    fn iter_all_scans_each_supported_layout_in_deterministic_order() {
        let expected_ids = vec!["alpha", "beta", "gamma"];
        let feature_files_root = write_temp_feature_files_root(&expected_ids);
        let feature_files_index_path = write_temp_index_path_with_prefix("feature-files");
        let mut feature_files_index = CityIndex::open(
            StorageLayout::FeatureFiles {
                root: feature_files_root,
                metadata_glob: "**/metadata.json".to_owned(),
                feature_glob: "**/*.city.jsonl".to_owned(),
            },
            &feature_files_index_path,
        )
        .expect("feature-files index should open");
        feature_files_index
            .reindex()
            .expect("feature-files dataset should index");
        assert_full_scan_order(&feature_files_index, &expected_ids);

        let cityjson_root = write_temp_cityjson_root(&expected_ids);
        let cityjson_index_path = write_temp_index_path_with_prefix("cityjson");
        let mut cityjson_index = CityIndex::open(
            StorageLayout::CityJson {
                paths: vec![cityjson_root],
            },
            &cityjson_index_path,
        )
        .expect("cityjson index should open");
        cityjson_index
            .reindex()
            .expect("cityjson dataset should index");
        assert_full_scan_order(&cityjson_index, &expected_ids);

        let ndjson_root = write_temp_ndjson_root(&expected_ids);
        let ndjson_index_path = write_temp_index_path_with_prefix("ndjson");
        let mut ndjson_index = CityIndex::open(
            StorageLayout::Ndjson {
                paths: vec![ndjson_root],
            },
            &ndjson_index_path,
        )
        .expect("CityJSONSeq index should open");
        ndjson_index
            .reindex()
            .expect("CityJSONSeq dataset should index");
        assert_full_scan_order(&ndjson_index, &expected_ids);
        assert_full_scan_pages(&ndjson_index, &expected_ids);
    }

    #[test]
    fn iter_all_paginates_across_multiple_pages() {
        let ids = (0..600)
            .map(|idx| format!("feature-{idx:03}"))
            .collect::<Vec<_>>();
        let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        let root = write_temp_ndjson_root(&id_refs);
        let index_path = write_temp_index_path_with_prefix("iter-all-pages");
        let layout = StorageLayout::Ndjson {
            paths: vec![root.clone()],
        };
        let mut index = CityIndex::open(layout, &index_path).expect("index should open");
        index.reindex().expect("dataset should index");

        let scanned_ids = index
            .iter_all_with_ids()
            .expect("iter_all_with_ids should build")
            .map(|result| result.map(|(id, _)| id))
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_with_ids should collect");

        assert_eq!(scanned_ids.len(), 600);
        assert_eq!(scanned_ids.first().expect("first id"), "feature-000");
        assert_eq!(scanned_ids.last().expect("last id"), "feature-599");

        let ref_pages = index
            .iter_all_feature_ref_pages(128)
            .expect("iter_all_feature_ref_pages should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_feature_ref_pages should collect");
        assert_eq!(
            ref_pages.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![128, 128, 128, 128, 88]
        );
        assert_eq!(
            ref_pages
                .iter()
                .flat_map(|page| page.iter().map(|feature| feature.feature_id.as_str()))
                .collect::<Vec<_>>(),
            ids.iter().map(String::as_str).collect::<Vec<_>>()
        );
        assert_eq!(
            ref_pages
                .iter()
                .flat_map(|page| page.iter().map(|feature| feature.row_id))
                .collect::<Vec<_>>(),
            (1..=600).collect::<Vec<_>>()
        );

        let first_batch = index
            .read_features(ref_pages.first().expect("first page should exist"))
            .expect("feature batch should reconstruct");
        assert_eq!(first_batch.len(), 128);

        assert_indexed_batch_preserves_order(&index, ref_pages.first().expect("first page"), &ids);
        assert_decoded_scan_pages(&index, &ids);
        assert_rowid_feature_reads(&index);

        for page in &ref_pages {
            for feature in page {
                let model = index
                    .read_feature(feature)
                    .expect("feature should reconstruct");
                assert!(model_contains_id(&model, &feature.feature_id));
                assert_eq!(
                    feature_bounds_for_model(&model).expect("bounds should be computable"),
                    feature.bounds
                );
            }
        }

        let bbox_pages = index
            .iter_all_bbox_pages(128)
            .expect("iter_all_bbox_pages should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_bbox_pages should collect");
        assert_eq!(
            bbox_pages
                .iter()
                .flat_map(|page| page.iter().map(|feature| feature.feature_id.as_str()))
                .collect::<Vec<_>>(),
            ids.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }

    #[test]
    fn iter_all_feature_ref_pages_handles_page_boundaries_without_gaps() {
        let ids = (0..256)
            .map(|idx| format!("boundary-{idx:03}"))
            .collect::<Vec<_>>();
        let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        let root = write_temp_ndjson_root(&id_refs);
        let index_path = write_temp_index_path_with_prefix("iter-boundary-pages");
        let mut index = CityIndex::open(StorageLayout::Ndjson { paths: vec![root] }, &index_path)
            .expect("index should open");
        index.reindex().expect("dataset should index");

        let pages = index
            .iter_all_feature_ref_pages(128)
            .expect("iter_all_feature_ref_pages should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_feature_ref_pages should collect");

        assert_eq!(
            pages.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![128, 128]
        );
        assert_eq!(
            pages
                .iter()
                .flat_map(|page| page.iter().map(|feature| feature.feature_id.as_str()))
                .collect::<Vec<_>>(),
            ids.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }

    #[test]
    fn feature_bounds_summary_matches_iterative_bounds() {
        let ids = ["alpha", "beta", "gamma"];
        let root = write_temp_ndjson_root(&ids);
        let index_path = write_temp_index_path_with_prefix("bounds-summary");
        let mut index = CityIndex::open(StorageLayout::Ndjson { paths: vec![root] }, &index_path)
            .expect("index should open");
        index.reindex().expect("dataset should index");

        let summary = index
            .feature_bounds_summary()
            .expect("bounds summary should load")
            .expect("non-empty index should have a summary");
        let mut pages = index
            .iter_all_bbox_pages(2)
            .expect("bbox pages should build")
            .collect::<Result<Vec<_>>>()
            .expect("bbox pages should collect")
            .into_iter()
            .flatten();
        let first = pages.next().expect("first indexed feature");
        let mut expected = first.bounds;
        let mut count = 1usize;
        for feature in pages {
            expected.min_x = expected.min_x.min(feature.bounds.min_x);
            expected.max_x = expected.max_x.max(feature.bounds.max_x);
            expected.min_y = expected.min_y.min(feature.bounds.min_y);
            expected.max_y = expected.max_y.max(feature.bounds.max_y);
            expected.min_z = expected.min_z.min(feature.bounds.min_z);
            expected.max_z = expected.max_z.max(feature.bounds.max_z);
            count += 1;
        }

        assert_eq!(summary.feature_count, count);
        assert_eq!(summary.bounds, expected);
    }

    #[test]
    fn feature_bounds_summary_returns_none_for_empty_index() {
        let index_path = write_temp_index_path_with_prefix("empty-bounds-summary");
        let index = CityIndex::open(StorageLayout::Ndjson { paths: Vec::new() }, &index_path)
            .expect("index should open");

        assert_eq!(
            index.feature_bounds_summary().expect("summary should load"),
            None
        );
    }

    #[test]
    fn iter_all_feature_ref_pages_rejects_zero_page_size() {
        let root = write_temp_ndjson_root(&["alpha"]);
        let index_path = write_temp_index_path_with_prefix("page-size-zero");
        let mut index = CityIndex::open(StorageLayout::Ndjson { paths: vec![root] }, &index_path)
            .expect("index should open");
        index.reindex().expect("dataset should index");

        match index.iter_all_feature_ref_pages(0) {
            Ok(_) => panic!("zero page size should be rejected"),
            Err(error) => assert!(error.to_string().contains("page_size")),
        }
    }

    #[test]
    fn read_exact_range_reads_only_the_requested_span() {
        let path = write_temp_bytes(b"abcdefghij");

        let bytes = read_exact_range(&path, 3, 4).expect("range read should succeed");

        assert_eq!(bytes, b"defg");
    }

    #[test]
    fn read_exact_range_rejects_short_reads() {
        let path = write_temp_bytes(b"abc");

        let error = read_exact_range(&path, 2, 4).expect_err("range read should fail");

        assert!(error.to_string().contains("short read"));
    }

    #[test]
    fn read_exact_range_rejects_oversized_lengths() {
        let path = write_temp_bytes(b"abc");

        let error = read_exact_range(&path, 0, u64::MAX).expect_err("range read should fail");

        assert!(
            error
                .to_string()
                .contains("exceeds the supported buffer size")
        );
    }

    #[test]
    fn feature_files_metadata_resolution_prefers_nearest_ancestor() {
        let root = PathBuf::from("/data/root");
        let mut metadata_by_dir = BTreeMap::new();
        metadata_by_dir.insert(root.clone(), root.join("metadata.json"));
        metadata_by_dir.insert(
            root.join("features/8"),
            root.join("features/8/metadata.json"),
        );

        let feature_path = root.join("features/8/296/592/sample.city.jsonl");
        let resolved = resolve_feature_metadata_path(&root, &feature_path, &metadata_by_dir)
            .expect("metadata must resolve");

        assert_eq!(resolved, root.join("features/8/metadata.json"));
    }

    fn object_entry_fragment(object_id: &str, object: &Value) -> Vec<u8> {
        let mut map = Map::new();
        map.insert(object_id.to_owned(), object.clone());
        let serialized = serde_json::to_vec(&Value::Object(map)).expect("object entry");
        serialized[1..serialized.len() - 1].to_vec()
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn write_temp_cityjson(bytes: &[u8]) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("cityjson-index-cityjson-read-one-{unique}.json"));
        fs::write(&path, bytes).expect("write temp cityjson");
        path
    }

    fn write_temp_ndjson(metadata: &Value, feature: &Value) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cityjson-index-ndjson-{unique}.jsonl"));
        let contents = format!(
            "{}\n{}\n",
            serde_json::to_string(metadata).expect("metadata JSON"),
            serde_json::to_string(feature).expect("feature JSON")
        );
        fs::write(&path, contents).expect("write temp CityJSONSeq");
        path
    }

    fn write_temp_index_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cityjson-index-ndjson-{unique}.sqlite"));
        if path.exists() {
            fs::remove_file(&path).expect("remove temp sqlite");
        }
        path
    }

    fn write_temp_index_path_with_prefix(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cityjson-index-{prefix}-{unique}.sqlite"));
        if path.exists() {
            fs::remove_file(&path).expect("remove temp sqlite");
        }
        path
    }

    fn write_temp_feature_files_root(ids: &[&str]) -> PathBuf {
        let root = write_temp_dir("cityjson-index-feature-files");
        fs::write(
            root.join("metadata.json"),
            serde_json::to_vec(&base_document()).expect("metadata JSON"),
        )
        .expect("write metadata");
        for (idx, id) in ids.iter().enumerate() {
            let feature_path = root.join(format!("features/{idx:03}.city.jsonl"));
            let idx = i64::try_from(idx).expect("test index fits in i64");
            if let Some(parent) = feature_path.parent() {
                fs::create_dir_all(parent).expect("create feature directory");
            }
            fs::write(
                &feature_path,
                serde_json::to_vec(&feature_feature_document(id, idx)).expect("feature JSON"),
            )
            .expect("write feature file");
        }
        root
    }

    fn write_temp_cityjson_root(ids: &[&str]) -> PathBuf {
        let root = write_temp_dir("cityjson-index-cityjson");
        let mut cityobjects = Map::new();
        for id in ids {
            cityobjects.insert((*id).to_owned(), feature_object(0));
        }
        let document = serde_json::json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [1.0, 1.0, 1.0],
                "translate": [0.0, 0.0, 0.0]
            },
            "metadata": {
                "referenceSystem": "https://www.opengis.net/def/crs/EPSG/0/7415"
            },
            "CityObjects": cityobjects,
            "vertices": [
                [0, 0, 0],
                [1, 0, 0],
                [0, 1, 0]
            ]
        });
        fs::write(
            root.join("dataset.city.json"),
            serde_json::to_vec(&document).expect("cityjson JSON"),
        )
        .expect("write cityjson");
        root
    }

    fn write_temp_ndjson_root(ids: &[&str]) -> PathBuf {
        let root = write_temp_dir("cityjson-index-ndjson-root");
        let mut contents = serde_json::to_string(&base_document()).expect("metadata JSON");
        contents.push('\n');
        for (idx, id) in ids.iter().enumerate() {
            let idx = i64::try_from(idx).expect("test index fits in i64");
            contents.push_str(
                &serde_json::to_string(&feature_feature_document(id, idx)).expect("feature JSON"),
            );
            contents.push('\n');
        }
        fs::write(root.join("dataset.city.jsonl"), contents).expect("write CityJSONSeq");
        root
    }

    fn write_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn base_document() -> Value {
        serde_json::json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [1.0, 1.0, 1.0],
                "translate": [0.0, 0.0, 0.0]
            },
            "metadata": {
                "referenceSystem": "https://www.opengis.net/def/crs/EPSG/0/7415"
            },
            "CityObjects": {},
            "vertices": []
        })
    }

    fn feature_feature_document(id: &str, offset: i64) -> Value {
        let object = feature_object(offset);
        serde_json::json!({
            "type": "CityJSONFeature",
            "id": id,
            "CityObjects": {
                id: object
            },
            "vertices": [
                [offset, 0, 0],
                [offset + 1, 0, 0],
                [offset, 1, 0]
            ]
        })
    }

    fn feature_object(_offset: i64) -> Value {
        serde_json::json!({
            "type": "Building",
            "geometry": [{
                "type": "MultiSurface",
                "lod": "1.0",
                "boundaries": [[[0, 1, 2]]]
            }]
        })
    }

    fn assert_full_scan_order(index: &CityIndex, expected_ids: &[&str]) {
        let ids = index
            .iter_all_with_ids()
            .expect("iter_all_with_ids should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_with_ids should collect");
        assert_eq!(
            ids.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            expected_ids
        );

        let models = index
            .iter_all()
            .expect("iter_all should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all should collect");
        assert_eq!(models.len(), expected_ids.len());

        let models_with_metadata = index
            .iter_all_with_metadata()
            .expect("iter_all_with_metadata should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_with_metadata should collect");
        assert_eq!(models_with_metadata.len(), expected_ids.len());
    }

    fn assert_full_scan_pages(index: &CityIndex, expected_ids: &[&str]) {
        let pages = index
            .iter_all_feature_ref_pages(2)
            .expect("iter_all_feature_ref_pages should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_feature_ref_pages should collect");
        assert_eq!(
            pages
                .iter()
                .flat_map(|page| page.iter().map(|feature| feature.feature_id.as_str()))
                .collect::<Vec<_>>(),
            expected_ids
        );

        let bbox_pages = index
            .iter_all_bbox_pages(2)
            .expect("iter_all_bbox_pages should build")
            .collect::<Result<Vec<_>>>()
            .expect("iter_all_bbox_pages should collect");
        assert_eq!(
            bbox_pages
                .iter()
                .flat_map(|page| page.iter().map(|feature| feature.feature_id.as_str()))
                .collect::<Vec<_>>(),
            expected_ids
        );

        for page in pages {
            for feature in page {
                let model = index
                    .read_feature(&feature)
                    .expect("feature should reconstruct");
                assert!(model_contains_id(&model, &feature.feature_id));
                assert_eq!(
                    feature_bounds_for_model(&model).expect("bounds should be computable"),
                    feature.bounds
                );
            }
        }
    }

    fn assert_indexed_batch_preserves_order(
        index: &CityIndex,
        features: &[IndexedFeatureRef],
        ids: &[String],
    ) {
        let indexed_features = index
            .read_indexed_features(features)
            .expect("indexed feature batch should reconstruct");
        assert_eq!(
            indexed_features
                .iter()
                .map(|feature| feature.reference.feature_id.as_str())
                .collect::<Vec<_>>(),
            ids.iter()
                .take(features.len())
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
    }

    fn assert_decoded_scan_pages(index: &CityIndex, ids: &[String]) {
        let scan_pages = index
            .scan_feature_pages(128)
            .expect("scan_feature_pages should build")
            .collect::<Result<Vec<_>>>()
            .expect("scan_feature_pages should collect");
        assert_eq!(
            scan_pages.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![128, 128, 128, 128, 88]
        );
        assert_eq!(
            scan_pages
                .iter()
                .flat_map(|page| {
                    page.iter()
                        .map(|feature| feature.reference.feature_id.as_str())
                })
                .collect::<Vec<_>>(),
            ids.iter().map(String::as_str).collect::<Vec<_>>()
        );

        let scanned_ids = index
            .scan_features()
            .expect("scan_features should build")
            .map(|result| result.map(|feature| feature.reference.feature_id))
            .collect::<Result<Vec<_>>>()
            .expect("scan_features should collect");
        assert_eq!(scanned_ids, ids);
    }

    fn assert_rowid_feature_reads(index: &CityIndex) {
        let first_ref = index
            .lookup_feature_ref_by_rowid(1)
            .expect("rowid lookup should load")
            .expect("first rowid should exist");
        assert_eq!(first_ref.feature_id, "feature-000");
        assert_eq!(
            index
                .lookup_feature_ref_by_rowid(9999)
                .expect("missing rowid lookup should load"),
            None
        );

        let rowid_features = index
            .read_features_by_rowids(&[2, 9999, 1, 2])
            .expect("rowid batch should reconstruct");
        assert_eq!(
            rowid_features
                .iter()
                .map(|feature| feature
                    .as_ref()
                    .map(|feature| feature.reference.feature_id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                Some("feature-001"),
                None,
                Some("feature-000"),
                Some("feature-001")
            ]
        );

        let range_features = index
            .read_feature_range_after_rowid(Some(127), 3)
            .expect("rowid range should reconstruct");
        assert_eq!(
            range_features
                .iter()
                .map(|feature| feature.reference.feature_id.as_str())
                .collect::<Vec<_>>(),
            vec!["feature-127", "feature-128", "feature-129"]
        );
    }

    fn model_contains_id(model: &CityModel, id: &str) -> bool {
        let value: Value =
            serde_json::from_str(&cityjson_lib::json::to_string(model).expect("serialize model"))
                .expect("model JSON");
        value["CityObjects"]
            .as_object()
            .is_some_and(|cityobjects| cityobjects.contains_key(id))
    }

    fn feature_bounds_for_model(model: &CityModel) -> Result<FeatureBounds> {
        let value: Value =
            serde_json::from_str(&cityjson_lib::json::to_string(model).expect("serialize model"))
                .expect("model JSON");
        let vertices = value
            .get("vertices")
            .and_then(Value::as_array)
            .ok_or_else(|| import_error("model JSON is missing vertices"))?;
        let transform = value
            .get("transform")
            .and_then(Value::as_object)
            .ok_or_else(|| import_error("model JSON is missing transform"))?;
        let scale = parse_transform_component(transform, "scale")?;
        let translate = parse_transform_component(transform, "translate")?;

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut min_z = f64::INFINITY;
        let mut max_z = f64::NEG_INFINITY;

        for vertex in vertices {
            let coords = vertex
                .as_array()
                .ok_or_else(|| import_error("vertex must be an array"))?;
            if coords.len() != 3 {
                return Err(import_error("vertex must have three coordinates"));
            }
            let x = translate[0] + scale[0] * value_as_f64(&coords[0])?;
            let y = translate[1] + scale[1] * value_as_f64(&coords[1])?;
            let z = translate[2] + scale[2] * value_as_f64(&coords[2])?;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }

        if !min_x.is_finite()
            || !max_x.is_finite()
            || !min_y.is_finite()
            || !max_y.is_finite()
            || !min_z.is_finite()
            || !max_z.is_finite()
        {
            return Err(import_error(
                "could not compute a finite bbox from the model",
            ));
        }

        Ok(FeatureBounds {
            min_x,
            max_x,
            min_y,
            max_y,
            min_z,
            max_z,
        })
    }

    fn parse_transform_component(
        transform: &serde_json::Map<String, Value>,
        key: &str,
    ) -> Result<[f64; 3]> {
        let values = transform
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| import_error(format!("transform is missing {key}")))?;
        if values.len() != 3 {
            return Err(import_error(format!(
                "transform {key} must contain three values"
            )));
        }
        Ok([
            value_as_f64(&values[0])?,
            value_as_f64(&values[1])?,
            value_as_f64(&values[2])?,
        ])
    }

    fn value_as_f64(value: &Value) -> Result<f64> {
        value
            .as_f64()
            .ok_or_else(|| import_error("expected a numeric value"))
    }

    fn write_temp_bytes(bytes: &[u8]) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cityjson-index-range-read-{unique}.bin"));
        fs::write(&path, bytes).expect("write temp bytes");
        path
    }
}
