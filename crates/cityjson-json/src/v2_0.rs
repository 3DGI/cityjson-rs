use std::collections::HashSet;
use std::io::{Read, Write};

use cityjson_types::resources::storage::StringStorage;
use cityjson_types::v2_0::{BBox, CityModel, OwnedCityModel, Transform, VertexRef};
use cityjson_types::{CityJSONVersion, CityModelType};
use serde::Serialize;
use serde_json::de::{IoRead, StreamDeserializer};
use serde_json::{Map, Value};

use crate::errors::{Error, Result};

#[derive(Debug, Clone)]
pub struct ReadOptions {
    pub symbol_storage: cityjson_types::symbols::SymbolStorageOptions,
    pub validate_default_themes: bool,
    pub reject_duplicate_ids: bool,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            symbol_storage: cityjson_types::symbols::SymbolStorageOptions::default(),
            validate_default_themes: false,
            reject_duplicate_ids: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    pub pretty: bool,
    pub validate_default_themes: bool,
    pub trailing_newline: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CityJsonSeqWriteReport {
    pub transform: Transform,
    pub geographical_extent: Option<BBox>,
    pub feature_count: usize,
    pub cityobject_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeatureStreamTransform {
    Explicit(Transform),
    Auto { scale: [f64; 3] },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CityJsonSeqWriteOptions {
    pub transform: FeatureStreamTransform,
    pub validate_default_themes: bool,
    pub trailing_newline: bool,
    pub update_metadata_geographical_extent: bool,
}

impl Default for CityJsonSeqWriteOptions {
    fn default() -> Self {
        Self {
            transform: FeatureStreamTransform::Auto {
                scale: [0.001, 0.001, 0.001],
            },
            validate_default_themes: true,
            trailing_newline: true,
            update_metadata_geographical_extent: true,
        }
    }
}

pub struct CityJsonSeqReader<R>
where
    R: Read,
{
    stream: StreamDeserializer<'static, IoRead<R>, Value>,
    base_root: Map<String, Value>,
    version: CityJSONVersion,
    seen_ids: HashSet<String>,
    options: ReadOptions,
}

impl<R> CityJsonSeqReader<R>
where
    R: Read,
{
    fn new(reader: R, options: &ReadOptions) -> Result<Self> {
        let mut stream = serde_json::Deserializer::from_reader(reader).into_iter::<Value>();
        let first = stream
            .next()
            .transpose()?
            .ok_or(Error::MalformedRootObject("empty feature stream"))?;
        let aggregate_root = into_object(first)?;
        let version = ensure_document_root(&aggregate_root)?;
        let seen_ids = if options.reject_duplicate_ids {
            collect_cityobject_ids(&aggregate_root)?
        } else {
            HashSet::new()
        };

        Ok(Self {
            stream,
            base_root: build_feature_base_root(&aggregate_root),
            version,
            seen_ids,
            options: options.clone(),
        })
    }
}

impl<R> Iterator for CityJsonSeqReader<R>
where
    R: Read,
{
    type Item = Result<OwnedCityModel>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.stream.next()?;
        Some(match next {
            Ok(value) => (|| -> Result<OwnedCityModel> {
                let feature = into_object(value)?;
                ensure_feature_root(&feature, self.version)?;
                if self.options.reject_duplicate_ids {
                    extend_seen_ids(&mut self.seen_ids, &feature)?;
                }

                let bytes = serde_json::to_vec(&Value::Object(materialize_feature_document(
                    &self.base_root,
                    feature,
                )))?;
                read_feature(&bytes, &self.options)
            })(),
            Err(err) => Err(err.into()),
        })
    }
}

/// Parse a `CityJSON` document into an owned model.
///
/// # Errors
///
/// Returns an error if the payload is not valid UTF-8, not valid JSON, or does
/// not contain a `CityJSON` root document.
pub fn read_model(bytes: &[u8], options: &ReadOptions) -> Result<OwnedCityModel> {
    let model = read_owned_model(bytes, options)?;
    ensure_parsed_model_type(model, CityModelType::CityJSON)
}

/// Parse a `CityJSONFeature` document into an owned model.
///
/// # Errors
///
/// Returns an error if the payload is not valid UTF-8, not valid JSON, or does
/// not contain a `CityJSONFeature` root document.
pub fn read_feature(bytes: &[u8], options: &ReadOptions) -> Result<OwnedCityModel> {
    let model = read_owned_model(bytes, options)?;
    ensure_parsed_model_type(model, CityModelType::CityJSONFeature)
}

/// Parse a feature payload using the shared non-feature root state from `base`.
///
/// # Errors
///
/// Returns an error if `base` is not a `CityJSON` document, if `feature_bytes`
/// do not contain a valid `CityJSONFeature`, or if the merged document cannot
/// be parsed.
pub fn read_feature_with_base(
    feature_bytes: &[u8],
    base: &OwnedCityModel,
    options: &ReadOptions,
) -> Result<OwnedCityModel> {
    let (version, base_root) = feature_base_root_from_model(base)?;
    let feature = into_object(serde_json::from_slice(feature_bytes)?)?;
    ensure_feature_root(&feature, version)?;
    let bytes = serde_json::to_vec(&Value::Object(materialize_feature_document(
        &base_root, feature,
    )))?;
    read_feature(&bytes, options)
}

/// Read a strict `CityJSONSeq` stream as self-contained feature models.
///
/// # Errors
///
/// Returns an error if the stream header is invalid or if stream items violate
/// the `CityJSON`/`CityJSONFeature` ordering and duplicate-id rules.
pub fn read_feature_stream<R>(reader: R, options: &ReadOptions) -> Result<CityJsonSeqReader<R>>
where
    R: Read,
{
    CityJsonSeqReader::new(reader, options)
}

/// Write a single `CityJSON` or `CityJSONFeature` document.
///
/// # Errors
///
/// Returns an error if validation is enabled and fails, or if serialization to
/// `writer` fails.
pub fn write_model<W>(mut writer: W, model: &OwnedCityModel, options: &WriteOptions) -> Result<()>
where
    W: Write,
{
    if options.validate_default_themes {
        model.validate_default_themes()?;
    }

    let serializable = SerializableCityModelWithOptions {
        model,
        options: crate::ser::CityModelSerializeOptions::for_model(model),
    };
    if options.pretty {
        serde_json::to_writer_pretty(&mut writer, &serializable)?;
    } else {
        serde_json::to_writer(&mut writer, &serializable)?;
    }
    if options.trailing_newline {
        write_newline(&mut writer)?;
    }
    Ok(())
}

/// Serialize a single document to an owned byte vector.
///
/// # Errors
///
/// Returns an error if validation is enabled and fails, or if serialization
/// fails.
pub fn to_vec(model: &OwnedCityModel, options: &WriteOptions) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    write_model(&mut output, model, options)?;
    Ok(output)
}

/// Write a strict `CityJSONSeq` stream from feature models.
///
/// # Errors
///
/// Returns an error if the feature models do not share compatible root state,
/// if duplicate feature-local `CityObject` ids are present, or if stream
/// serialization fails.
pub fn write_feature_stream<W, I>(
    writer: W,
    models: I,
    options: &CityJsonSeqWriteOptions,
) -> Result<CityJsonSeqWriteReport>
where
    W: Write,
    I: IntoIterator<Item = OwnedCityModel>,
{
    write_feature_stream_from_header_source(
        writer,
        FeatureStreamHeaderSource::DeriveFromFeatures,
        models,
        options,
    )
}

/// Write a strict `CityJSONSeq` stream from feature models using an explicit
/// document root as the stream header.
///
/// The feature items themselves do not carry root-level state such as metadata,
/// extensions, appearance, or geometry templates. Use this when the caller has
/// already chosen the aggregate root state for the output stream.
///
/// # Errors
///
/// Returns an error if `base_root` is not a `CityJSON` document root, if any
/// feature model is not a `CityJSONFeature`, if duplicate feature-local
/// `CityObject` ids are present, or if stream serialization fails.
pub fn write_feature_stream_with_base<W, I>(
    writer: W,
    base_root: &OwnedCityModel,
    models: I,
    options: &CityJsonSeqWriteOptions,
) -> Result<CityJsonSeqWriteReport>
where
    W: Write,
    I: IntoIterator<Item = OwnedCityModel>,
{
    write_feature_stream_from_header_source(
        writer,
        FeatureStreamHeaderSource::ExplicitBase(base_root),
        models,
        options,
    )
}

#[derive(Clone, Copy)]
enum FeatureStreamHeaderSource<'a> {
    DeriveFromFeatures,
    ExplicitBase(&'a OwnedCityModel),
}

fn write_feature_stream_from_header_source<W, I>(
    mut writer: W,
    header_source: FeatureStreamHeaderSource<'_>,
    models: I,
    options: &CityJsonSeqWriteOptions,
) -> Result<CityJsonSeqWriteReport>
where
    W: Write,
    I: IntoIterator<Item = OwnedCityModel>,
{
    let features: Vec<_> = models.into_iter().collect();
    match header_source {
        FeatureStreamHeaderSource::DeriveFromFeatures => {
            validate_feature_models(&features, options.validate_default_themes)?;
        }
        FeatureStreamHeaderSource::ExplicitBase(base_root) => {
            ensure_stream_base_root(base_root)?;
            validate_feature_model_items(&features, options.validate_default_themes)?;
        }
    }
    let feature_refs: Vec<_> = features.iter().collect();
    let geographical_extent = collect_features_extent(&feature_refs);
    let transform = match &options.transform {
        FeatureStreamTransform::Explicit(transform) => transform.clone(),
        FeatureStreamTransform::Auto { scale } => auto_transform(&feature_refs, *scale),
    };
    let metadata_geographical_extent = if options.update_metadata_geographical_extent {
        geographical_extent.as_ref()
    } else {
        None
    };
    let header_model = match header_source {
        FeatureStreamHeaderSource::DeriveFromFeatures => feature_refs.first().copied(),
        FeatureStreamHeaderSource::ExplicitBase(base_root) => Some(base_root),
    };
    let header_root =
        build_feature_stream_header(header_model, &transform, metadata_geographical_extent)?;
    write_feature_stream_items(
        &mut writer,
        &feature_refs,
        &header_root,
        &transform,
        options,
    )?;

    Ok(CityJsonSeqWriteReport {
        transform,
        geographical_extent,
        feature_count: feature_refs.len(),
        cityobject_count: feature_refs
            .iter()
            .map(|feature| feature.cityobjects().len())
            .sum(),
    })
}

fn read_owned_model(bytes: &[u8], options: &ReadOptions) -> Result<OwnedCityModel> {
    let input = std::str::from_utf8(bytes)?;
    let model = crate::de::from_str_owned(input)?;
    if options.validate_default_themes {
        model.validate_default_themes()?;
    }
    Ok(model)
}

fn ensure_parsed_model_type(
    model: OwnedCityModel,
    expected: CityModelType,
) -> Result<OwnedCityModel> {
    if model.type_citymodel() == expected {
        Ok(model)
    } else {
        Err(Error::UnsupportedType(model.type_citymodel().to_string()))
    }
}

fn feature_base_root_from_model(
    base: &OwnedCityModel,
) -> Result<(CityJSONVersion, Map<String, Value>)> {
    let root = serialize_model_root(
        base,
        crate::ser::CityModelSerializeOptions {
            type_name: base.type_citymodel(),
            include_id: false,
            include_version: true,
            transform: base.transform(),
            include_transform: base.transform().is_some(),
            include_metadata: true,
            metadata_geographical_extent: None,
            include_extensions: true,
            include_vertices: false,
            include_appearance: true,
            include_geometry_templates: true,
            include_cityobjects: false,
            include_extra: true,
        },
    )?;
    let version = ensure_document_root(&root)?;
    Ok((version, build_feature_base_root(&root)))
}

fn serialize_model_root(
    model: &OwnedCityModel,
    options: crate::ser::CityModelSerializeOptions<'_>,
) -> Result<Map<String, Value>> {
    into_object(serde_json::to_value(&SerializableCityModelWithOptions {
        model,
        options,
    })?)
}

struct SerializableCityModelWithOptions<'a, VR, SS>
where
    VR: VertexRef + Serialize,
    SS: StringStorage,
{
    model: &'a CityModel<VR, SS>,
    options: crate::ser::CityModelSerializeOptions<'a>,
}

impl<VR, SS> Serialize for SerializableCityModelWithOptions<'_, VR, SS>
where
    VR: VertexRef + Serialize,
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        crate::ser::serialize_citymodel_with_options(serializer, self.model, &self.options)
    }
}

fn into_object(value: Value) -> Result<Map<String, Value>> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(Error::MalformedRootObject(
            "stream items must be JSON objects",
        )),
    }
}

fn ensure_document_root(root: &Map<String, Value>) -> Result<CityJSONVersion> {
    let kind = root_kind(root)?;
    if kind != CityModelType::CityJSON {
        return Err(Error::MalformedRootObject(
            "first non-empty stream item must be CityJSON",
        ));
    }

    let version = root
        .get("version")
        .and_then(Value::as_str)
        .ok_or(Error::MalformedRootObject("missing root version"))?;
    let version = CityJSONVersion::try_from(version)
        .map_err(|_| Error::UnsupportedVersion(version.to_owned()))?;
    if version != CityJSONVersion::V2_0 {
        return Err(Error::UnsupportedVersion(version.to_string()));
    }
    Ok(version)
}

fn ensure_feature_root(root: &Map<String, Value>, version: CityJSONVersion) -> Result<()> {
    let kind = root_kind(root)?;
    if kind != CityModelType::CityJSONFeature {
        return Err(Error::MalformedRootObject(
            "stream items after the first must be CityJSONFeature",
        ));
    }

    if let Some(found) = root.get("version").and_then(Value::as_str) {
        let found = CityJSONVersion::try_from(found)
            .map_err(|_| Error::UnsupportedVersion(found.to_owned()))?;
        if found != version {
            return Err(Error::InvalidValue(format!(
                "feature stream version mismatch: expected {version}, found {found}"
            )));
        }
    }

    Ok(())
}

fn build_feature_base_root(root: &Map<String, Value>) -> Map<String, Value> {
    root.iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "type" | "version" | "CityObjects" | "vertices"
            )
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn root_kind(root: &Map<String, Value>) -> Result<CityModelType> {
    let type_name = root
        .get("type")
        .and_then(Value::as_str)
        .ok_or(Error::MalformedRootObject("missing root type"))?;
    CityModelType::try_from(type_name).map_err(|_| Error::UnsupportedType(type_name.to_owned()))
}

fn collect_cityobject_ids(root: &Map<String, Value>) -> Result<HashSet<String>> {
    let Some(cityobjects) = root.get("CityObjects") else {
        return Ok(HashSet::new());
    };
    let cityobjects = cityobjects
        .as_object()
        .ok_or(Error::MalformedRootObject("CityObjects must be an object"))?;
    Ok(cityobjects.keys().cloned().collect())
}

fn extend_seen_ids(seen: &mut HashSet<String>, root: &Map<String, Value>) -> Result<()> {
    for id in collect_cityobject_ids(root)? {
        if !seen.insert(id.clone()) {
            return Err(Error::InvalidValue(format!(
                "duplicate CityObject id in feature stream: {id}"
            )));
        }
    }
    Ok(())
}

fn materialize_feature_document(
    base_root: &Map<String, Value>,
    feature: Map<String, Value>,
) -> Map<String, Value> {
    let mut document = base_root.clone();
    document.insert(
        "type".to_owned(),
        Value::String(CityModelType::CityJSONFeature.to_string()),
    );
    for (key, value) in feature {
        if key != "version" {
            document.insert(key, value);
        }
    }
    document
}

fn validate_feature_models(
    features: &[OwnedCityModel],
    validate_default_themes: bool,
) -> Result<()> {
    let Some(first) = features.first() else {
        return Ok(());
    };

    let base_signature = shared_root_signature(first)?;
    validate_feature_model_items(features, validate_default_themes)?;
    for feature in features {
        if shared_root_signature(feature)? != base_signature {
            return Err(Error::InvalidValue(
                "feature stream carries incompatible root state".to_owned(),
            ));
        }
    }

    Ok(())
}

fn validate_feature_model_items(
    features: &[OwnedCityModel],
    validate_default_themes: bool,
) -> Result<()> {
    let mut seen_ids = HashSet::new();
    for feature in features {
        ensure_stream_feature_root(feature)?;
        if validate_default_themes {
            feature.validate_default_themes()?;
        }

        for (_, cityobject) in feature.cityobjects().iter() {
            let id = cityobject.id().to_owned();
            if !seen_ids.insert(id.clone()) {
                return Err(Error::InvalidValue(format!(
                    "duplicate CityObject id in feature stream: {id}"
                )));
            }
        }
    }

    Ok(())
}

fn auto_transform(features: &[&OwnedCityModel], scale: [f64; 3]) -> Transform {
    let extent = collect_features_extent(features);
    let mut transform = Transform::new();
    transform.set_scale(scale);
    transform.set_translate(extent.as_ref().map_or([0.0, 0.0, 0.0], |bbox| {
        [bbox.min_x(), bbox.min_y(), bbox.min_z()]
    }));
    transform
}

fn build_feature_stream_header(
    header_model: Option<&OwnedCityModel>,
    transform: &Transform,
    geographical_extent: Option<&BBox>,
) -> Result<Map<String, Value>> {
    let mut root = if let Some(model) = header_model {
        serialize_feature_stream_header_model(model, transform, geographical_extent)?
    } else {
        let mut root = Map::new();
        root.insert(
            "type".to_owned(),
            Value::String(CityModelType::CityJSON.to_string()),
        );
        root.insert(
            "version".to_owned(),
            Value::String(CityJSONVersion::V2_0.to_string()),
        );
        root.insert("transform".to_owned(), serialize_transform(transform)?);
        root
    };
    root.remove("id");
    root.insert("CityObjects".to_owned(), Value::Object(Map::new()));
    root.insert("vertices".to_owned(), Value::Array(Vec::new()));
    Ok(root)
}

fn serialize_feature_stream_header_model(
    model: &OwnedCityModel,
    transform: &Transform,
    geographical_extent: Option<&BBox>,
) -> Result<Map<String, Value>> {
    serialize_model_root(
        model,
        crate::ser::CityModelSerializeOptions {
            type_name: CityModelType::CityJSON,
            include_id: false,
            include_version: true,
            transform: Some(transform),
            include_transform: true,
            include_metadata: true,
            metadata_geographical_extent: geographical_extent,
            include_extensions: true,
            include_vertices: false,
            include_appearance: true,
            include_geometry_templates: true,
            include_cityobjects: false,
            include_extra: true,
        },
    )
}

fn write_feature_stream_items<W>(
    mut writer: W,
    features: &[&OwnedCityModel],
    header_root: &Map<String, Value>,
    transform: &Transform,
    options: &CityJsonSeqWriteOptions,
) -> Result<()>
where
    W: Write,
{
    serde_json::to_writer(&mut writer, header_root)?;
    if !features.is_empty() || options.trailing_newline {
        write_newline(&mut writer)?;
    }

    for (index, feature) in features.iter().enumerate() {
        let item = SerializableCityModelWithOptions {
            model: *feature,
            options: crate::ser::CityModelSerializeOptions {
                type_name: CityModelType::CityJSONFeature,
                include_id: true,
                include_version: false,
                transform: Some(transform),
                include_transform: false,
                include_metadata: false,
                metadata_geographical_extent: None,
                include_extensions: false,
                include_vertices: true,
                include_appearance: false,
                include_geometry_templates: false,
                include_cityobjects: true,
                include_extra: false,
            },
        };
        serde_json::to_writer(&mut writer, &item)?;
        if index + 1 < features.len() || options.trailing_newline {
            write_newline(&mut writer)?;
        }
    }

    Ok(())
}

fn serialize_transform(transform: &Transform) -> Result<Value> {
    #[derive(Serialize)]
    struct TransformValue {
        scale: [f64; 3],
        translate: [f64; 3],
    }

    Ok(serde_json::to_value(TransformValue {
        scale: transform.scale(),
        translate: transform.translate(),
    })?)
}

fn ensure_stream_feature_root<VR, SS>(feature: &CityModel<VR, SS>) -> Result<()>
where
    VR: VertexRef + Serialize,
    SS: StringStorage,
{
    if feature.type_citymodel() != CityModelType::CityJSONFeature {
        return Err(Error::UnsupportedType(feature.type_citymodel().to_string()));
    }
    if feature.id().is_none() {
        return Err(Error::InvalidValue(
            "CityJSONFeature root id is required".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_stream_base_root(base_root: &OwnedCityModel) -> Result<()> {
    if base_root.type_citymodel() != CityModelType::CityJSON {
        return Err(Error::UnsupportedType(
            base_root.type_citymodel().to_string(),
        ));
    }
    Ok(())
}

fn shared_root_signature<VR, SS>(model: &CityModel<VR, SS>) -> Result<Map<String, Value>>
where
    VR: VertexRef + Serialize,
    SS: StringStorage,
{
    let value = serde_json::to_value(&SerializableCityModelWithOptions {
        model,
        options: crate::ser::CityModelSerializeOptions {
            type_name: model.type_citymodel(),
            include_id: false,
            include_version: true,
            transform: model.transform(),
            include_transform: model.transform().is_some(),
            include_metadata: true,
            metadata_geographical_extent: None,
            include_extensions: true,
            include_vertices: false,
            include_appearance: true,
            include_geometry_templates: true,
            include_cityobjects: false,
            include_extra: true,
        },
    })?;
    let mut root = into_object(value)?;
    root.remove("type");
    root.remove("version");
    root.remove("transform");
    if let Some(metadata) = root.get_mut("metadata").and_then(Value::as_object_mut) {
        metadata.remove("geographicalExtent");
        if metadata.is_empty() {
            root.remove("metadata");
        }
    }
    Ok(root)
}

fn collect_features_extent<VR, SS>(features: &[&CityModel<VR, SS>]) -> Option<BBox>
where
    VR: VertexRef + Serialize,
    SS: StringStorage,
{
    let mut extent = ExtentAccumulator::default();
    for feature in features {
        for vertex in feature.vertices().as_slice() {
            extent.include([vertex.x(), vertex.y(), vertex.z()]);
        }
        for (_, cityobject) in feature.cityobjects().iter() {
            if let Some(bbox) = cityobject.geographical_extent() {
                extent.include_bbox(*bbox);
            }
        }
    }
    extent.finish()
}

fn write_newline<W>(writer: &mut W) -> Result<()>
where
    W: Write,
{
    writer
        .write_all(b"\n")
        .map_err(|err| Error::Json(serde_json::Error::io(err)))
}

#[derive(Default)]
struct ExtentAccumulator {
    min: Option<[f64; 3]>,
    max: Option<[f64; 3]>,
}

impl ExtentAccumulator {
    fn include(&mut self, coordinate: [f64; 3]) {
        match (&mut self.min, &mut self.max) {
            (Some(min), Some(max)) => {
                for axis in 0..3 {
                    min[axis] = min[axis].min(coordinate[axis]);
                    max[axis] = max[axis].max(coordinate[axis]);
                }
            }
            (None, None) => {
                self.min = Some(coordinate);
                self.max = Some(coordinate);
            }
            _ => unreachable!("extent accumulator stores min and max together"),
        }
    }

    fn include_bbox(&mut self, bbox: BBox) {
        self.include([bbox.min_x(), bbox.min_y(), bbox.min_z()]);
        self.include([bbox.max_x(), bbox.max_y(), bbox.max_z()]);
    }

    fn finish(self) -> Option<BBox> {
        match (self.min, self.max) {
            (Some(min), Some(max)) => {
                Some(BBox::new(min[0], min[1], min[2], max[0], max[1], max[2]))
            }
            (None, None) => None,
            _ => unreachable!("extent accumulator stores min and max together"),
        }
    }
}
