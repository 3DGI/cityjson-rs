use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::io::Cursor;

use cityjson_types::CityModelType;
use cityjson_types::v2_0::OwnedCityModel;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::errors::{Error, Result};
use crate::v2_0::{
    ReadOptions, WriteOptions, read_feature, read_feature_stream, read_model, to_vec,
};

pub mod staged {
    use std::borrow::Cow;
    use std::collections::{BTreeMap, HashMap};
    use std::io::Write;
    use std::path::Path;

    use cityjson_types::CityModelType;
    use cityjson_types::prelude::OwnedStringStorage;
    use cityjson_types::v2_0::OwnedCityModel;
    use serde_json::Value;
    use serde_json::value::RawValue;

    use crate::de::attributes::RawAttribute;
    use crate::de::build::build_model;
    use crate::de::root::{PreparedRoot, parse_root};
    use crate::errors::{Error, Result};
    use crate::v2_0::{WriteOptions, write_model};

    #[derive(Debug, Clone, Copy)]
    pub struct FeatureObjectFragment<'a> {
        pub id: &'a str,
        pub object: &'a RawValue,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct FeatureAssembly<'a> {
        pub id: &'a str,
        pub cityobjects: &'a [FeatureObjectFragment<'a>],
        pub vertices: &'a [[i64; 3]],
    }

    /// # Errors
    ///
    /// Returns an error if JSON parsing fails or the input is not a valid `CityJSONFeature`.
    pub fn from_feature_slice_with_base(
        feature_bytes: &[u8],
        base_document_bytes: &[u8],
    ) -> Result<OwnedCityModel> {
        from_feature_slice_with_base_direct(feature_bytes, base_document_bytes)
    }

    /// # Errors
    ///
    /// Returns an error if JSON parsing fails or the input is not a valid `CityJSONFeature`.
    pub fn from_feature_slice_with_base_assume_cityjson_feature_v2_0(
        feature_bytes: &[u8],
        base_document_bytes: &[u8],
    ) -> Result<OwnedCityModel> {
        from_feature_slice_with_base_direct(feature_bytes, base_document_bytes)
    }

    /// # Errors
    ///
    /// Returns an error if JSON parsing fails or the input is not a valid `CityJSONFeature`.
    pub fn from_feature_slice_with_base_direct(
        feature_bytes: &[u8],
        base_document_bytes: &[u8],
    ) -> Result<OwnedCityModel> {
        build_feature_slice_with_base_direct(feature_bytes, None, base_document_bytes, false)
    }

    /// # Errors
    ///
    /// Returns an error if JSON serialization or parsing fails, or the assembly is not valid.
    pub fn from_feature_assembly_with_base(
        assembly: FeatureAssembly<'_>,
        base_document_bytes: &[u8],
    ) -> Result<OwnedCityModel> {
        from_feature_assembly_with_base_direct(assembly, base_document_bytes)
    }

    /// # Errors
    ///
    /// Returns an error if JSON parsing fails or the input is not a valid `CityJSONFeature`.
    pub fn from_feature_slice_with_indexed_id_and_base(
        feature_bytes: &[u8],
        indexed_id: &str,
        base_document_bytes: &[u8],
    ) -> Result<OwnedCityModel> {
        build_feature_slice_with_base_direct(
            feature_bytes,
            Some(indexed_id),
            base_document_bytes,
            true,
        )
    }

    fn build_feature_slice_with_base_direct(
        feature_bytes: &[u8],
        indexed_id: Option<&str>,
        base_document_bytes: &[u8],
        insert_missing_root: bool,
    ) -> Result<OwnedCityModel> {
        let base_input = std::str::from_utf8(base_document_bytes)?;
        let feature_input = std::str::from_utf8(feature_bytes)?;
        let base = parse_base_root(base_input)?;
        let mut feature = parse_feature_root(feature_input)?;
        let root_id = match indexed_id {
            Some(id) => Cow::Borrowed(id),
            None => feature_root_id(feature.id.take())?,
        };
        let adjusted_cityobjects = if insert_missing_root {
            cityobjects_raw_with_feature_root(feature.cityobjects, root_id.as_ref())?
        } else {
            None
        };
        let cityobjects = adjusted_cityobjects
            .as_deref()
            .unwrap_or(feature.cityobjects);
        let root = merge_base_and_feature_roots(base, feature, root_id, cityobjects);
        build_model::<OwnedStringStorage>(root)
    }

    /// # Errors
    ///
    /// Returns an error if JSON parsing fails or the assembled feature is not valid.
    // CityJSON vertices are serialized as JSON numbers, so this widening is intentional here.
    #[allow(clippy::cast_precision_loss)]
    pub fn from_feature_assembly_with_base_direct(
        assembly: FeatureAssembly<'_>,
        base_document_bytes: &[u8],
    ) -> Result<OwnedCityModel> {
        let base_input = std::str::from_utf8(base_document_bytes)?;
        let base = parse_base_root(base_input)?;
        let cityobjects = cityobjects_raw_from_fragments(assembly.cityobjects)?;
        let vertices = assembly
            .vertices
            .iter()
            .map(|vertex| [vertex[0] as f64, vertex[1] as f64, vertex[2] as f64])
            .collect();
        let root = PreparedRoot {
            type_name: "CityJSONFeature",
            version: base.version,
            transform: base.transform,
            vertices,
            metadata: base.metadata,
            extensions: base.extensions,
            cityobjects: cityobjects.as_ref(),
            appearance: base.appearance,
            geometry_templates: base.geometry_templates,
            id: Some(RawAttribute::String(Cow::Borrowed(assembly.id))),
            extra: base.extra,
        };
        build_model::<OwnedStringStorage>(root)
    }

    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the content is not a valid `CityJSONFeature`.
    pub fn from_feature_file_with_base<P: AsRef<Path>>(
        path: P,
        base_document_bytes: &[u8],
    ) -> Result<OwnedCityModel> {
        let bytes =
            std::fs::read(path).map_err(|error| Error::Json(serde_json::Error::io(error)))?;
        from_feature_slice_with_base(&bytes, base_document_bytes)
    }

    /// # Errors
    ///
    /// Returns an error if the model is not a `CityJSONFeature` or if serialization fails.
    pub fn to_feature_writer(writer: &mut impl Write, model: &OwnedCityModel) -> Result<()> {
        match model.type_citymodel() {
            CityModelType::CityJSONFeature => write_model(writer, model, &WriteOptions::default()),
            other => Err(Error::UnsupportedType(other.to_string())),
        }
    }

    fn parse_base_root(input: &str) -> Result<PreparedRoot<'_>> {
        let root = parse_root(input)?;
        if root.type_name != "CityJSON" {
            return Err(Error::MalformedRootObject(
                "base document must be a CityJSON root",
            ));
        }
        Ok(root)
    }

    fn parse_feature_root(input: &str) -> Result<PreparedRoot<'_>> {
        let root = parse_root(input)?;
        if root.type_name != "CityJSONFeature" {
            return Err(Error::MalformedRootObject(
                "feature document must be a CityJSONFeature root",
            ));
        }
        Ok(root)
    }

    fn merge_base_and_feature_roots<'a>(
        base: PreparedRoot<'a>,
        feature: PreparedRoot<'a>,
        root_id: Cow<'a, str>,
        cityobjects: &'a RawValue,
    ) -> PreparedRoot<'a> {
        let mut extra: HashMap<&'a str, RawAttribute<'a>> = base.extra;
        extra.extend(feature.extra);
        PreparedRoot {
            type_name: "CityJSONFeature",
            version: base.version,
            transform: feature.transform.or(base.transform),
            vertices: feature.vertices,
            metadata: feature.metadata.or(base.metadata),
            extensions: feature.extensions.or(base.extensions),
            cityobjects,
            appearance: feature.appearance.or(base.appearance),
            geometry_templates: feature.geometry_templates.or(base.geometry_templates),
            id: Some(RawAttribute::String(root_id)),
            extra,
        }
    }

    fn feature_root_id(id: Option<RawAttribute<'_>>) -> Result<Cow<'_, str>> {
        match id {
            Some(RawAttribute::String(value)) => Ok(value),
            Some(_) => Err(Error::InvalidValue(
                "CityJSONFeature root id must be a string".to_owned(),
            )),
            None => Err(Error::InvalidValue(
                "CityJSONFeature root id is required".to_owned(),
            )),
        }
    }

    fn cityobjects_raw_with_feature_root(
        cityobjects: &RawValue,
        feature_id: &str,
    ) -> Result<Option<Box<RawValue>>> {
        let entries: BTreeMap<&str, &RawValue> = serde_json::from_str(cityobjects.get())?;
        if entries.contains_key(feature_id) {
            return Ok(None);
        }

        let wrapper_type = entries
            .values()
            .find_map(|object| cityobject_type_name(object).transpose())
            .transpose()?
            .unwrap_or_else(|| "Building".to_owned());
        let children = entries.keys().copied().collect::<Vec<_>>();

        let mut output = String::with_capacity(cityobjects.get().len() + feature_id.len() + 64);
        output.push('{');
        output.push_str(&serde_json::to_string(feature_id)?);
        output.push_str(r#":{"type":"#);
        output.push_str(&serde_json::to_string(&wrapper_type)?);
        output.push_str(r#","children":"#);
        output.push_str(&serde_json::to_string(&children)?);
        output.push('}');
        for (id, object) in entries {
            output.push(',');
            output.push_str(&serde_json::to_string(id)?);
            output.push(':');
            output.push_str(object.get());
        }
        output.push('}');

        RawValue::from_string(output).map(Some).map_err(Error::from)
    }

    fn cityobject_type_name(object: &RawValue) -> Result<Option<String>> {
        let value: Value = serde_json::from_str(object.get())?;
        Ok(value
            .get("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned))
    }

    fn cityobjects_raw_from_fragments(
        cityobjects: &[FeatureObjectFragment<'_>],
    ) -> Result<Box<RawValue>> {
        let capacity = cityobjects.iter().fold(2, |capacity, cityobject| {
            capacity + cityobject.id.len() + cityobject.object.get().len() + 4
        });
        let mut output = String::with_capacity(capacity);
        output.push('{');
        for (index, cityobject) in cityobjects.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str(&serde_json::to_string(cityobject.id)?);
            output.push(':');
            output.push_str(cityobject.object.get());
        }
        output.push('}');
        RawValue::from_string(output).map_err(Error::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    CityJSON,
    CityJSONFeature,
}

impl RootKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::CityJSON => "CityJSON",
            Self::CityJSONFeature => "CityJSONFeature",
        }
    }
}

impl Display for RootKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    kind: RootKind,
    version: Option<String>,
}

impl Probe {
    #[must_use]
    pub fn kind(&self) -> RootKind {
        self.kind
    }

    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

#[derive(Debug, Deserialize)]
struct Header {
    #[serde(rename = "type")]
    kind: String,
    version: Option<String>,
}

/// # Errors
///
/// Returns an error if JSON parsing fails or the root type is not recognized.
pub fn probe(bytes: &[u8]) -> Result<Probe> {
    let header: Header = serde_json::from_slice(bytes)?;
    let kind = match header.kind.as_str() {
        "CityJSON" => RootKind::CityJSON,
        "CityJSONFeature" => RootKind::CityJSONFeature,
        other => return Err(Error::UnsupportedType(other.to_owned())),
    };

    Ok(Probe {
        kind,
        version: header.version,
    })
}

fn import_error(message: impl Into<String>) -> Error {
    Error::InvalidValue(message.into())
}

fn serialize_root(model: &OwnedCityModel) -> Result<Map<String, Value>> {
    match serde_json::from_slice(&to_vec(model, &WriteOptions::default())?)? {
        Value::Object(root) => Ok(root),
        _ => Err(import_error("serialized CityJSON root is not an object")),
    }
}

fn parse_root(root: Map<String, Value>) -> Result<OwnedCityModel> {
    let bytes = serde_json::to_vec(&Value::Object(root))?;
    match probe(&bytes)?.kind() {
        RootKind::CityJSON => read_model(&bytes, &ReadOptions::default()),
        RootKind::CityJSONFeature => read_feature(&bytes, &ReadOptions::default()),
    }
}

fn root_kind(root: &Map<String, Value>) -> Result<&str> {
    root.get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| import_error("CityJSON root is missing its type"))
}

fn get_object<'a>(root: &'a Map<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    root.get(key).and_then(Value::as_object)
}

fn get_object_mut<'a>(
    root: &'a mut Map<String, Value>,
    key: &str,
) -> Option<&'a mut Map<String, Value>> {
    root.get_mut(key).and_then(Value::as_object_mut)
}

fn get_array<'a>(root: &'a Map<String, Value>, key: &str) -> Option<&'a Vec<Value>> {
    root.get(key).and_then(Value::as_array)
}

fn get_array_mut<'a>(root: &'a mut Map<String, Value>, key: &str) -> Option<&'a mut Vec<Value>> {
    root.get_mut(key).and_then(Value::as_array_mut)
}

#[derive(Debug, Clone, PartialEq)]
enum TransformMergeState {
    Empty,
    Present(Value),
    Cleared,
}

impl TransformMergeState {
    fn from_root(root: &Map<String, Value>) -> Self {
        match root.get("transform") {
            Some(transform) => Self::Present(transform.clone()),
            None => Self::Empty,
        }
    }
}

fn reconcile_transform_state(
    current: TransformMergeState,
    source: Option<&Value>,
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

fn apply_transform_state(root: &mut Map<String, Value>, state: &TransformMergeState) {
    match state {
        TransformMergeState::Empty | TransformMergeState::Cleared => {
            root.remove("transform");
        }
        TransformMergeState::Present(transform) => {
            root.insert("transform".to_string(), transform.clone());
        }
    }
}

fn number_value(value: &Value, context: &str) -> Result<f64> {
    value
        .as_f64()
        .ok_or_else(|| import_error(format!("expected numeric {context}")))
}

fn parse_transform(root: &Map<String, Value>) -> Result<Option<([f64; 3], [f64; 3])>> {
    let Some(transform) = root.get("transform") else {
        return Ok(None);
    };
    let transform = transform
        .as_object()
        .ok_or_else(|| import_error("transform is not an object"))?;
    let scale = transform
        .get("scale")
        .and_then(Value::as_array)
        .ok_or_else(|| import_error("transform.scale is missing or not an array"))?;
    let translate = transform
        .get("translate")
        .and_then(Value::as_array)
        .ok_or_else(|| import_error("transform.translate is missing or not an array"))?;
    if scale.len() != 3 || translate.len() != 3 {
        return Err(import_error(
            "transform.scale and transform.translate must have three values",
        ));
    }

    Ok(Some((
        [
            number_value(&scale[0], "transform scale")?,
            number_value(&scale[1], "transform scale")?,
            number_value(&scale[2], "transform scale")?,
        ],
        [
            number_value(&translate[0], "transform translate")?,
            number_value(&translate[1], "transform translate")?,
            number_value(&translate[2], "transform translate")?,
        ],
    )))
}

fn normalize_root_vertices_to_world(root: &mut Map<String, Value>) -> Result<()> {
    let Some((scale, translate)) = parse_transform(root)? else {
        return Ok(());
    };

    let vertices = get_array_mut(root, "vertices")
        .ok_or_else(|| import_error("model with transform is missing its vertices array"))?;
    for vertex in vertices {
        let vertex = vertex
            .as_array_mut()
            .ok_or_else(|| import_error("vertex entry is not an array"))?;
        if vertex.len() != 3 {
            return Err(import_error("vertex entry must have three coordinates"));
        }
        for index in 0..3 {
            let coordinate = number_value(&vertex[index], "vertex coordinate")?;
            vertex[index] = Value::from(coordinate * scale[index] + translate[index]);
        }
    }

    root.remove("transform");
    Ok(())
}

fn append_kind_compatible(target_kind: &str, source_kind: &str) -> bool {
    target_kind == source_kind || (target_kind == "CityJSON" && source_kind == "CityJSONFeature")
}

fn merge_root_object_field(
    target: &mut Map<String, Value>,
    source: &Map<String, Value>,
    key: &str,
) -> Result<()> {
    let Some(source_map) = get_object(source, key) else {
        return Ok(());
    };

    let target_value = target
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let target_map = target_value
        .as_object_mut()
        .ok_or_else(|| import_error(format!("target '{key}' field is not an object")))?;

    for (entry_key, entry_value) in source_map {
        match target_map.get(entry_key) {
            Some(existing) if existing != entry_value => {
                return Err(import_error(format!(
                    "conflicting '{key}' entry for '{entry_key}' during append"
                )));
            }
            Some(_) => {}
            None => {
                target_map.insert(entry_key.clone(), entry_value.clone());
            }
        }
    }

    Ok(())
}

fn remap_index_value(value: &mut Value, offset: u64) -> Result<()> {
    match value {
        Value::Number(number) => {
            let index = number
                .as_u64()
                .ok_or_else(|| import_error("expected non-negative integer index"))?;
            *value = Value::from(index + offset);
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                remap_index_value(item, offset)?;
            }
            Ok(())
        }
        Value::Null => Ok(()),
        _ => Err(import_error("expected an index array")),
    }
}

fn remap_geometry_boundaries(geometry: &mut Map<String, Value>, vertex_offset: u64) -> Result<()> {
    if let Some(boundaries) = geometry.get_mut("boundaries") {
        remap_index_value(boundaries, vertex_offset)?;
    }

    Ok(())
}

fn prune_relations(cityobject: &mut Map<String, Value>, selected: &BTreeSet<String>, key: &str) {
    let Some(values) = cityobject.get_mut(key).and_then(Value::as_array_mut) else {
        return;
    };

    values.retain(|value| value.as_str().is_some_and(|id| selected.contains(id)));
    if values.is_empty() {
        cityobject.remove(key);
    }
}

/// # Errors
///
/// Returns an error if serialization or re-parsing of the model fails, or the type is unsupported.
pub fn cleanup(model: &OwnedCityModel) -> Result<OwnedCityModel> {
    let options = WriteOptions {
        validate_default_themes: matches!(model.type_citymodel(), CityModelType::CityJSONFeature),
        ..WriteOptions::default()
    };
    let bytes = to_vec(model, &options)?;

    match model.type_citymodel() {
        CityModelType::CityJSON => read_model(&bytes, &ReadOptions::default()),
        CityModelType::CityJSONFeature => read_feature(&bytes, &ReadOptions::default()),
        other => Err(Error::UnsupportedType(other.to_string())),
    }
}

/// # Errors
///
/// Returns an error if serialization fails, the id set is empty, or no `CityObjects` match.
pub fn extract<'a, I>(model: &OwnedCityModel, cityobject_ids: I) -> Result<OwnedCityModel>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut root = serialize_root(model)?;
    let selected = cityobject_ids
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    if selected.is_empty() {
        return Err(import_error(
            "extract requires at least one CityObject identifier",
        ));
    }

    let cityobjects = get_object_mut(&mut root, "CityObjects")
        .ok_or_else(|| import_error("CityJSON root is missing its CityObjects map"))?;
    cityobjects.retain(|id, _| selected.contains(id));

    if cityobjects.is_empty() {
        return Err(import_error("extract selection matched no CityObjects"));
    }

    for cityobject in cityobjects.values_mut() {
        let Some(cityobject) = cityobject.as_object_mut() else {
            return Err(import_error("CityObject entry is not an object"));
        };

        prune_relations(cityobject, &selected, "children");
        prune_relations(cityobject, &selected, "parents");
    }

    parse_root(root)
}

fn merge_one(
    target_root: &mut Map<String, Value>,
    source_root: &Map<String, Value>,
    transform_state: &mut TransformMergeState,
) -> Result<()> {
    if !append_kind_compatible(root_kind(target_root)?, root_kind(source_root)?) {
        return Err(import_error(
            "model append currently requires both inputs to have the same root type",
        ));
    }

    let mut source_root = source_root.clone();
    if matches!(transform_state, TransformMergeState::Cleared)
        || target_root.get("transform") != source_root.get("transform")
    {
        normalize_root_vertices_to_world(target_root)?;
        normalize_root_vertices_to_world(&mut source_root)?;
        *transform_state = TransformMergeState::Cleared;
    } else {
        *transform_state =
            reconcile_transform_state(transform_state.clone(), source_root.get("transform"));
    }

    let vertex_offset = get_array(target_root, "vertices").map_or(0_u64, |vertices| {
        u64::try_from(vertices.len()).unwrap_or(u64::MAX)
    });

    let source_vertices = get_array(&source_root, "vertices")
        .cloned()
        .ok_or_else(|| import_error("source model is missing its vertices array"))?;
    let target_vertices = get_array_mut(target_root, "vertices")
        .ok_or_else(|| import_error("target model is missing its vertices array"))?;
    target_vertices.extend(source_vertices);

    merge_root_object_field(target_root, &source_root, "extensions")?;

    let source_cityobjects = get_object(&source_root, "CityObjects")
        .ok_or_else(|| import_error("source model is missing its CityObjects map"))?;
    let target_cityobjects = get_object_mut(target_root, "CityObjects")
        .ok_or_else(|| import_error("target model is missing its CityObjects map"))?;

    for (id, cityobject_value) in source_cityobjects {
        if target_cityobjects.contains_key(id) {
            return Err(import_error(format!(
                "duplicate CityObject id during append: {id}"
            )));
        }

        let mut cityobject = cityobject_value
            .as_object()
            .ok_or_else(|| import_error("source CityObject entry is not an object"))?
            .clone();

        if let Some(geometries) = cityobject.get_mut("geometry").and_then(Value::as_array_mut) {
            for geometry in geometries {
                let geometry = geometry
                    .as_object_mut()
                    .ok_or_else(|| import_error("geometry entry is not an object"))?;
                remap_geometry_boundaries(geometry, vertex_offset)?;
            }
        }

        target_cityobjects.insert(id.clone(), Value::Object(cityobject));
    }

    Ok(())
}

/// # Errors
///
/// Returns an error if JSON serialization or parsing fails, the root types are incompatible,
/// ids conflict, or the append cannot be applied.
pub fn append(target: &mut OwnedCityModel, source: &OwnedCityModel) -> Result<()> {
    let mut target_root = serialize_root(target)?;
    let source_root = serialize_root(source)?;
    let mut transform_state = TransformMergeState::from_root(&target_root);

    merge_one(&mut target_root, &source_root, &mut transform_state)?;
    apply_transform_state(&mut target_root, &transform_state);

    *target = parse_root(target_root)?;
    Ok(())
}

/// # Errors
///
/// Returns an error if the iterator is empty or if any merge step fails.
pub fn merge<I>(models: I) -> Result<OwnedCityModel>
where
    I: IntoIterator<Item = OwnedCityModel>,
{
    let mut models = models.into_iter();
    let Some(first) = models.next() else {
        return Err(import_error("merge requires at least one model"));
    };

    let mut merged_root = serialize_root(&first)?;
    let mut transform_state = TransformMergeState::from_root(&merged_root);

    for model in models {
        let source_root = serialize_root(&model)?;
        merge_one(&mut merged_root, &source_root, &mut transform_state)?;
    }

    apply_transform_state(&mut merged_root, &transform_state);
    parse_root(merged_root)
}

/// # Errors
///
/// Returns an error if the stream is empty, items are not JSON objects, or merging fails.
pub fn merge_feature_stream_slice(bytes: &[u8]) -> Result<OwnedCityModel> {
    let mut stream = serde_json::Deserializer::from_slice(bytes).into_iter::<Value>();
    let Some(first) = stream.next().transpose()? else {
        return Err(import_error("empty feature stream"));
    };
    let Value::Object(first) = first else {
        return Err(import_error("stream items must be JSON objects"));
    };
    let first_bytes = serde_json::to_vec(&Value::Object(first.clone()))?;

    if matches!(probe(&first_bytes)?.kind(), RootKind::CityJSON) {
        let reader = Cursor::new(bytes);
        let mut merged = read_model(&first_bytes, &ReadOptions::default())?;
        for feature in read_feature_stream(reader, &ReadOptions::default())? {
            append(&mut merged, &feature?)?;
        }
        return Ok(merged);
    }

    let mut models = vec![read_feature(&first_bytes, &ReadOptions::default())?];
    for item in stream {
        let Value::Object(item) = item? else {
            return Err(import_error("stream items must be JSON objects"));
        };
        let item_bytes = serde_json::to_vec(&Value::Object(item))?;
        models.push(read_feature(&item_bytes, &ReadOptions::default())?);
    }
    merge(models)
}

/// # Errors
///
/// Returns an error if the input is not a valid `CityJSONSeq` stream or merging fails.
pub fn merge_cityjsonseq_slice(bytes: &[u8]) -> Result<OwnedCityModel> {
    merge_feature_stream_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transformed_feature(id: &str, translate_x: f64) -> OwnedCityModel {
        let bytes = format!(
            r#"{{
                "type":"CityJSONFeature",
                "id":"{id}",
                "transform":{{"scale":[1.0,1.0,1.0],"translate":[{translate_x},0.0,0.0]}},
                "CityObjects":{{
                    "{id}":{{
                        "type":"Building",
                        "geometry":[{{"type":"MultiSurface","lod":"1","boundaries":[[[0,1,2]]]}}]
                    }}
                }},
                "vertices":[[0,0,0],[1,0,0],[0,1,0]]
            }}"#
        );
        read_feature(bytes.as_bytes(), &ReadOptions::default()).expect("feature should parse")
    }

    #[test]
    fn merge_preserves_world_coordinates_when_transforms_differ() {
        let merged = merge([
            transformed_feature("first", 100.0),
            transformed_feature("second", 200.0),
        ])
        .expect("features should merge");
        let root: Value =
            serde_json::from_slice(&to_vec(&merged, &WriteOptions::default()).expect("serialize"))
                .expect("parse serialized root");

        assert!(root.get("transform").is_none());
        let world_xs = root
            .get("vertices")
            .and_then(Value::as_array)
            .expect("vertices should exist")
            .iter()
            .map(|vertex| {
                vertex
                    .get(0)
                    .and_then(Value::as_f64)
                    .expect("x vertex should be numeric")
            })
            .collect::<Vec<_>>();

        assert!(world_xs.iter().any(|x| (*x - 100.0).abs() < 1e-9));
        assert!(world_xs.iter().any(|x| (*x - 200.0).abs() < 1e-9));
    }
}
