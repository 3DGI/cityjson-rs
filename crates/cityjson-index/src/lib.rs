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
const SCHEMA_VERSION: i64 = 2;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingLodSelection {
    pub cityobject_type: String,
    pub requested_lod: String,
    pub available_lods: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct PackageFilterDiagnostics {
    pub available_types: BTreeSet<String>,
    pub retained_types: BTreeSet<String>,
    pub ignored_types: BTreeSet<String>,
    pub available_lods: BTreeMap<String, BTreeSet<String>>,
    pub retained_lods: BTreeMap<String, BTreeSet<String>>,
    pub missing_lods: Vec<MissingLodSelection>,
    pub retained_geometry_count: usize,
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
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.cityobject_types.is_some()
            || self.default_lod != LodSelection::All
            || self
                .lods_by_type
                .values()
                .any(|selection| *selection != LodSelection::All)
    }

    /// Applies this package filter to one `CityJSONFeature` package.
    ///
    /// Object-type selection keeps descendants of selected `CityObjects`. This
    /// preserves common packages where a selected parent object, such as
    /// `Building`, carries its renderable geometry on child objects.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `CityObject` or `LoD` selection fails.
    pub fn apply(&self, model: &CityModel) -> Result<PackageFilterResult> {
        let retained_handles = retained_cityobject_handles(model, self)?;
        let diagnostics = filter_diagnostics(model, &retained_handles, self);
        let filtered_model = if self.is_active() {
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
                (None, None) => model_selection_all(model)?,
            };
            extract_or_empty_package(model, &selection)?
        } else {
            model.clone()
        };

        let mut report = PackageFilterReport::from_diagnostics(&diagnostics);
        if let LodSelection::Exact(requested_lod) = &self.default_lod
            && diagnostics.retained_geometry_count == 0
        {
            for cityobject_type in &diagnostics.available_types {
                report
                    .missing_lods
                    .entry(cityobject_type.clone())
                    .or_insert_with(|| MissingLodSelection {
                        cityobject_type: cityobject_type.clone(),
                        requested_lod: requested_lod.clone(),
                        available_lods: diagnostics
                            .available_lods
                            .get(cityobject_type)
                            .cloned()
                            .unwrap_or_default(),
                    });
            }
        }
        let model = if diagnostics.retained_geometry_count == 0 {
            None
        } else {
            Some(filtered_model)
        };
        Ok(PackageFilterResult { model, report })
    }
}

impl PackageFilterReport {
    fn from_diagnostics(diagnostics: &PackageFilterDiagnostics) -> Self {
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
    filter: &PackageFilter,
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
    filter: &PackageFilter,
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
    filter: &PackageFilter,
) -> PackageFilterDiagnostics {
    let highest_lods = highest_lods_by_cityobject(model, retained_handles);
    let mut diagnostics = PackageFilterDiagnostics::default();

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

fn model_selection_all(model: &CityModel) -> Result<cityjson_lib::ops::ModelSelection> {
    cityjson_lib::ops::select_cityobjects(model, |_| true)
}

fn extract_or_empty_package(
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
    /// Passing `None` starts from the first package. Results use keyset
    /// pagination and are stable for large scans.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` lookup fails or `limit` is zero.
    pub fn package_ref_page_after_record_id(
        &self,
        after_record_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<IndexedPackageRef>> {
        if limit == 0 {
            return Err(import_error("limit must be greater than zero"));
        }
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
            WHERE (?1 IS NULL OR p.id > ?1)
            ORDER BY p.id
            LIMIT ?2
            ",
        ))?;
        let rows =
            sqlite_result(stmt.query_map(params![after_record_id, limit], package_ref_from_row))?;
        sqlite_result(rows.collect())
    }

    /// Returns a page of package refs whose package bbox intersects `bbox`.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` query fails or `limit` is zero.
    pub fn query_package_refs_page(
        &self,
        bbox: &BBox,
        after_package_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<IndexedPackageRef>> {
        if limit == 0 {
            return Err(import_error("limit must be greater than zero"));
        }
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
              AND (?5 IS NULL OR p.id > ?5)
            ORDER BY p.id
            LIMIT ?6
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(
            params![
                bbox.min_x,
                bbox.max_x,
                bbox.min_y,
                bbox.max_y,
                after_package_id,
                limit
            ],
            package_ref_from_row,
        ))?;
        sqlite_result(rows.collect())
    }

    /// Returns package refs whose package bbox intersects `bbox`.
    ///
    /// This eager convenience method is implemented on top of paged keyset
    /// queries. Prefer [`CityIndex::query_package_refs_page`] for large result
    /// sets.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` query fails.
    pub fn query_package_refs(&self, bbox: &BBox) -> Result<Vec<IndexedPackageRef>> {
        let mut refs = Vec::new();
        let mut after_package_id = None;
        loop {
            let page =
                self.query_package_refs_page(bbox, after_package_id, DEFAULT_SCAN_PAGE_SIZE)?;
            if page.is_empty() {
                break;
            }
            after_package_id = page.last().map(|package| package.record_id);
            refs.extend(page);
        }
        Ok(refs)
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

    /// Returns decoded package models whose package bbox intersects `bbox`.
    ///
    /// # Errors
    ///
    /// Returns an error if package lookup or reconstruction fails.
    pub fn query_package_models(&self, bbox: &BBox) -> Result<Vec<CityModel>> {
        self.query_packages(bbox)
            .map(|packages| packages.into_iter().map(|package| package.model).collect())
    }

    /// Returns decoded packages whose package bbox intersects `bbox`.
    ///
    /// # Errors
    ///
    /// Returns an error if package lookup or reconstruction fails.
    pub fn query_packages(&self, bbox: &BBox) -> Result<Vec<IndexedPackage>> {
        let refs = self.query_package_refs(bbox)?;
        self.read_packages(&refs)
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
                self.read_cityjson_package(&location, metadata.bytes.as_ref())?
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
        metadata_bytes: &[u8],
    ) -> Result<CityModel> {
        let members = self.index.package_members(location.reference.record_id)?;
        let backend = self.backend.as_cityjson().ok_or_else(|| {
            import_error("regular CityJSON package read requires CityJSON backend")
        })?;
        backend.read_package_members(
            &location.reference.model_id,
            &location.path,
            &members,
            metadata_bytes,
        )
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

    /// Returns the total number of indexed packages.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be read from the index.
    pub fn package_count(&self) -> Result<usize> {
        self.index.package_count()
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

    /// Returns cached metadata entries.
    ///
    /// # Errors
    ///
    /// Returns an error if metadata lookup fails.
    pub fn metadata(&self) -> Result<Vec<Arc<Meta>>> {
        self.index.metadata()
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

struct PackageLocation {
    reference: IndexedPackageRef,
    source_id: i64,
    path: PathBuf,
    offset: Option<u64>,
    length: Option<u64>,
}

struct PackageMemberLocation {
    external_id: String,
    source_id: i64,
    source_path: PathBuf,
    offset: u64,
    length: u64,
    vertices_offset: Option<u64>,
    vertices_length: Option<u64>,
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
        let existing_schema_version = if has_schema_state {
            Some(Self::schema_version(&conn)?)
        } else {
            None
        };
        if let Some(schema_version) = existing_schema_version
            && schema_version > SCHEMA_VERSION
        {
            return Err(import_error(format!(
                "unsupported cityjson-index schema version {schema_version}; rebuild with a newer cjindex"
            )));
        }

        Self::create_schema(&conn)?;
        let needs_reindex = i64::from(
            existing_schema_version.is_some_and(|version| version < SCHEMA_VERSION)
                || (!has_schema_state && has_legacy_features),
        );
        Self::ensure_schema_state(&conn, needs_reindex)?;

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
            CREATE INDEX IF NOT EXISTS package_cityobjects_package_order_idx
                ON package_cityobjects(package_id, ordinal, cityobject_id);
            CREATE INDEX IF NOT EXISTS package_cityobjects_cityobject_id_idx
                ON package_cityobjects(cityobject_id, package_id);
            CREATE INDEX IF NOT EXISTS cityobject_relationships_child_idx
                ON cityobject_relationships(child_cityobject_id);
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
                    member_ranges_json: feature
                        .member_ranges
                        .as_ref()
                        .map(json_string)
                        .transpose()?,
                });
            }
        }
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

    fn package_members(&self, package_id: i64) -> Result<Vec<PackageMemberLocation>> {
        let mut stmt = sqlite_result(self.conn.prepare(
            r"
            SELECT
                c.external_id,
                c.source_id,
                c.path,
                c.offset,
                c.length,
                s.vertices_offset,
                s.vertices_length
            FROM package_cityobjects AS pc
            JOIN cityobjects AS c ON c.id = pc.cityobject_id
            JOIN sources AS s ON s.id = c.source_id
            WHERE pc.package_id = ?1
            ORDER BY pc.ordinal
            ",
        ))?;
        let rows = sqlite_result(stmt.query_map(params![package_id], |row| {
            Ok(PackageMemberLocation {
                external_id: row.get(0)?,
                source_id: row.get(1)?,
                source_path: PathBuf::from(row.get::<_, String>(2)?),
                offset: i64_to_u64(row.get::<_, i64>(3)?)?,
                length: i64_to_u64(row.get::<_, i64>(4)?)?,
                vertices_offset: row.get::<_, Option<i64>>(5)?.map(i64_to_u64).transpose()?,
                vertices_length: row.get::<_, Option<i64>>(6)?.map(i64_to_u64).transpose()?,
            })
        }))?;
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
        self.package_count()
    }

    fn cityobject_count(&self) -> Result<usize> {
        self.normalized_cityobject_count()
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
            FROM packages
            WHERE package_type = 'feature-files'
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

    fn clear_tables(tx: &rusqlite::Transaction<'_>) -> Result<()> {
        sqlite_result(tx.execute_batch(
            r"
            DELETE FROM cityobject_relationships;
            DELETE FROM package_cityobjects;
            DELETE FROM cityobject_bbox;
            DELETE FROM package_bbox;
            DELETE FROM cityobjects;
            DELETE FROM packages;
            DELETE FROM sources;
            DROP TABLE IF EXISTS bbox_map;
            DROP TABLE IF EXISTS feature_bbox;
            DROP TABLE IF EXISTS features;
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
    cityobject_entry_ranges(&feature_bytes)?
        .into_iter()
        .map(|(range_external_id, relative_offset, length)| {
            let offset = entry.offset.checked_add(relative_offset).ok_or_else(|| {
                import_error("CityObject absolute byte offset does not fit in u64")
            })?;
            let fragment = read_exact_range(&entry.path, offset, length)?;
            let (external_id, object) = parse_cityobject_entry(&fragment)?;
            if external_id != range_external_id {
                return Err(import_error(format!(
                    "indexed CityObject member {range_external_id} resolved to fragment for {external_id}"
                )));
            }
            let cityobject_type = cityobject_type(&object, &external_id)?;
            let children = normalized_children(&external_id, &object)?;
            Ok(NormalizedCityObjectScan {
                external_id,
                cityobject_type,
                path: entry.path.clone(),
                offset,
                length,
                children,
            })
        })
        .collect()
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

    fn as_cityjson(&self) -> Option<&CityJsonBackend> {
        None
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

    fn as_cityjson(&self) -> Option<&CityJsonBackend> {
        Some(self)
    }
}

impl CityJsonBackend {
    fn read_package_members(
        &self,
        model_id: &str,
        source_path: &Path,
        members: &[PackageMemberLocation],
        metadata_bytes: &[u8],
    ) -> Result<CityModel> {
        let first_member = members
            .first()
            .ok_or_else(|| import_error(format!("package {model_id} has no CityObject members")))?;
        let vertices_offset = first_member.vertices_offset.ok_or_else(|| {
            Error::UnsupportedFeature(
                "regular CityJSON reads require an indexed shared vertices range".into(),
            )
        })?;
        let vertices_length = first_member.vertices_length.ok_or_else(|| {
            Error::UnsupportedFeature(
                "regular CityJSON reads require an indexed shared vertices range".into(),
            )
        })?;

        let mut source_file = fs::File::open(source_path)?;
        let mut object_entries = Vec::with_capacity(members.len());
        for member in members {
            if member.source_id != first_member.source_id
                || member.source_path != first_member.source_path
            {
                return Err(import_error(format!(
                    "package {model_id} spans multiple CityJSON sources"
                )));
            }
            let object_fragment = read_exact_range_from_file(
                &mut source_file,
                source_path,
                member.offset,
                member.length,
            )?;
            let (object_id, object_value) = parse_cityobject_entry(&object_fragment)?;
            if object_id != member.external_id {
                return Err(import_error(format!(
                    "indexed CityJSON member {} resolved to fragment for {}",
                    member.external_id, object_id
                )));
            }
            object_entries.push((object_id, object_value));
        }
        let shared_vertices = self.load_shared_vertices(
            source_path,
            &mut source_file,
            vertices_offset,
            vertices_length,
        )?;
        let feature_parts =
            build_feature_parts(model_id, object_entries, shared_vertices.as_ref())?;
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
    let (ids, bounds, spatial) = parse_feature_file_bounds(&feature, item.metadata)?;
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
) -> Result<(Vec<String>, FeatureBounds, bool)> {
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
    Ok((ids, bounds, spatial))
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
