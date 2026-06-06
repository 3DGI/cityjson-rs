//! 3D City Database v5 Arrow Schema Components
//!
//! This module provides Arrow schema definitions and utilities for mapping
//! 3D City Database v5 (3DCityDB v5) tables to Apache Arrow format.
//! 
//! The schema is organized into components that align with the 3DCityDB v5
//! modular structure and CityJSON Arrow Schema for transformationless
//! import/export.

pub mod geometry;

pub use geometry::{
    CityDbGeometry, GeometryAppearance, GeometryProperties, ImplicitGeometry, Semantics,
    Transformation, geometry_schema,
};
