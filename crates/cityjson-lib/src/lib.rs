#![allow(clippy::all, clippy::pedantic)]
#![doc = include_str!("../docs/public-api.md")]

#[cfg(feature = "arrow")]
pub mod arrow;
mod error;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "json")]
pub mod ops;
#[cfg(feature = "parquet")]
pub mod parquet;
#[cfg(feature = "proj")]
mod proj;
pub mod query {
    pub use cityjson_types::query::{ModelSummary, summary};
}
mod version;

pub use Model as CityModel;
pub use cityjson_types;
pub use cityjson_types::v2_0::OwnedCityModel as Model;
pub use error::{Error, ErrorKind, Result};
pub use version::CityJSONVersion;
