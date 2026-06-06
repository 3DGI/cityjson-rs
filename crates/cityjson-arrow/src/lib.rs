//! Arrow transport for `cityjson-rs`.
//!
//! The semantic boundary remains `cityjson_types::v2_0::OwnedCityModel`, but the
//! public transport surface is batch-first and stream-oriented.

mod codec;
mod convert;
pub mod error;
#[doc(hidden)]
pub mod internal;
pub mod schema;
mod stream;
#[doc(hidden)]
pub mod transport;

pub use codec::{
    ExportOptions, ImportOptions, ModelBatchDecoder, ModelBatchReader, SchemaVersion, WriteReport,
    export_reader, import_batches, read_stream, write_stream,
};
pub use schema::{
    CityArrowHeader, CityArrowPackageVersion, PackageManifest, PackageTableRef, ProjectedFieldSpec,
    ProjectedStructSpec, ProjectedValueSpec, ProjectionLayout, canonical_schema_set,
};
