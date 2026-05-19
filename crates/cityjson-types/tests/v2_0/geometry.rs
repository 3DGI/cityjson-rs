//! Geometry test suite.
//!
//! This module is the primary geometry specification for the crate.
//! It covers all geometry kinds, boundary layers, mapping rules, and
//! template / instance separation.
//!
//! ## Structure
//!
//! - `fixtures`:   canonical reusable geometry fixtures (P1, L1, S1, D1, MS1, T1, I1)
//! - `acceptance`: positive acceptance tests (families 1, 5, 7, 8)
//! - `roundtrip`:  boundary round-trip tests (family 4)
//! - `instances`:  template geometry and `GeometryInstance` separation (family 11)
//!
//! Unit-level tests for families 2, 3, 6, 9, and builder-order coverage for 10
//! live in the source crate
//! next to the data structures they test:
//! - `src/backend/default/boundary.rs` – boundary offset and kind-shape tests
//! - `src/backend/default/geometry_validation.rs` – stored-geometry shape and mapping validation
//! - `src/backend/default/geometry_builder.rs` – flattened traversal ordering across layers
//! - `src/resources/mapping.rs` – semantic/material map shape tests
//! - `src/resources/mapping/textures.rs` – texture topology tests

pub mod acceptance;
pub mod editing;
pub mod fixtures;
pub mod instances;
pub mod roundtrip;
