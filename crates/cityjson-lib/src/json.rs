use std::io::{BufRead, Write};
use std::path::Path;

pub use cityjson_json::CityJsonSeqWriteReport;
pub use cityjson_json::RootKind;

use crate::{CityJSONVersion, CityModel, Error, Result};

pub use cityjson_json::v2_0::{
    CityJsonSeqReader, CityJsonSeqWriteOptions, FeatureStreamTransform,
    ReadOptions as JsonReadOptions, WriteOptions as JsonWriteOptions, read_feature,
    read_feature_stream as read_feature_stream_raw,
    read_feature_with_base as read_feature_with_base_raw, read_model, to_vec as to_vec_raw,
    write_feature_stream as write_feature_stream_raw,
    write_feature_stream_with_base as write_feature_stream_with_base_raw, write_model,
};

pub mod staged {
    use std::io::Write;
    use std::path::Path;

    pub use cityjson_json::staged::{FeatureAssembly, FeatureObjectFragment};

    use crate::{CityModel, Error, Result};

    pub fn from_feature_slice_with_base(
        feature_bytes: &[u8],
        base_document_bytes: &[u8],
    ) -> Result<CityModel> {
        cityjson_json::staged::from_feature_slice_with_base(feature_bytes, base_document_bytes)
            .map(CityModel::from)
            .map_err(Error::from)
    }

    pub fn from_feature_slice_with_base_assume_cityjson_feature_v2_0(
        feature_bytes: &[u8],
        base_document_bytes: &[u8],
    ) -> Result<CityModel> {
        from_feature_slice_with_base(feature_bytes, base_document_bytes)
    }

    pub fn from_feature_slice_with_indexed_id_and_base(
        feature_bytes: &[u8],
        indexed_id: &str,
        base_document_bytes: &[u8],
    ) -> Result<CityModel> {
        cityjson_json::staged::from_feature_slice_with_indexed_id_and_base(
            feature_bytes,
            indexed_id,
            base_document_bytes,
        )
        .map(CityModel::from)
        .map_err(Error::from)
    }

    pub fn from_feature_assembly_with_base(
        assembly: FeatureAssembly<'_>,
        base_document_bytes: &[u8],
    ) -> Result<CityModel> {
        cityjson_json::staged::from_feature_assembly_with_base(assembly, base_document_bytes)
            .map(CityModel::from)
            .map_err(Error::from)
    }

    pub fn from_feature_file_with_base<P: AsRef<Path>>(
        path: P,
        base_document_bytes: &[u8],
    ) -> Result<CityModel> {
        cityjson_json::staged::from_feature_file_with_base(path, base_document_bytes)
            .map(CityModel::from)
            .map_err(Error::from)
    }

    pub fn to_feature_writer(writer: &mut impl Write, model: &CityModel) -> Result<()> {
        cityjson_json::staged::to_feature_writer(writer, model).map_err(Error::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    kind: RootKind,
    version: Option<CityJSONVersion>,
}

impl Probe {
    pub fn kind(&self) -> RootKind {
        self.kind
    }

    pub fn version(&self) -> Option<CityJSONVersion> {
        self.version
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WriteOptions {
    pub pretty: bool,
    pub validate_default_themes: bool,
}

fn write_options(options: WriteOptions) -> cityjson_json::WriteOptions {
    cityjson_json::WriteOptions {
        pretty: options.pretty,
        validate_default_themes: options.validate_default_themes,
        trailing_newline: false,
    }
}

pub fn probe(bytes: &[u8]) -> Result<Probe> {
    let probe = cityjson_json::probe(bytes).map_err(Error::from)?;
    let version = probe.version().map(CityJSONVersion::try_from).transpose()?;
    Ok(Probe {
        kind: probe.kind(),
        version,
    })
}

pub fn from_slice_assume_cityjson_v2_0(bytes: &[u8]) -> Result<CityModel> {
    cityjson_json::read_model(bytes, &cityjson_json::ReadOptions::default())
        .map(CityModel::from)
        .map_err(Error::from)
}

pub fn from_slice(bytes: &[u8]) -> Result<CityModel> {
    let probe = probe(bytes)?;
    match probe.kind() {
        RootKind::CityJSON => match probe.version() {
            Some(CityJSONVersion::V2_0) => from_slice_assume_cityjson_v2_0(bytes),
            None => Err(Error::MissingVersion),
            Some(other) => Err(Error::UnsupportedVersion {
                found: other.to_string(),
                supported: CityJSONVersion::V2_0.to_string(),
            }),
        },
        RootKind::CityJSONFeature => Err(Error::ExpectedCityJSON(probe.kind().to_string())),
    }
}

pub fn from_file<P: AsRef<Path>>(path: P) -> Result<CityModel> {
    let path = path.as_ref();
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("jsonl") => Err(Error::UnsupportedFeature(
            "CityJSONFeature streams must be read with json::read_feature_stream".into(),
        )),
        _ => from_slice(&std::fs::read(path)?),
    }
}

pub fn from_feature_slice(bytes: &[u8]) -> Result<CityModel> {
    let probe = probe(bytes)?;
    match probe.kind() {
        RootKind::CityJSON => Err(Error::ExpectedCityJSONFeature(probe.kind().to_string())),
        RootKind::CityJSONFeature => from_feature_slice_assume_cityjson_feature_v2_0(bytes),
    }
}

pub fn from_feature_file<P: AsRef<Path>>(path: P) -> Result<CityModel> {
    from_feature_slice(&std::fs::read(path)?)
}

pub fn from_feature_slice_assume_cityjson_feature_v2_0(bytes: &[u8]) -> Result<CityModel> {
    cityjson_json::read_feature(bytes, &cityjson_json::ReadOptions::default())
        .map(CityModel::from)
        .map_err(Error::from)
}

pub fn read_feature_stream<R>(reader: R) -> Result<impl Iterator<Item = Result<CityModel>>>
where
    R: BufRead,
{
    let iter = cityjson_json::read_feature_stream(reader, &cityjson_json::ReadOptions::default())?;
    Ok(iter.map(|item| item.map(CityModel::from).map_err(Error::from)))
}

pub fn read_cityjsonseq<R>(reader: R) -> Result<impl Iterator<Item = Result<CityModel>>>
where
    R: BufRead,
{
    read_feature_stream(reader)
}

pub fn write_feature_stream<I, W>(mut writer: W, models: I) -> Result<()>
where
    I: IntoIterator<Item = CityModel>,
    W: Write,
{
    for model in models {
        staged::to_feature_writer(&mut writer, &model).map_err(Error::from)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

pub fn write_feature_stream_refs<'a, I, W>(mut writer: W, models: I) -> Result<()>
where
    I: IntoIterator<Item = &'a CityModel>,
    W: Write,
{
    for model in models {
        staged::to_feature_writer(&mut writer, model).map_err(Error::from)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

pub fn write_cityjsonseq<I, W>(
    writer: W,
    base_root: &CityModel,
    features: I,
    transform: &cityjson_types::v2_0::Transform,
) -> Result<CityJsonSeqWriteReport>
where
    I: IntoIterator<Item = CityModel>,
    W: Write,
{
    let features = features.into_iter().collect::<Vec<_>>();
    write_cityjsonseq_refs(writer, base_root, features.iter(), transform)
}

pub fn write_cityjsonseq_refs<'a, I, W>(
    writer: W,
    base_root: &CityModel,
    features: I,
    transform: &cityjson_types::v2_0::Transform,
) -> Result<CityJsonSeqWriteReport>
where
    I: IntoIterator<Item = &'a CityModel>,
    W: Write,
{
    let options = cityjson_json::CityJsonSeqWriteOptions {
        transform: cityjson_json::FeatureStreamTransform::Explicit(transform.clone()),
        ..cityjson_json::CityJsonSeqWriteOptions::default()
    };
    cityjson_json::write_feature_stream_with_base(
        writer,
        base_root,
        features.into_iter().cloned(),
        &options,
    )
    .map_err(Error::from)
}

pub fn write_cityjsonseq_auto_transform<I, W>(
    writer: W,
    base_root: &CityModel,
    features: I,
    scale: [f64; 3],
) -> Result<CityJsonSeqWriteReport>
where
    I: IntoIterator<Item = CityModel>,
    W: Write,
{
    let features = features.into_iter().collect::<Vec<_>>();
    write_cityjsonseq_auto_transform_refs(writer, base_root, features.iter(), scale)
}

pub fn write_cityjsonseq_auto_transform_refs<'a, I, W>(
    writer: W,
    base_root: &CityModel,
    features: I,
    scale: [f64; 3],
) -> Result<CityJsonSeqWriteReport>
where
    I: IntoIterator<Item = &'a CityModel>,
    W: Write,
{
    let options = cityjson_json::CityJsonSeqWriteOptions {
        transform: cityjson_json::FeatureStreamTransform::Auto { scale },
        ..cityjson_json::CityJsonSeqWriteOptions::default()
    };
    cityjson_json::write_feature_stream_with_base(
        writer,
        base_root,
        features.into_iter().cloned(),
        &options,
    )
    .map_err(Error::from)
}

pub fn to_vec(model: &CityModel) -> Result<Vec<u8>> {
    cityjson_json::to_vec(model, &write_options(WriteOptions::default())).map_err(Error::from)
}

pub fn to_vec_with_options(model: &CityModel, options: WriteOptions) -> Result<Vec<u8>> {
    cityjson_json::to_vec(model, &write_options(options)).map_err(Error::from)
}

pub fn to_string(model: &CityModel) -> Result<String> {
    String::from_utf8(to_vec(model)?).map_err(|error| Error::Import(error.to_string()))
}

pub fn to_string_with_options(model: &CityModel, options: WriteOptions) -> Result<String> {
    String::from_utf8(to_vec_with_options(model, options)?)
        .map_err(|error| Error::Import(error.to_string()))
}

pub fn to_writer(writer: &mut impl Write, model: &CityModel) -> Result<()> {
    cityjson_json::write_model(writer, model, &write_options(WriteOptions::default()))
        .map_err(Error::from)
}

pub fn to_writer_with_options(
    writer: &mut impl Write,
    model: &CityModel,
    options: WriteOptions,
) -> Result<()> {
    cityjson_json::write_model(writer, model, &write_options(options)).map_err(Error::from)
}

pub fn to_feature_string(model: &CityModel) -> Result<String> {
    to_feature_string_with_options(model, WriteOptions::default())
}

pub fn to_feature_vec_with_options(model: &CityModel, options: WriteOptions) -> Result<Vec<u8>> {
    Ok(to_feature_string_with_options(model, options)?.into_bytes())
}

pub fn to_feature_string_with_options(model: &CityModel, options: WriteOptions) -> Result<String> {
    match model.type_citymodel() {
        cityjson_types::CityModelType::CityJSONFeature => to_string_with_options(model, options),
        other => Err(Error::UnsupportedType(other.to_string())),
    }
}

pub fn to_feature_writer(writer: &mut impl Write, model: &CityModel) -> Result<()> {
    staged::to_feature_writer(writer, model).map_err(Error::from)
}

pub fn merge_feature_stream_slice(bytes: &[u8]) -> Result<CityModel> {
    cityjson_json::merge_feature_stream_slice(bytes).map_err(Error::from)
}

pub fn merge_cityjsonseq_slice(bytes: &[u8]) -> Result<CityModel> {
    cityjson_json::merge_cityjsonseq_slice(bytes).map_err(Error::from)
}
