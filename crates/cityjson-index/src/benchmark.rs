use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cityjson_lib::{Error, Result};
use clap::{Parser, ValueEnum};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile;
use crate::{BBox, CityIndex, resolve_dataset};

const DEFAULT_CORPUS_ROOT: &str = "/home/balazs/Development/cityjson-corpus";
const DEFAULT_BASISVOORZIENING_ARTIFACT: &str =
    "artifacts/acquired/basisvoorziening-3d/2022/3d_volledig_84000_450000.city.json";
const DEFAULT_WORK_ROOT: &str = "target/benchmarks/basisvoorziening-3d";
const DEFAULT_SUBSET_SIZES: &[usize] = &[1_000, 5_000, 10_000, 25_000];
const DEFAULT_MULTI_SOURCE_SHARDS: usize = 4;
const DEFAULT_MULTI_SOURCE_FEATURES_PER_SHARD: usize = 1_000;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "bench-index",
    about = "Run JSON-emitting CityJSON indexing benchmarks",
    long_about = r#"Run JSON-emitting CityJSON indexing benchmarks.

The benchmark runner prepares Basisvoorziening 3D inputs from the pinned corpus artifact and records one JSON object per measured operation. Default cases include single-source full/subset datasets plus a deterministic multi-source dataset derived from the same pinned artifact. NDJSON and regular CityJSON parallel indexing currently scales across source files; the generated multi-source case is the default parallelism signal. Feature-file datasets scale across feature files under each metadata source.

Each worker-count measurement uses a fresh SQLite index path. Prefer repeated benchmark invocations over a single pass when comparing timings. RSS fields report Linux /proc/self/status snapshots: current_rss_bytes is VmRSS, process_peak_rss_bytes is process-lifetime VmHWM, and peak_rss_bytes is a deprecated compatibility alias for that same process-lifetime peak.
"#
)]
pub struct BenchmarkCli {
    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub json: bool,

    /// Root of the cityjson-corpus checkout.
    #[arg(long, default_value = DEFAULT_CORPUS_ROOT)]
    pub corpus_root: PathBuf,

    /// Benchmark work directory for prepared datasets.
    #[arg(long, default_value = DEFAULT_WORK_ROOT)]
    pub work_root: PathBuf,

    /// Override the pinned Basisvoorziening artifact path.
    #[arg(long)]
    pub artifact: Option<PathBuf>,

    /// Include a benchmark case.
    #[arg(long, value_enum)]
    pub case: Vec<BenchmarkCaseKind>,

    /// Include a prepared storage layout. Defaults to all supported layouts.
    #[arg(long, value_enum)]
    pub layout: Vec<BenchmarkLayoutKind>,

    /// Worker counts to record for each dataset.
    #[arg(long, value_name = "WORKERS")]
    pub workers: Vec<usize>,

    /// Optional root directory containing additional Basisvoorziening tiles.
    #[arg(long)]
    pub multi_tile_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BenchmarkCaseKind {
    SingleTileFull,
    SingleTileSubsets,
    MultiSource,
    MultiTile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum BenchmarkLayoutKind {
    CityJson,
    CityJsonSeq,
    FeatureFiles,
}

impl BenchmarkLayoutKind {
    const ALL: [Self; 3] = [Self::CityJson, Self::CityJsonSeq, Self::FeatureFiles];

    fn as_label(self) -> &'static str {
        match self {
            Self::CityJson => "cityjson",
            Self::CityJsonSeq => "cityjson-seq",
            Self::FeatureFiles => "feature-files",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub runs: Vec<BenchmarkOperationRecord>,
}

#[derive(Debug, Clone)]
struct BenchmarkRecordInput {
    dataset_label: String,
    source_artifact: PathBuf,
    prepared_dataset: PathBuf,
    subset_size: Option<usize>,
    layout: BenchmarkLayoutKind,
    byte_size: u64,
    sidecar_byte_size: u64,
    worker_count: usize,
    operation: String,
    variant: Option<String>,
    elapsed_ns: u64,
    memory: profile::MemorySnapshot,
    feature_count: usize,
    package_count: usize,
    source_count: usize,
    cityobject_count: usize,
    cityobject_relationship_count: usize,
    multi_geometry_cityobject_count: usize,
    query_hit_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkOperationRecord {
    pub dataset_label: String,
    pub source_artifact: PathBuf,
    pub prepared_dataset: PathBuf,
    pub subset_size: Option<usize>,
    pub layout: BenchmarkLayoutKind,
    pub byte_size: u64,
    pub sidecar_byte_size: u64,
    pub worker_count: usize,
    pub operation: String,
    pub variant: Option<String>,
    pub elapsed_ns: u64,
    pub current_rss_bytes: u64,
    pub process_peak_rss_bytes: u64,
    /// Deprecated compatibility field. This is a process-lifetime peak RSS
    /// alias, not an operation-local peak.
    pub peak_rss_bytes: u64,
    pub feature_count: usize,
    pub package_count: usize,
    pub source_count: usize,
    pub cityobject_count: usize,
    pub cityobject_relationship_count: usize,
    pub multi_geometry_cityobject_count: usize,
    pub query_hit_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkManifest {
    dataset_label: String,
    source_artifact: PathBuf,
    prepared_dataset: PathBuf,
    subset_size: Option<usize>,
    layout: BenchmarkLayoutKind,
    byte_size: u64,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
    cityobject_relationship_count: usize,
    multi_geometry_cityobject_count: usize,
    dataset_bbox: BBox,
    representative_feature_ids: Vec<String>,
    query_windows: Vec<QueryWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueryWindow {
    label: String,
    bbox: BBox,
}

#[derive(Debug, Clone)]
struct PreparedDataset {
    manifest: BenchmarkManifest,
}

/// Executes the benchmark suite and returns the collected records.
///
/// # Errors
///
/// Returns an error if the pinned artifact is missing, dataset preparation
/// fails, or any benchmarked operation fails.
pub fn run(cli: &BenchmarkCli) -> Result<BenchmarkReport> {
    let artifact = cli
        .artifact
        .clone()
        .unwrap_or_else(|| cli.corpus_root.join(DEFAULT_BASISVOORZIENING_ARTIFACT));
    if !artifact.exists() {
        return Err(Error::Import(format!(
            "missing pinned Basisvoorziening 3D artifact {}; run `cd /home/balazs/Development/cityjson-corpus && just acquire-basisvoorziening-3d`",
            artifact.display()
        )));
    }

    let cases = if cli.case.is_empty() {
        vec![
            BenchmarkCaseKind::SingleTileFull,
            BenchmarkCaseKind::SingleTileSubsets,
            BenchmarkCaseKind::MultiSource,
        ]
    } else {
        cli.case.clone()
    };
    let worker_counts = worker_counts(cli.workers.clone());
    let layouts = benchmark_layouts(&cli.layout);

    let mut runs = Vec::new();
    for case in cases {
        for layout in &layouts {
            for dataset in prepare_case(cli, case, *layout, &artifact)? {
                for worker_count in &worker_counts {
                    runs.extend(with_worker_count_env(*worker_count, || {
                        run_dataset(&dataset)
                    })?);
                }
            }
        }
    }

    Ok(BenchmarkReport { runs })
}

/// Writes the benchmark report to stdout in either JSON or compact text form.
///
/// # Errors
///
/// Returns an error if writing to stdout fails or JSON serialization fails.
pub fn print_report(report: &BenchmarkReport, json: bool) -> Result<()> {
    if json {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        serde_json::to_writer_pretty(&mut handle, report)
            .map_err(|error| Error::Import(error.to_string()))?;
        handle.write_all(b"\n")?;
        handle.flush()?;
        return Ok(());
    }

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for run in &report.runs {
        writeln!(
            handle,
            "{} worker={} op={} variant={} elapsed_ns={} current_rss_bytes={} process_peak_rss_bytes={} hits={}",
            run.dataset_label,
            run.worker_count,
            run.operation,
            run.variant.as_deref().unwrap_or("-"),
            run.elapsed_ns,
            run.current_rss_bytes,
            run.process_peak_rss_bytes,
            run.query_hit_count
                .map_or_else(|| "-".to_owned(), |count| count.to_string())
        )?;
    }
    handle.flush()?;
    Ok(())
}

fn prepare_case(
    cli: &BenchmarkCli,
    case: BenchmarkCaseKind,
    layout: BenchmarkLayoutKind,
    artifact: &Path,
) -> Result<Vec<PreparedDataset>> {
    match case {
        BenchmarkCaseKind::SingleTileFull => Ok(vec![prepare_single_tile_dataset(
            cli,
            "single-tile-full",
            layout,
            artifact,
            None,
        )?]),
        BenchmarkCaseKind::SingleTileSubsets => DEFAULT_SUBSET_SIZES
            .iter()
            .map(|subset_size| {
                prepare_single_tile_dataset(
                    cli,
                    &format!("single-tile-subset-{subset_size}"),
                    layout,
                    artifact,
                    Some(*subset_size),
                )
            })
            .collect(),
        BenchmarkCaseKind::MultiSource => {
            Ok(vec![prepare_multi_source_dataset(cli, layout, artifact)?])
        }
        BenchmarkCaseKind::MultiTile => prepare_multi_tile_dataset(cli, layout),
    }
}

fn prepare_single_tile_dataset(
    cli: &BenchmarkCli,
    label: &str,
    layout: BenchmarkLayoutKind,
    artifact: &Path,
    subset_size: Option<usize>,
) -> Result<PreparedDataset> {
    let prepared_root = cli.work_root.join(layout.as_label()).join(label);
    reset_dir(&prepared_root)?;
    fs::create_dir_all(&prepared_root)?;

    let bytes = fs::read(artifact)?;
    let mut document: Value =
        serde_json::from_slice(&bytes).map_err(|error| Error::Import(error.to_string()))?;
    let original_bytes = if subset_size.is_none() {
        Some(bytes.clone())
    } else {
        None
    };
    if let Some(limit) = subset_size {
        document = subset_cityjson_document(&mut document, limit)?;
    }
    let feature_count = extract_root_ids(&document)?.len();
    let byte_size = u64::try_from(bytes.len())
        .map_err(|_| Error::Import("prepared dataset size does not fit in u64".to_owned()))?;
    let source = CityJsonSourceDocument {
        file_stem: "dataset".to_owned(),
        document,
        original_bytes,
    };
    let manifest = materialize_layout_dataset(
        label,
        layout,
        artifact,
        &prepared_root,
        subset_size.map(|_| feature_count),
        &[source],
        byte_size,
    )?;
    write_manifest(&prepared_root.join("benchmark-manifest.json"), &manifest)?;
    Ok(PreparedDataset { manifest })
}

fn prepare_multi_source_dataset(
    cli: &BenchmarkCli,
    layout: BenchmarkLayoutKind,
    artifact: &Path,
) -> Result<PreparedDataset> {
    let prepared_root = cli.work_root.join(layout.as_label()).join("multi-source");
    reset_dir(&prepared_root)?;
    fs::create_dir_all(&prepared_root)?;

    let bytes = fs::read(artifact)?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|error| Error::Import(error.to_string()))?;
    let root_ids = extract_root_ids(&document)?;
    if root_ids.len() < 2 {
        return Err(Error::Import(
            "multi-source benchmark preparation requires at least two root CityObjects".to_owned(),
        ));
    }

    let shard_count = DEFAULT_MULTI_SOURCE_SHARDS.min(root_ids.len());
    let total_feature_count =
        (DEFAULT_MULTI_SOURCE_FEATURES_PER_SHARD * shard_count).min(root_ids.len());
    let selected_root_ids = root_ids
        .into_iter()
        .take(total_feature_count)
        .collect::<Vec<_>>();
    let mut sources = Vec::with_capacity(shard_count);
    for shard_index in 0..shard_count {
        let start = shard_index * total_feature_count / shard_count;
        let end = (shard_index + 1) * total_feature_count / shard_count;
        let subset = subset_cityjson_document_by_roots(&document, &selected_root_ids[start..end])?;
        sources.push(CityJsonSourceDocument {
            file_stem: format!("source-{shard_index:02}"),
            document: subset,
            original_bytes: None,
        });
    }

    let byte_size = u64::try_from(bytes.len())
        .map_err(|_| Error::Import("prepared dataset size does not fit in u64".to_owned()))?;
    let manifest = materialize_layout_dataset(
        "multi-source",
        layout,
        artifact,
        &prepared_root,
        None,
        &sources,
        byte_size,
    )?;
    write_manifest(&prepared_root.join("benchmark-manifest.json"), &manifest)?;
    Ok(PreparedDataset { manifest })
}

fn prepare_multi_tile_dataset(
    cli: &BenchmarkCli,
    layout: BenchmarkLayoutKind,
) -> Result<Vec<PreparedDataset>> {
    let multi_root = cli.multi_tile_root.as_ref().ok_or_else(|| {
        Error::Import(
            "multi-tile benchmarking requires --multi-tile-root pointing at extra Basisvoorziening tiles"
                .to_owned(),
        )
    })?;
    if !multi_root.exists() {
        return Err(Error::Import(format!(
            "multi-tile root {} does not exist",
            multi_root.display()
        )));
    }

    let prepared_root = cli.work_root.join(layout.as_label()).join("multi-tile");
    reset_dir(&prepared_root)?;
    fs::create_dir_all(&prepared_root)?;

    let mut sources = Vec::new();
    let mut byte_size = 0u64;
    for entry in WalkBuilder::new(multi_root)
        .hidden(false)
        .follow_links(true)
        .build()
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        let document: Value =
            serde_json::from_slice(&bytes).map_err(|error| Error::Import(error.to_string()))?;
        byte_size = byte_size
            .checked_add(u64::try_from(bytes.len()).map_err(|_| {
                Error::Import("prepared dataset size does not fit in u64".to_owned())
            })?)
            .ok_or_else(|| Error::Import("prepared dataset size overflowed u64".to_owned()))?;
        let stem = entry
            .path()
            .strip_prefix(multi_root)
            .unwrap_or(entry.path())
            .with_extension("")
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "-");
        sources.push(CityJsonSourceDocument {
            file_stem: stem,
            document,
            original_bytes: Some(bytes),
        });
    }
    if sources.is_empty() {
        return Err(Error::Import(format!(
            "multi-tile root {} did not contain any CityJSON tiles",
            multi_root.display()
        )));
    }

    let manifest = materialize_layout_dataset(
        "multi-tile",
        layout,
        multi_root,
        &prepared_root,
        None,
        &sources,
        byte_size,
    )?;
    write_manifest(&prepared_root.join("benchmark-manifest.json"), &manifest)?;
    Ok(vec![PreparedDataset { manifest }])
}

#[derive(Debug, Clone)]
struct CityJsonSourceDocument {
    file_stem: String,
    document: Value,
    original_bytes: Option<Vec<u8>>,
}

fn benchmark_layouts(requested: &[BenchmarkLayoutKind]) -> Vec<BenchmarkLayoutKind> {
    if requested.is_empty() {
        return BenchmarkLayoutKind::ALL.to_vec();
    }
    let mut layouts = requested.to_vec();
    layouts.sort_by_key(|layout| match layout {
        BenchmarkLayoutKind::CityJson => 0,
        BenchmarkLayoutKind::CityJsonSeq => 1,
        BenchmarkLayoutKind::FeatureFiles => 2,
    });
    layouts.dedup();
    layouts
}

fn materialize_layout_dataset(
    label: &str,
    layout: BenchmarkLayoutKind,
    source_artifact: &Path,
    prepared_root: &Path,
    subset_size: Option<usize>,
    sources: &[CityJsonSourceDocument],
    _source_byte_size: u64,
) -> Result<BenchmarkManifest> {
    match layout {
        BenchmarkLayoutKind::CityJson => materialize_cityjson_dataset(sources, prepared_root)?,
        BenchmarkLayoutKind::CityJsonSeq => {
            materialize_cityjson_seq_dataset(sources, prepared_root)?;
        }
        BenchmarkLayoutKind::FeatureFiles => {
            materialize_feature_files_dataset(sources, prepared_root)?;
        }
    }

    let mut feature_count = 0usize;
    let mut cityobject_count = 0usize;
    let mut relationship_count = 0usize;
    let mut multi_geometry_count = 0usize;
    let mut all_ids = Vec::new();
    let mut bbox: Option<BBox> = None;

    for source in sources {
        let ids = extract_root_ids(&source.document)?;
        feature_count += ids.len();
        cityobject_count += count_cityobjects(&source.document)?;
        relationship_count += count_cityobject_relationships(&source.document)?;
        multi_geometry_count += count_multi_geometry_cityobjects(&source.document)?;
        all_ids.extend(ids);
        bbox = Some(match bbox {
            None => bbox_for_cityjson_document(&source.document)?,
            Some(existing) => existing.union(&bbox_for_cityjson_document(&source.document)?),
        });
    }

    let dataset_bbox = bbox.unwrap_or(BBox {
        min_x: 0.0,
        max_x: 0.0,
        min_y: 0.0,
        max_y: 0.0,
    });

    Ok(BenchmarkManifest {
        dataset_label: format!("{label}-{}", layout.as_label()),
        source_artifact: source_artifact.to_path_buf(),
        prepared_dataset: prepared_root.to_path_buf(),
        subset_size,
        layout,
        byte_size: total_file_size(prepared_root)?,
        feature_count,
        source_count: sources.len(),
        cityobject_count,
        cityobject_relationship_count: relationship_count,
        multi_geometry_cityobject_count: multi_geometry_count,
        dataset_bbox,
        representative_feature_ids: representative_feature_ids(&all_ids),
        query_windows: build_query_windows(dataset_bbox),
    })
}

fn materialize_cityjson_dataset(
    sources: &[CityJsonSourceDocument],
    prepared_root: &Path,
) -> Result<()> {
    for source in sources {
        let path = prepared_root.join(format!("{}.city.json", source.file_stem));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(bytes) = &source.original_bytes {
            fs::write(path, bytes)?;
        } else {
            let bytes = serde_json::to_vec(&source.document)
                .map_err(|error| Error::Import(error.to_string()))?;
            fs::write(path, bytes)?;
        }
    }
    Ok(())
}

fn materialize_cityjson_seq_dataset(
    sources: &[CityJsonSourceDocument],
    prepared_root: &Path,
) -> Result<()> {
    for source in sources {
        let path = prepared_root.join(format!("{}.city.jsonl", source.file_stem));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        write_json_line(&mut file, &cityjson_base_document(&source.document)?)?;
        for feature in feature_documents_for_roots(&source.document)? {
            write_json_line(&mut file, &feature)?;
        }
    }
    Ok(())
}

fn materialize_feature_files_dataset(
    sources: &[CityJsonSourceDocument],
    prepared_root: &Path,
) -> Result<()> {
    for source in sources {
        let source_root = if sources.len() == 1 {
            prepared_root.to_path_buf()
        } else {
            prepared_root.join(&source.file_stem)
        };
        fs::create_dir_all(source_root.join("features"))?;
        let metadata_path = source_root.join("metadata.json");
        let metadata = cityjson_base_document(&source.document)?;
        fs::write(
            metadata_path,
            serde_json::to_vec(&metadata).map_err(|error| Error::Import(error.to_string()))?,
        )?;
        for feature in feature_documents_for_roots(&source.document)? {
            let feature_id = feature
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::Import("CityJSONFeature is missing id".to_owned()))?;
            let path = source_root
                .join("features")
                .join(format!("{}.city.jsonl", safe_file_stem(feature_id)));
            write_json_line(&mut fs::File::create(path)?, &feature)?;
        }
    }
    Ok(())
}

fn write_json_line(file: &mut fs::File, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *file, value).map_err(|error| Error::Import(error.to_string()))?;
    file.write_all(b"\n")?;
    Ok(())
}

fn cityjson_base_document(document: &Value) -> Result<Value> {
    let mut metadata = document.clone();
    let root = metadata
        .as_object_mut()
        .ok_or_else(|| Error::Import("CityJSON document must be an object".to_owned()))?;
    root.insert(
        "CityObjects".to_owned(),
        Value::Object(serde_json::Map::new()),
    );
    root.insert("vertices".to_owned(), Value::Array(Vec::new()));
    Ok(metadata)
}

fn feature_documents_for_roots(document: &Value) -> Result<Vec<Value>> {
    extract_root_ids(document)?
        .into_iter()
        .map(|root_id| cityjson_feature_for_root(document, &root_id))
        .collect()
}

fn cityjson_feature_for_root(document: &Value, root_id: &str) -> Result<Value> {
    let cityobjects = document
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Import("CityJSON document is missing CityObjects".to_owned()))?;
    let vertices = document
        .get("vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Import("CityJSON document is missing vertices".to_owned()))?;

    let mut selected_ids = BTreeSet::new();
    collect_cityobject_closure(root_id, cityobjects, &mut selected_ids)?;

    let mut selected_cityobjects = BTreeMap::new();
    for id in &selected_ids {
        let object = cityobjects
            .get(id)
            .ok_or_else(|| Error::Import(format!("CityObject {id} was not found")))?;
        let mut object = object.clone();
        filter_cityobject_relationships(&mut object, &selected_ids)?;
        selected_cityobjects.insert(id.clone(), object);
    }

    let mut referenced_vertices = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_object_vertex_indices(
        &selected_cityobjects,
        root_id,
        &mut referenced_vertices,
        &mut visited,
    )?;

    let mut remap = HashMap::new();
    let mut local_vertices = Vec::with_capacity(referenced_vertices.len());
    for (new_index, old_index) in referenced_vertices.iter().enumerate() {
        remap.insert(*old_index, new_index);
        let vertex = vertices
            .get(*old_index)
            .ok_or_else(|| Error::Import(format!("vertex index {old_index} is out of bounds")))?;
        local_vertices.push(vertex.clone());
    }

    for object in selected_cityobjects.values_mut() {
        if let Some(geometries) = object
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

    let mut feature = serde_json::Map::new();
    feature.insert(
        "type".to_owned(),
        Value::String("CityJSONFeature".to_owned()),
    );
    feature.insert("id".to_owned(), Value::String(root_id.to_owned()));
    feature.insert(
        "CityObjects".to_owned(),
        Value::Object(selected_cityobjects.into_iter().collect()),
    );
    feature.insert("vertices".to_owned(), Value::Array(local_vertices));
    Ok(Value::Object(feature))
}

fn safe_file_stem(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '_',
        })
        .collect()
}

fn total_file_size(root: &Path) -> Result<u64> {
    let mut total = 0u64;
    for entry in WalkBuilder::new(root)
        .hidden(false)
        .follow_links(true)
        .build()
    {
        let entry = entry.map_err(|error| Error::Import(error.to_string()))?;
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        total = total
            .checked_add(
                entry
                    .metadata()
                    .map_err(|error| Error::Import(error.to_string()))?
                    .len(),
            )
            .ok_or_else(|| Error::Import("prepared dataset size overflowed u64".to_owned()))?;
    }
    Ok(total)
}

#[allow(
    clippy::too_many_lines,
    reason = "benchmark orchestration keeps measured run order explicit"
)]
fn run_dataset(dataset: &PreparedDataset) -> Result<Vec<BenchmarkOperationRecord>> {
    let manifest = &dataset.manifest;
    let worker_count = crate::configured_worker_count()?;
    let index_path = fresh_benchmark_index_path(manifest, worker_count)?;
    let resolved = resolve_dataset(&manifest.prepared_dataset, Some(index_path))?;

    let open_started = Instant::now();
    let index = CityIndex::open(resolved.storage_layout(), &resolved.index_path)?;
    let open_elapsed = u64::try_from(open_started.elapsed().as_nanos())
        .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
    let open_ended = profile::current_memory_snapshot()?;

    let mut index = index;
    let index_started = Instant::now();
    index.reindex()?;
    let index_elapsed = u64::try_from(index_started.elapsed().as_nanos())
        .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
    let index_ended = profile::current_memory_snapshot()?;

    let feature_count = index.package_count()?;
    let source_count = index.source_count()?;
    let cityobject_count = index.cityobject_count()?;
    let sidecar_byte_size = fs::metadata(&resolved.index_path).map_or(0, |metadata| metadata.len());

    let mut runs = vec![
        build_record(BenchmarkRecordInput {
            dataset_label: manifest.dataset_label.clone(),
            source_artifact: manifest.source_artifact.clone(),
            prepared_dataset: manifest.prepared_dataset.clone(),
            subset_size: manifest.subset_size,
            layout: manifest.layout,
            byte_size: manifest.byte_size,
            sidecar_byte_size,
            worker_count,
            operation: "dataset_open".to_owned(),
            variant: None,
            elapsed_ns: open_elapsed,
            memory: open_ended,
            feature_count,
            package_count: feature_count,
            source_count,
            cityobject_count,
            cityobject_relationship_count: manifest.cityobject_relationship_count,
            multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
            query_hit_count: None,
        }),
        build_record(BenchmarkRecordInput {
            dataset_label: manifest.dataset_label.clone(),
            source_artifact: manifest.source_artifact.clone(),
            prepared_dataset: manifest.prepared_dataset.clone(),
            subset_size: manifest.subset_size,
            layout: manifest.layout,
            byte_size: manifest.byte_size,
            sidecar_byte_size,
            worker_count,
            operation: "index_reindex".to_owned(),
            variant: None,
            elapsed_ns: index_elapsed,
            memory: index_ended,
            feature_count,
            package_count: feature_count,
            source_count,
            cityobject_count,
            cityobject_relationship_count: manifest.cityobject_relationship_count,
            multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
            query_hit_count: None,
        }),
    ];

    let all_refs = index.package_ref_page_after_record_id(None, feature_count.min(256))?;
    let sampled_refs = all_refs.into_iter().take(256).collect::<Vec<_>>();
    let sampled_cityobjects =
        index.cityobject_ref_page_after_record_id(None, cityobject_count.min(256))?;

    runs.extend(run_full_scan(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
    )?);
    runs.extend(run_cityobject_full_scan(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
    )?);
    runs.extend(run_gets(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
    )?);
    runs.push(run_cityobject_id_lookup(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
        &sampled_cityobjects,
    )?);
    runs.extend(run_package_bbox_lookup_only(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
    )?);
    runs.extend(run_cityobject_queries(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
    )?);
    runs.extend(run_queries(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
    )?);
    runs.push(run_read_package(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
        &sampled_refs,
    )?);
    runs.push(run_read_packages(
        &index,
        manifest,
        worker_count,
        feature_count,
        source_count,
        cityobject_count,
        &sampled_refs,
    )?);

    Ok(runs)
}

fn fresh_benchmark_index_path(
    manifest: &BenchmarkManifest,
    worker_count: usize,
) -> Result<PathBuf> {
    let index_path = manifest
        .prepared_dataset
        .join(format!(".cityjson-index.worker-{worker_count}.sqlite"));
    remove_file_if_exists(&index_path)?;
    remove_file_if_exists(&index_path.with_extension("sqlite-wal"))?;
    remove_file_if_exists(&index_path.with_extension("sqlite-shm"))?;
    Ok(index_path)
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn run_full_scan(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
) -> Result<Vec<BenchmarkOperationRecord>> {
    let started = Instant::now();
    let mut count = 0usize;
    let mut after_record_id = None;
    loop {
        let page = index.package_ref_page_after_record_id(after_record_id, 512)?;
        if page.is_empty() {
            break;
        }
        after_record_id = page.last().map(|package| package.record_id);
        count += page.len();
    }
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
    let memory = profile::current_memory_snapshot()?;
    Ok(vec![build_record(BenchmarkRecordInput {
        dataset_label: manifest.dataset_label.clone(),
        source_artifact: manifest.source_artifact.clone(),
        prepared_dataset: manifest.prepared_dataset.clone(),
        subset_size: manifest.subset_size,
        byte_size: manifest.byte_size,
        layout: manifest.layout,
        sidecar_byte_size: fs::metadata(
            manifest
                .prepared_dataset
                .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
        )
        .map_or(0, |metadata| metadata.len()),
        worker_count,
        operation: "full_scan_reference_iteration".to_owned(),
        variant: None,
        elapsed_ns,
        memory,
        feature_count,
        package_count: feature_count,
        source_count,
        cityobject_count,
        cityobject_relationship_count: manifest.cityobject_relationship_count,
        multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
        query_hit_count: Some(count),
    })])
}

fn run_cityobject_full_scan(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
) -> Result<Vec<BenchmarkOperationRecord>> {
    let started = Instant::now();
    let mut count = 0usize;
    let mut after_record_id = None;
    loop {
        let page = index.cityobject_ref_page_after_record_id(after_record_id, 512)?;
        if page.is_empty() {
            break;
        }
        after_record_id = page.last().map(|cityobject| cityobject.record_id);
        count += page.len();
    }
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
    let memory = profile::current_memory_snapshot()?;
    Ok(vec![build_record(BenchmarkRecordInput {
        dataset_label: manifest.dataset_label.clone(),
        source_artifact: manifest.source_artifact.clone(),
        prepared_dataset: manifest.prepared_dataset.clone(),
        subset_size: manifest.subset_size,
        byte_size: manifest.byte_size,
        layout: manifest.layout,
        sidecar_byte_size: fs::metadata(
            manifest
                .prepared_dataset
                .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
        )
        .map_or(0, |metadata| metadata.len()),
        worker_count,
        operation: "cityobject_full_scan_reference_iteration".to_owned(),
        variant: None,
        elapsed_ns,
        memory,
        feature_count,
        package_count: feature_count,
        source_count,
        cityobject_count,
        cityobject_relationship_count: manifest.cityobject_relationship_count,
        multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
        query_hit_count: Some(count),
    })])
}

fn run_gets(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
) -> Result<Vec<BenchmarkOperationRecord>> {
    let mut runs = Vec::new();
    for feature_id in representative_ids(manifest, feature_count) {
        let started = Instant::now();
        let hit = index.get_packages(&feature_id)?;
        let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
            .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
        let memory = profile::current_memory_snapshot()?;
        runs.push(build_record(BenchmarkRecordInput {
            dataset_label: manifest.dataset_label.clone(),
            source_artifact: manifest.source_artifact.clone(),
            prepared_dataset: manifest.prepared_dataset.clone(),
            subset_size: manifest.subset_size,
            layout: manifest.layout,
            byte_size: manifest.byte_size,
            sidecar_byte_size: fs::metadata(
                manifest
                    .prepared_dataset
                    .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
            )
            .map_or(0, |metadata| metadata.len()),
            worker_count,
            operation: "get".to_owned(),
            variant: Some(feature_id),
            elapsed_ns,
            memory,
            feature_count,
            package_count: feature_count,
            source_count,
            cityobject_count,
            cityobject_relationship_count: manifest.cityobject_relationship_count,
            multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
            query_hit_count: Some(hit.len()),
        }));
    }
    Ok(runs)
}

fn run_cityobject_id_lookup(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
    refs: &[crate::IndexedCityObjectRef],
) -> Result<BenchmarkOperationRecord> {
    let ids = refs
        .iter()
        .map(|cityobject| cityobject.external_id.as_str())
        .collect::<Vec<_>>();
    let started = Instant::now();
    let hits = index.lookup_cityobject_refs_for_ids(&ids)?;
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
    let memory = profile::current_memory_snapshot()?;
    Ok(build_record(BenchmarkRecordInput {
        dataset_label: manifest.dataset_label.clone(),
        source_artifact: manifest.source_artifact.clone(),
        prepared_dataset: manifest.prepared_dataset.clone(),
        subset_size: manifest.subset_size,
        layout: manifest.layout,
        byte_size: manifest.byte_size,
        sidecar_byte_size: fs::metadata(
            manifest
                .prepared_dataset
                .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
        )
        .map_or(0, |metadata| metadata.len()),
        worker_count,
        operation: "cityobject_id_lookup".to_owned(),
        variant: Some(format!("sample-{}", ids.len())),
        elapsed_ns,
        memory,
        feature_count,
        package_count: feature_count,
        source_count,
        cityobject_count,
        cityobject_relationship_count: manifest.cityobject_relationship_count,
        multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
        query_hit_count: Some(hits.len()),
    }))
}

fn run_package_bbox_lookup_only(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
) -> Result<Vec<BenchmarkOperationRecord>> {
    let mut runs = Vec::new();
    for window in &manifest.query_windows {
        let started = Instant::now();
        let hits = index.query_package_refs(&window.bbox)?;
        let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
            .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
        let memory = profile::current_memory_snapshot()?;
        runs.push(build_record(BenchmarkRecordInput {
            dataset_label: manifest.dataset_label.clone(),
            source_artifact: manifest.source_artifact.clone(),
            prepared_dataset: manifest.prepared_dataset.clone(),
            subset_size: manifest.subset_size,
            layout: manifest.layout,
            byte_size: manifest.byte_size,
            sidecar_byte_size: fs::metadata(
                manifest
                    .prepared_dataset
                    .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
            )
            .map_or(0, |metadata| metadata.len()),
            worker_count,
            operation: "package_bbox_lookup_only".to_owned(),
            variant: Some(window.label.clone()),
            elapsed_ns,
            memory,
            feature_count,
            package_count: feature_count,
            source_count,
            cityobject_count,
            cityobject_relationship_count: manifest.cityobject_relationship_count,
            multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
            query_hit_count: Some(hits.len()),
        }));
    }
    Ok(runs)
}

fn run_cityobject_queries(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
) -> Result<Vec<BenchmarkOperationRecord>> {
    let mut runs = Vec::new();
    for window in &manifest.query_windows {
        let started = Instant::now();
        let hits = index.query_cityobject_refs(&window.bbox)?;
        let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
            .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
        let memory = profile::current_memory_snapshot()?;
        runs.push(build_record(BenchmarkRecordInput {
            dataset_label: manifest.dataset_label.clone(),
            source_artifact: manifest.source_artifact.clone(),
            prepared_dataset: manifest.prepared_dataset.clone(),
            subset_size: manifest.subset_size,
            layout: manifest.layout,
            byte_size: manifest.byte_size,
            sidecar_byte_size: fs::metadata(
                manifest
                    .prepared_dataset
                    .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
            )
            .map_or(0, |metadata| metadata.len()),
            worker_count,
            operation: "cityobject_bbox_query".to_owned(),
            variant: Some(window.label.clone()),
            elapsed_ns,
            memory,
            feature_count,
            package_count: feature_count,
            source_count,
            cityobject_count,
            cityobject_relationship_count: manifest.cityobject_relationship_count,
            multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
            query_hit_count: Some(hits.len()),
        }));
    }
    Ok(runs)
}

fn run_queries(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
) -> Result<Vec<BenchmarkOperationRecord>> {
    let mut runs = Vec::new();
    for window in &manifest.query_windows {
        let started = Instant::now();
        let hits = index.query_package_refs(&window.bbox)?;
        let _packages = index.read_packages(&hits)?;
        let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
            .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
        let memory = profile::current_memory_snapshot()?;
        runs.push(build_record(BenchmarkRecordInput {
            dataset_label: manifest.dataset_label.clone(),
            source_artifact: manifest.source_artifact.clone(),
            prepared_dataset: manifest.prepared_dataset.clone(),
            subset_size: manifest.subset_size,
            layout: manifest.layout,
            byte_size: manifest.byte_size,
            sidecar_byte_size: fs::metadata(
                manifest
                    .prepared_dataset
                    .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
            )
            .map_or(0, |metadata| metadata.len()),
            worker_count,
            operation: "bbox_query".to_owned(),
            variant: Some(window.label.clone()),
            elapsed_ns,
            memory,
            feature_count,
            package_count: feature_count,
            source_count,
            cityobject_count,
            cityobject_relationship_count: manifest.cityobject_relationship_count,
            multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
            query_hit_count: Some(hits.len()),
        }));
    }
    Ok(runs)
}

fn run_read_package(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
    refs: &[crate::IndexedPackageRef],
) -> Result<BenchmarkOperationRecord> {
    let started = Instant::now();
    let mut reconstructed = 0usize;
    for package in refs {
        let _model = index.read_package(package)?;
        reconstructed += 1;
    }
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
    let memory = profile::current_memory_snapshot()?;
    Ok(build_record(BenchmarkRecordInput {
        dataset_label: manifest.dataset_label.clone(),
        source_artifact: manifest.source_artifact.clone(),
        prepared_dataset: manifest.prepared_dataset.clone(),
        subset_size: manifest.subset_size,
        byte_size: manifest.byte_size,
        layout: manifest.layout,
        sidecar_byte_size: fs::metadata(
            manifest
                .prepared_dataset
                .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
        )
        .map_or(0, |metadata| metadata.len()),
        worker_count,
        operation: "read_package".to_owned(),
        variant: Some(format!("sample-{}", refs.len())),
        elapsed_ns,
        memory,
        feature_count,
        package_count: feature_count,
        source_count,
        cityobject_count,
        cityobject_relationship_count: manifest.cityobject_relationship_count,
        multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
        query_hit_count: Some(reconstructed),
    }))
}

fn run_read_packages(
    index: &CityIndex,
    manifest: &BenchmarkManifest,
    worker_count: usize,
    feature_count: usize,
    source_count: usize,
    cityobject_count: usize,
    refs: &[crate::IndexedPackageRef],
) -> Result<BenchmarkOperationRecord> {
    let started = Instant::now();
    let packages = index.read_packages(refs)?;
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| Error::Import("benchmark elapsed time does not fit in u64".to_owned()))?;
    let memory = profile::current_memory_snapshot()?;
    Ok(build_record(BenchmarkRecordInput {
        dataset_label: manifest.dataset_label.clone(),
        source_artifact: manifest.source_artifact.clone(),
        prepared_dataset: manifest.prepared_dataset.clone(),
        subset_size: manifest.subset_size,
        byte_size: manifest.byte_size,
        layout: manifest.layout,
        sidecar_byte_size: fs::metadata(
            manifest
                .prepared_dataset
                .join(format!(".cityjson-index.worker-{worker_count}.sqlite")),
        )
        .map_or(0, |metadata| metadata.len()),
        worker_count,
        operation: "read_packages".to_owned(),
        variant: Some(format!("sample-{}", refs.len())),
        elapsed_ns,
        memory,
        feature_count,
        package_count: feature_count,
        source_count,
        cityobject_count,
        cityobject_relationship_count: manifest.cityobject_relationship_count,
        multi_geometry_cityobject_count: manifest.multi_geometry_cityobject_count,
        query_hit_count: Some(packages.len()),
    }))
}

fn build_record(input: BenchmarkRecordInput) -> BenchmarkOperationRecord {
    BenchmarkOperationRecord {
        dataset_label: input.dataset_label,
        source_artifact: input.source_artifact,
        prepared_dataset: input.prepared_dataset,
        subset_size: input.subset_size,
        layout: input.layout,
        byte_size: input.byte_size,
        sidecar_byte_size: input.sidecar_byte_size,
        worker_count: input.worker_count,
        operation: input.operation,
        variant: input.variant,
        elapsed_ns: input.elapsed_ns,
        current_rss_bytes: input.memory.current_rss_bytes,
        process_peak_rss_bytes: input.memory.process_peak_rss_bytes,
        peak_rss_bytes: input.memory.peak_rss_bytes,
        feature_count: input.feature_count,
        package_count: input.package_count,
        source_count: input.source_count,
        cityobject_count: input.cityobject_count,
        cityobject_relationship_count: input.cityobject_relationship_count,
        multi_geometry_cityobject_count: input.multi_geometry_cityobject_count,
        query_hit_count: input.query_hit_count,
    }
}

fn representative_ids(manifest: &BenchmarkManifest, feature_count: usize) -> Vec<String> {
    if manifest.representative_feature_ids.is_empty() {
        return Vec::new();
    }
    let mut ids = manifest.representative_feature_ids.clone();
    ids.truncate(ids.len().min(feature_count.max(1)));
    ids
}

fn worker_counts(mut requested: Vec<usize>) -> Vec<usize> {
    if requested.is_empty() {
        requested = vec![
            1,
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
            4,
        ];
    }
    requested.sort_unstable();
    requested.dedup();
    requested
}

fn with_worker_count_env<T>(worker_count: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
    struct WorkerCountEnvGuard {
        previous: Option<std::ffi::OsString>,
    }

    impl Drop for WorkerCountEnvGuard {
        fn drop(&mut self) {
            // SAFETY: the benchmark runner sets and restores the variable on the
            // current thread immediately around a single indexing run.
            unsafe {
                match self.previous.take() {
                    Some(previous) => std::env::set_var(crate::WORKER_COUNT_ENV, previous),
                    None => std::env::remove_var(crate::WORKER_COUNT_ENV),
                }
            }
        }
    }

    let previous = std::env::var_os(crate::WORKER_COUNT_ENV);
    let _guard = WorkerCountEnvGuard { previous };
    // SAFETY: the benchmark process is single-threaded around environment
    // mutation for a given run, and the variable is restored by the guard.
    unsafe {
        std::env::set_var(crate::WORKER_COUNT_ENV, worker_count.to_string());
    }
    f()
}

fn reset_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn write_manifest(path: &Path, manifest: &BenchmarkManifest) -> Result<()> {
    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, manifest).map_err(|error| Error::Import(error.to_string()))
}

fn build_query_windows(bbox: BBox) -> Vec<QueryWindow> {
    vec![
        QueryWindow {
            label: "small".to_owned(),
            bbox: shrink_bbox(bbox, 0.01),
        },
        QueryWindow {
            label: "medium".to_owned(),
            bbox: shrink_bbox(bbox, 0.10),
        },
        QueryWindow {
            label: "large".to_owned(),
            bbox: shrink_bbox(bbox, 0.50),
        },
        QueryWindow {
            label: "full".to_owned(),
            bbox,
        },
    ]
}

fn shrink_bbox(bbox: BBox, fraction: f64) -> BBox {
    let width = (bbox.max_x - bbox.min_x).abs();
    let height = (bbox.max_y - bbox.min_y).abs();
    if width == 0.0 || height == 0.0 {
        return bbox;
    }
    let x_pad = width * (1.0 - fraction) / 2.0;
    let y_pad = height * (1.0 - fraction) / 2.0;
    BBox {
        min_x: bbox.min_x + x_pad,
        max_x: bbox.max_x - x_pad,
        min_y: bbox.min_y + y_pad,
        max_y: bbox.max_y - y_pad,
    }
}

fn representative_feature_ids(feature_ids: &[String]) -> Vec<String> {
    if feature_ids.is_empty() {
        return Vec::new();
    }
    let mut selected = Vec::new();
    selected.push(feature_ids[0].clone());
    if feature_ids.len() > 2 {
        selected.push(feature_ids[feature_ids.len() / 2].clone());
    }
    if feature_ids.len() > 1 {
        selected.push(feature_ids[feature_ids.len() - 1].clone());
    }
    selected.sort();
    selected.dedup();
    selected
}

fn extract_root_ids(document: &Value) -> Result<Vec<String>> {
    let cityobjects = document
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Import("CityJSON document is missing CityObjects".to_owned()))?;

    let mut child_ids = BTreeSet::new();
    for object in cityobjects.values() {
        if let Some(children) = object.get("children").and_then(Value::as_array) {
            for child in children {
                if let Some(child_id) = child.as_str() {
                    child_ids.insert(child_id.to_owned());
                }
            }
        }
    }

    let mut ids = cityobjects
        .iter()
        .filter(|(id, object)| {
            object
                .get("parents")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
                && !child_ids.contains(id.as_str())
        })
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    Ok(ids)
}

fn count_cityobjects(document: &Value) -> Result<usize> {
    let cityobjects = document
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Import("CityJSON document is missing CityObjects".to_owned()))?;
    Ok(cityobjects.len())
}

fn count_cityobject_relationships(document: &Value) -> Result<usize> {
    let cityobjects = document
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Import("CityJSON document is missing CityObjects".to_owned()))?;
    let mut relationships = BTreeSet::new();
    for (object_id, object) in cityobjects {
        if let Some(children) = object.get("children").and_then(Value::as_array) {
            for child in children {
                if let Some(child_id) = child.as_str() {
                    relationships.insert((object_id.clone(), child_id.to_owned()));
                }
            }
        }
        if let Some(parents) = object.get("parents").and_then(Value::as_array) {
            for parent in parents {
                if let Some(parent_id) = parent.as_str() {
                    relationships.insert((parent_id.to_owned(), object_id.clone()));
                }
            }
        }
    }
    Ok(relationships.len())
}

fn count_multi_geometry_cityobjects(document: &Value) -> Result<usize> {
    let cityobjects = document
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Import("CityJSON document is missing CityObjects".to_owned()))?;
    Ok(cityobjects
        .values()
        .filter(|object| {
            object
                .get("geometry")
                .and_then(Value::as_array)
                .is_some_and(|geometries| geometries.len() > 1)
        })
        .count())
}

fn bbox_for_cityjson_document(document: &Value) -> Result<BBox> {
    let vertices = document
        .get("vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Import("CityJSON document is missing vertices".to_owned()))?;
    let transform = document
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Import("CityJSON document is missing transform".to_owned()))?;
    let scale = parse_transform_component(transform, "scale")?;
    let translate = parse_transform_component(transform, "translate")?;

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for vertex in vertices {
        let coords = vertex
            .as_array()
            .ok_or_else(|| Error::Import("vertex must be an array".to_owned()))?;
        if coords.len() != 3 {
            return Err(Error::Import(
                "vertex must have three coordinates".to_owned(),
            ));
        }
        let x = translate[0]
            + scale[0]
                * coords[0].as_f64().ok_or_else(|| {
                    Error::Import("vertex coordinates must be numeric".to_owned())
                })?;
        let y = translate[1]
            + scale[1]
                * coords[1].as_f64().ok_or_else(|| {
                    Error::Import("vertex coordinates must be numeric".to_owned())
                })?;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    Ok(BBox {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn parse_transform_component(
    transform: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<[f64; 3]> {
    let values = transform
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Import(format!("transform is missing {key}")))?;
    if values.len() != 3 {
        return Err(Error::Import(format!(
            "transform {key} must contain three values"
        )));
    }
    Ok([
        values[0]
            .as_f64()
            .ok_or_else(|| Error::Import("transform values must be numeric".to_owned()))?,
        values[1]
            .as_f64()
            .ok_or_else(|| Error::Import("transform values must be numeric".to_owned()))?,
        values[2]
            .as_f64()
            .ok_or_else(|| Error::Import("transform values must be numeric".to_owned()))?,
    ])
}

fn subset_cityjson_document(document: &mut Value, limit: usize) -> Result<Value> {
    let root_ids = extract_root_ids(document)?;
    let selected_roots = root_ids.into_iter().take(limit).collect::<Vec<_>>();
    subset_cityjson_document_by_roots(document, &selected_roots)
}

fn subset_cityjson_document_by_roots(document: &Value, selected_roots: &[String]) -> Result<Value> {
    let cityobjects = document
        .get("CityObjects")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Import("CityJSON document is missing CityObjects".to_owned()))?
        .clone();
    let vertices = document
        .get("vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Import("CityJSON document is missing vertices".to_owned()))?
        .clone();
    let mut selected_ids = BTreeSet::new();
    for root_id in selected_roots {
        collect_cityobject_closure(root_id, &cityobjects, &mut selected_ids)?;
    }

    let mut selected_cityobjects = BTreeMap::new();
    for id in &selected_ids {
        let object = cityobjects
            .get(id)
            .ok_or_else(|| Error::Import(format!("CityObject {id} was not found")))?;
        let mut object = object.clone();
        filter_cityobject_relationships(&mut object, &selected_ids)?;
        selected_cityobjects.insert(id.clone(), object);
    }

    let mut referenced_vertices = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for id in selected_roots {
        collect_object_vertex_indices(
            &selected_cityobjects,
            id,
            &mut referenced_vertices,
            &mut visited,
        )?;
    }

    let mut remap = HashMap::new();
    let mut local_vertices = Vec::with_capacity(referenced_vertices.len());
    for (new_index, old_index) in referenced_vertices.iter().enumerate() {
        remap.insert(*old_index, new_index);
        let vertex = vertices
            .get(*old_index)
            .ok_or_else(|| Error::Import(format!("vertex index {old_index} is out of bounds")))?;
        local_vertices.push(vertex.clone());
    }

    for object in selected_cityobjects.values_mut() {
        if let Some(geometries) = object
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

    let mut root = document.clone();
    let root_object = root
        .as_object_mut()
        .ok_or_else(|| Error::Import("CityJSON document must be an object".to_owned()))?;
    root_object.insert(
        "CityObjects".to_owned(),
        Value::Object(selected_cityobjects.into_iter().collect()),
    );
    root_object.insert("vertices".to_owned(), Value::Array(local_vertices));
    Ok(root)
}

fn collect_cityobject_closure(
    object_id: &str,
    cityobjects: &serde_json::Map<String, Value>,
    selected_ids: &mut BTreeSet<String>,
) -> Result<()> {
    if !selected_ids.insert(object_id.to_owned()) {
        return Ok(());
    }
    let object = cityobjects
        .get(object_id)
        .ok_or_else(|| Error::Import(format!("CityObject {object_id} was not found")))?;
    if let Some(children) = object.get("children").and_then(Value::as_array) {
        for child in children {
            let child_id = child
                .as_str()
                .ok_or_else(|| Error::Import("CityObject children must be strings".to_owned()))?;
            if cityobjects.contains_key(child_id) {
                collect_cityobject_closure(child_id, cityobjects, selected_ids)?;
            }
        }
    }
    Ok(())
}

fn filter_cityobject_relationships(
    object: &mut Value,
    selected_ids: &BTreeSet<String>,
) -> Result<()> {
    let object = object
        .as_object_mut()
        .ok_or_else(|| Error::Import("CityObject must be an object".to_owned()))?;
    for key in ["children", "parents"] {
        let remove_key = match object.get_mut(key) {
            Some(value) => {
                let refs = value
                    .as_array_mut()
                    .ok_or_else(|| Error::Import(format!("{key} must be an array")))?;
                refs.retain(|entry| {
                    entry
                        .as_str()
                        .is_some_and(|object_id| selected_ids.contains(object_id))
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

fn collect_object_vertex_indices(
    cityobjects: &BTreeMap<String, Value>,
    object_id: &str,
    indices: &mut BTreeSet<usize>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    if !visited.insert(object_id.to_owned()) {
        return Ok(());
    }
    let object = cityobjects
        .get(object_id)
        .ok_or_else(|| Error::Import(format!("CityObject {object_id} was not found")))?;
    if let Some(geometries) = object.get("geometry").and_then(Value::as_array) {
        for geometry in geometries {
            if let Some(boundaries) = geometry.get("boundaries") {
                collect_vertex_indices_from_value(boundaries, indices)?;
            }
        }
    }
    if let Some(children) = object.get("children").and_then(Value::as_array) {
        for child in children {
            let child_id = child
                .as_str()
                .ok_or_else(|| Error::Import("CityObject children must be strings".to_owned()))?;
            if cityobjects.contains_key(child_id) {
                collect_object_vertex_indices(cityobjects, child_id, indices, visited)?;
            }
        }
    }
    Ok(())
}

fn collect_vertex_indices_from_value(value: &Value, indices: &mut BTreeSet<usize>) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_vertex_indices_from_value(item, indices)?;
            }
            Ok(())
        }
        Value::Number(number) => {
            let index = number.as_u64().ok_or_else(|| {
                Error::Import("vertex indices must be non-negative integers".to_owned())
            })?;
            let index = usize::try_from(index)
                .map_err(|_| Error::Import("vertex index does not fit in usize".to_owned()))?;
            indices.insert(index);
            Ok(())
        }
        Value::Null => Ok(()),
        _ => Err(Error::Import(
            "geometry boundaries must be arrays or non-negative integers".to_owned(),
        )),
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
            let old_index = number.as_u64().ok_or_else(|| {
                Error::Import("vertex indices must be non-negative integers".to_owned())
            })?;
            let old_index = usize::try_from(old_index)
                .map_err(|_| Error::Import("vertex index does not fit in usize".to_owned()))?;
            let new_index = remap.get(&old_index).copied().ok_or_else(|| {
                Error::Import(format!("missing remap entry for vertex {old_index}"))
            })?;
            *value = Value::Number(serde_json::Number::from(
                u64::try_from(new_index)
                    .map_err(|_| Error::Import("vertex index does not fit in u64".to_owned()))?,
            ));
            Ok(())
        }
        Value::Null => Ok(()),
        _ => Err(Error::Import(
            "geometry boundaries must be arrays or non-negative integers".to_owned(),
        )),
    }
}

impl BBox {
    fn union(self, other: &BBox) -> BBox {
        BBox {
            min_x: self.min_x.min(other.min_x),
            max_x: self.max_x.max(other.max_x),
            min_y: self.min_y.min(other.min_y),
            max_y: self.max_y.max(other.max_y),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    #[test]
    fn single_tile_preparation_materializes_every_benchmark_layout() -> Result<()> {
        let root = temp_dir("benchmark-layouts");
        let artifact = root.join("basisvoorziening.city.json");
        fs::write(
            &artifact,
            serde_json::to_vec_pretty(&synthetic_cityjson_document(3))
                .map_err(|error| Error::Import(error.to_string()))?,
        )?;
        let cli = BenchmarkCli {
            json: false,
            corpus_root: root.clone(),
            work_root: root.join("work"),
            artifact: Some(artifact.clone()),
            case: Vec::new(),
            layout: Vec::new(),
            workers: vec![1],
            multi_tile_root: None,
        };

        for layout in BenchmarkLayoutKind::ALL {
            let prepared =
                prepare_case(&cli, BenchmarkCaseKind::SingleTileFull, layout, &artifact)?;
            assert_eq!(prepared.len(), 1);
            let manifest = &prepared[0].manifest;
            assert_eq!(manifest.layout, layout);
            assert_eq!(manifest.feature_count, 3);
            assert_eq!(manifest.source_count, 1);
            assert!(manifest.byte_size > 0);
            assert!(manifest.dataset_label.ends_with(layout.as_label()));

            let resolved = resolve_dataset(&manifest.prepared_dataset, None)?;
            assert_eq!(resolved.source_paths().len(), 1);

            let index_path = fresh_benchmark_index_path(manifest, 1)?;
            let resolved = resolve_dataset(&manifest.prepared_dataset, Some(index_path))?;
            let mut index = CityIndex::open(resolved.storage_layout(), &resolved.index_path)?;
            index.reindex()?;
            assert_eq!(index.package_count()?, manifest.feature_count);
            assert_eq!(index.cityobject_count()?, manifest.cityobject_count);
        }

        Ok(())
    }

    #[test]
    fn single_tile_cityjson_preparation_preserves_unmodified_artifact_bytes() -> Result<()> {
        // Input: a minified CityJSON artifact prepared as the full single-tile CityJSON layout.
        // Assertions: the prepared dataset file is byte-for-byte identical to the source artifact
        // and the manifest records that exact prepared size.
        let root = temp_dir("benchmark-cityjson-raw-bytes");
        let artifact = root.join("basisvoorziening.city.json");
        let artifact_bytes = serde_json::to_vec(&synthetic_cityjson_document(3))
            .map_err(|error| Error::Import(error.to_string()))?;
        fs::write(&artifact, &artifact_bytes)?;
        let cli = BenchmarkCli {
            json: false,
            corpus_root: root.clone(),
            work_root: root.join("work"),
            artifact: Some(artifact.clone()),
            case: Vec::new(),
            layout: vec![BenchmarkLayoutKind::CityJson],
            workers: vec![1],
            multi_tile_root: None,
        };

        let prepared = prepare_case(
            &cli,
            BenchmarkCaseKind::SingleTileFull,
            BenchmarkLayoutKind::CityJson,
            &artifact,
        )?;
        let manifest = &prepared[0].manifest;
        let prepared_bytes = fs::read(manifest.prepared_dataset.join("dataset.city.json"))?;

        assert_eq!(prepared_bytes, artifact_bytes);
        assert_eq!(
            manifest.byte_size,
            u64::try_from(artifact_bytes.len())
                .map_err(|_| Error::Import("test artifact size overflowed u64".to_owned()))?
        );

        Ok(())
    }

    #[test]
    fn subset_cityjson_preparation_writes_compact_valid_json() -> Result<()> {
        // Input: a pretty-printed CityJSON artifact prepared as a two-package CityJSON subset.
        // Assertions: the transformed prepared file is valid CityJSON JSON, contains the requested
        // package count, and is serialized compactly without pretty-print newlines.
        let root = temp_dir("benchmark-cityjson-compact-subset");
        let artifact = root.join("basisvoorziening.city.json");
        fs::write(
            &artifact,
            serde_json::to_vec_pretty(&synthetic_cityjson_document(4))
                .map_err(|error| Error::Import(error.to_string()))?,
        )?;
        let cli = BenchmarkCli {
            json: false,
            corpus_root: root.clone(),
            work_root: root.join("work"),
            artifact: Some(artifact.clone()),
            case: Vec::new(),
            layout: vec![BenchmarkLayoutKind::CityJson],
            workers: vec![1],
            multi_tile_root: None,
        };

        let prepared = prepare_single_tile_dataset(
            &cli,
            "single-tile-subset-2",
            BenchmarkLayoutKind::CityJson,
            &artifact,
            Some(2),
        )?;
        let prepared_bytes =
            fs::read(prepared.manifest.prepared_dataset.join("dataset.city.json"))?;
        let prepared_document: Value = serde_json::from_slice(&prepared_bytes)
            .map_err(|error| Error::Import(error.to_string()))?;

        assert_eq!(extract_root_ids(&prepared_document)?.len(), 2);
        assert!(
            !prepared_bytes.contains(&b'\n'),
            "transformed CityJSON benchmark fixtures should not be pretty-printed"
        );

        Ok(())
    }

    #[test]
    fn multi_source_preparation_creates_parallel_source_shards() -> Result<()> {
        let root = temp_dir("benchmark-multi-source");
        let artifact = root.join("basisvoorziening.city.json");
        fs::write(
            &artifact,
            serde_json::to_vec_pretty(&synthetic_cityjson_document(8))
                .map_err(|error| Error::Import(error.to_string()))?,
        )?;
        let cli = BenchmarkCli {
            json: false,
            corpus_root: root.clone(),
            work_root: root.join("work"),
            artifact: Some(artifact.clone()),
            case: Vec::new(),
            layout: vec![BenchmarkLayoutKind::CityJson],
            workers: vec![4],
            multi_tile_root: None,
        };

        let prepared = prepare_case(
            &cli,
            BenchmarkCaseKind::MultiSource,
            BenchmarkLayoutKind::CityJson,
            &artifact,
        )?;
        assert_eq!(prepared.len(), 1);
        let manifest = &prepared[0].manifest;
        assert!(
            manifest.source_count > 1,
            "multi-source preparation should create more than one source file"
        );

        let resolved = resolve_dataset(&manifest.prepared_dataset, None)?;
        assert!(
            resolved.source_paths().len() > 1,
            "resolved prepared dataset should expose multiple source shards"
        );
        for source_path in resolved.source_paths() {
            let shard_bytes = fs::read(source_path)?;
            let shard_document: Value = serde_json::from_slice(&shard_bytes)
                .map_err(|error| Error::Import(error.to_string()))?;
            assert!(
                !shard_bytes.contains(&b'\n'),
                "derived multi-source CityJSON shards should be compact JSON"
            );
            assert!(
                !extract_root_ids(&shard_document)?.is_empty(),
                "each benchmark shard should contain at least one package"
            );
        }
        assert!(
            resolved.source_paths().len().min(4) > 1,
            "a worker count greater than one should be able to reach multiple shards"
        );

        with_worker_count_env(4, || {
            let index_path = fresh_benchmark_index_path(manifest, 4)?;
            let resolved = resolve_dataset(&manifest.prepared_dataset, Some(index_path))?;
            let mut index = CityIndex::open(resolved.storage_layout(), &resolved.index_path)?;
            index.reindex()?;
            assert_eq!(
                index.source_count()?,
                manifest.source_count,
                "indexed source count should match prepared shards"
            );
            Ok(())
        })?;

        Ok(())
    }

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cityjson-index-{label}-{unique}.dir"));
        fs::create_dir_all(&path).expect("temp benchmark directory should be creatable");
        path
    }

    fn synthetic_cityjson_document(feature_count: usize) -> Value {
        let mut cityobjects = serde_json::Map::new();
        let mut vertices = Vec::with_capacity(feature_count * 3);
        for index in 0..feature_count {
            let base = index * 3;
            cityobjects.insert(
                format!("feature-{index:02}"),
                json!({
                    "type": "Building",
                    "geometry": [{
                        "type": "MultiSurface",
                        "lod": "1.0",
                        "boundaries": [[[base, base + 1, base + 2]]]
                    }]
                }),
            );
            let x = i64::try_from(index).expect("feature index should fit in i64") * 100;
            vertices.push(json!([x, 0, 0]));
            vertices.push(json!([x + 10, 0, 0]));
            vertices.push(json!([x, 10, 0]));
        }

        json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [1.0, 1.0, 1.0],
                "translate": [0.0, 0.0, 0.0]
            },
            "metadata": {
                "referenceSystem": "https://www.opengis.net/def/crs/EPSG/0/4979",
                "title": "benchmark test fixture"
            },
            "CityObjects": cityobjects,
            "vertices": vertices
        })
    }
}
