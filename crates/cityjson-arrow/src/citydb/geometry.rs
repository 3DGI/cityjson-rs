//! Geometry Component for 3D City Database v5 Arrow Schema
//!
//! This module defines the Arrow schema for the Geometry Component, which maps
//! to the following 3DCityDB v5 tables:
//! - `GEOMETRY_DATA` - Main geometry storage (PostGIS spatial types)
//! - `GEOMETRY_PROPERTY` - JSON metadata for geometry hierarchy and structure
//! - `IMPLICIT_GEOMETRY` - Implicit geometry templates
//!
//! # Design Decisions
//!
//! 1. **WKB Encoding**: Geometries are stored as Well-Known Binary (WKB) in Arrow
//!    `Binary` type, matching GeoParquet specification.
//! 2. **Hierarchy Preservation**: CityGML geometry hierarchy (parent/child relationships)
//!    is stored as structured metadata in the `GeometryProperties` struct.
//! 3. **Implicit Geometry Support**: Template geometries that can be reused across
//!    features are stored in the `ImplicitGeometry` struct.
//! 4. **3D Support**: Full support for 3D geometry types (PolyhedralSurface, etc.).

use arrow::datatypes::{DataType, Field, Fields, Schema};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Main geometry struct representing a geometry in 3DCityDB v5
#[derive(Debug, Clone)]
pub struct CityDbGeometry {
    /// Unique identifier
    pub id: i64,
    
    /// Feature reference (which feature owns this geometry)
    pub feature_id: i64,
    
    /// Level of Detail (0, 1, 2, 3, 4)
    pub lod: i8,
    
    /// Geometry type (e.g., "Solid", "MultiSurface", "CompositeSurface")
    pub geometry_type: String,
    
    /// Geometry type code (from codelist)
    pub geometry_type_code: i32,
    
    /// The actual geometry data in WKB format
    /// This is the transformationless representation - direct from PostGIS
    pub geometry: Vec<u8>, // WKB bytes
    
    /// Geometry properties (JSON metadata from GEOMETRY_PROPERTY)
    pub properties: GeometryProperties,
    
    /// Is this an implicit geometry template?
    pub is_implicit: bool,
    
    /// For implicit geometries: the template definition
    pub implicit_geometry: Option<ImplicitGeometry>,
}

/// Geometry properties preserving CityGML geometry hierarchy and structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryProperties {
    /// Unique object identifier for this geometry component
    pub object_id: String,
    
    /// Parent reference in the geometry hierarchy
    /// 0-based index or -1 for root
    pub parent: i32,
    
    /// Children references
    pub children: Vec<i32>,
    
    /// Geometry index (for linking to spatial database primitives)
    pub geometry_index: i32,
    
    /// Is the geometry reversed (for orientable surfaces)
    pub is_reversed: bool,
    
    /// Semantics (what this geometry represents)
    pub semantics: Option<Semantics>,
    
    /// Texture/material assignments
    pub appearance: Option<GeometryAppearance>,
}

/// Semantic information for geometry surfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Semantics {
    /// Surface type (e.g., "RoofSurface", "WallSurface", "GroundSurface")
    pub surface_type: String,
    
    /// Surface type code (from codelist)
    pub surface_type_code: i32,
    
    /// Level of Detail
    pub lod: i8,
}

/// Appearance information for geometry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryAppearance {
    /// Reference to appearance ID
    pub appearance_id: Option<i64>,
    
    /// Surface-specific texture mapping
    pub texture_mapping: Option<TextureMapping>,
}

/// Texture mapping with UV coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureMapping {
    /// UV coordinates or other mapping info
    pub coordinates: Vec<f64>,
    
    /// Material reference
    pub material: Option<String>,
}

/// Implicit geometry template
#[derive(Debug, Clone)]
pub struct ImplicitGeometry {
    /// Unique identifier for the implicit geometry template
    pub id: i64,
    
    /// The template geometry in WKB
    pub template_geometry: Vec<u8>,
    
    /// Reference point for positioning the template
    pub reference_point: Option<[f64; 3]>,
    
    /// Transformation parameters
    pub transformation: Option<Transformation>,
    
    /// Which features use this implicit geometry
    pub used_by_features: Vec<i64>,
}

/// Transformation parameters for implicit geometry
#[derive(Debug, Clone)]
pub struct Transformation {
    /// Translation vector [x, y, z]
    pub translation: Option<[f64; 3]>,
    
    /// Rotation (Euler angles in radians [rx, ry, rz])
    pub rotation: Option<[f64; 3]>,
    
    /// Scale factors [sx, sy, sz]
    pub scale: Option<[f64; 3]>,
}

/// Create the Arrow schema for the Geometry Component
///
/// This schema maps to 3DCityDB v5 GEOMETRY_DATA, GEOMETRY_PROPERTY, and IMPLICIT_GEOMETRY tables
pub fn geometry_schema() -> Schema {
    Schema::new(vec![
        Field::new("geometry_id", DataType::Int64, false),
        Field::new("feature_id", DataType::Int64, false),
        Field::new("lod", DataType::Int8, false),
        Field::new("geometry_type", DataType::Utf8, false),
        Field::new("geometry_type_code", DataType::Int32, false),
        Field::new("geometry", DataType::Binary, false),
        Field::new("is_implicit", DataType::Boolean, false),
        
        // Geometry properties as nested struct
        Field::new(
            "properties",
            DataType::Struct(Fields::from(vec![
                Field::new("object_id", DataType::Utf8, false),
                Field::new("parent", DataType::Int32, false),
                Field::new(
                    "children", 
                    DataType::List(Arc::new(Field::new("item", DataType::Int32, false))),
                    false
                ),
                Field::new("geometry_index", DataType::Int32, false),
                Field::new("is_reversed", DataType::Boolean, false),
                
                // Semantics
                Field::new(
                    "semantics",
                    DataType::Struct(Fields::from(vec![
                        Field::new("surface_type", DataType::Utf8, true),
                        Field::new("surface_type_code", DataType::Int32, true),
                        Field::new("lod", DataType::Int8, true),
                    ])),
                    true
                ),
                
                // Appearance
                Field::new(
                    "appearance",
                    DataType::Struct(Fields::from(vec![
                        Field::new("appearance_id", DataType::Int64, true),
                        Field::new(
                            "texture_mapping",
                            DataType::Struct(Fields::from(vec![
                                Field::new(
                                    "coordinates", 
                                    DataType::List(Arc::new(Field::new("item", DataType::Float64, false))),
                                    true
                                ),
                                Field::new("material", DataType::Utf8, true),
                            ])),
                            true
                        ),
                    ])),
                    true
                ),
            ])),
            false
        ),
        
        // Implicit geometry (optional)
        Field::new(
            "implicit_geometry",
            DataType::Struct(Fields::from(vec![
                Field::new("id", DataType::Int64, true),
                Field::new("template_geometry", DataType::Binary, true),
                Field::new(
                    "reference_point",
                    DataType::List(Arc::new(Field::new("item", DataType::Float64, false))),
                    true
                ),
                Field::new(
                    "used_by_features",
                    DataType::List(Arc::new(Field::new("item", DataType::Int64, false))),
                    true
                ),
            ])),
            true
        ),
    ])
}

/// WKB type codes for CityGML geometry types
/// These match PostGIS/GEOS WKB type codes
pub mod wkb_type_codes {
    pub const POINT: u32 = 1001;
    pub const LINESTRING: u32 = 1002;
    pub const POLYGON: u32 = 1003;
    pub const MULTIPOINT: u32 = 1004;
    pub const MULTILINESTRING: u32 = 1005;
    pub const MULTIPOLYGON: u32 = 1006;
    pub const GEOMETRY_COLLECTION: u32 = 1007;
    pub const POLYHEDRAL_SURFACE: u32 = 1015;
    pub const TIN: u32 = 1016;
    pub const TRIANGLE: u32 = 1017;
}

/// Geometry type names for CityGML
pub mod geometry_types {
    pub const POINT: &str = "Point";
    pub const LINESTRING: &str = "LineString";
    pub const POLYGON: &str = "Polygon";
    pub const MULTIPOINT: &str = "MultiPoint";
    pub const MULTILINESTRING: &str = "MultiLineString";
    pub const MULTIPOLYGON: &str = "MultiPolygon";
    pub const MULTISURFACE: &str = "MultiSurface";
    pub const COMPOSITESURFACE: &str = "CompositeSurface";
    pub const SOLID: &str = "Solid";
    pub const MULTISOLID: &str = "MultiSolid";
    pub const COMPOSITESOLID: &str = "CompositeSolid";
    pub const POLYHEDRAL_SURFACE: &str = "PolyhedralSurface";
    pub const TIN: &str = "TriangulatedIrregularNetwork";
    pub const TRIANGLE: &str = "Triangle";
}

/// Surface type names for semantics
pub mod surface_types {
    pub const ROOF_SURFACE: &str = "RoofSurface";
    pub const WALL_SURFACE: &str = "WallSurface";
    pub const GROUND_SURFACE: &str = "GroundSurface";
    pub const FLOOR_SURFACE: &str = "FloorSurface";
    pub const CEILING_SURFACE: &str = "CeilingSurface";
    pub const DOOR_SURFACE: &str = "DoorSurface";
    pub const WINDOW_SURFACE: &str = "WindowSurface";
    pub const CLOSURE_SURFACE: &str = "ClosureSurface";
    pub const OUTER_CEILING_SURFACE: &str = "OuterCeilingSurface";
    pub const OUTER_FLOOR_SURFACE: &str = "OuterFloorSurface";
}


