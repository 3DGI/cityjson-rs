mod common;

use std::fs;
use std::path::Path;

use cityjson_index::CityIndex;
use common::{
    cityjson_feature, hierarchy_cityobjects, hierarchy_vertices, open_cityjson_index,
    open_cityjson_seq_index, open_feature_files_index, shared_child_cityobjects, temp_index_path,
    triangle_geometry, write_cityjson_fixture, write_cityjson_seq_fixture,
    write_feature_files_fixture,
};
use rusqlite::Connection;
use serde_json::json;

/// Input: a CityJSON document with one root Building and one BuildingPart child.
/// Assertions: reindex creates one package, two CityObjects, two memberships, and one relationship record.
#[test]
fn cityjson_scan_normalizes_root_package_cityobjects_and_relationships() {
    let index_path = temp_index_path("normalized-cityjson");
    let mut index = open_cityjson_index(
        &write_cityjson_fixture(
            "normalized-cityjson",
            hierarchy_cityobjects(),
            hierarchy_vertices(),
        ),
        &index_path,
    );
    index.reindex().expect("normalized CityJSON should reindex");

    assert_normalized_counts(&index_path, 1, 2, 2, 1);
}

/// Input: one CityJSONSeq feature line containing a root Building and BuildingPart child.
/// Assertions: reindex creates one package, two CityObjects, two memberships, and one relationship record without alias packages.
#[test]
fn cityjson_seq_scan_indexes_one_package_and_each_cityobject_once() {
    let index_path = temp_index_path("normalized-cityjson-seq");
    let root = write_cityjson_seq_fixture(
        "normalized-cityjson-seq",
        &[cityjson_feature(
            "building",
            hierarchy_cityobjects(),
            hierarchy_vertices(),
        )],
    );
    let mut index = open_cityjson_seq_index(&root, &index_path);
    index
        .reindex()
        .expect("normalized CityJSONSeq should reindex");

    assert_normalized_counts(&index_path, 1, 2, 2, 1);
}

/// Input: one feature-files package containing a root Building and BuildingPart child.
/// Assertions: reindex creates the same normalized package, CityObject, membership, and relationship counts as CityJSONSeq.
#[test]
fn feature_files_scan_indexes_one_package_and_each_cityobject_once() {
    let index_path = temp_index_path("normalized-feature-files");
    let root = write_feature_files_fixture(
        "normalized-feature-files",
        &cityjson_feature("building", hierarchy_cityobjects(), hierarchy_vertices()),
    );
    let mut index = open_feature_files_index(&root, &index_path);
    index
        .reindex()
        .expect("normalized feature files should reindex");

    assert_normalized_counts(&index_path, 1, 2, 2, 1);
}

/// Input: a geometry-less parent CityObject with a spatial descendant child.
/// Assertions: parent CityObject bounds and package bounds include the descendant XYZ extent.
#[test]
fn cityobject_bounds_include_descendant_geometry() {
    let index_path = temp_index_path("descendant-bounds");
    let mut index = open_cityjson_index(
        &write_cityjson_fixture(
            "descendant-bounds",
            hierarchy_cityobjects(),
            hierarchy_vertices(),
        ),
        &index_path,
    );
    index.reindex().expect("hierarchy should reindex");

    let conn = Connection::open(&index_path).expect("index should open with SQLite");
    assert_eq!(
        cityobject_bounds(&conn, "building"),
        (10.0, 14.0, 20.0, 26.0, 30.0, 36.0)
    );
    assert_eq!(package_bounds(&conn), (10.0, 14.0, 20.0, 26.0, 30.0, 36.0));
}

/// Input: a CityJSON document containing one CityObject with no geometry and no spatial descendants.
/// Assertions: the CityObject is indexed for id lookup while no cityobject_bbox RTree record is written.
#[test]
fn non_spatial_cityobject_is_addressable_without_rtree_record() {
    let index_path = temp_index_path("non-spatial-cityobject");
    let mut index = open_cityjson_index(
        &write_cityjson_fixture(
            "non-spatial-cityobject",
            json!({"addressable": {"type": "Building"}}),
            json!([]),
        ),
        &index_path,
    );
    index.reindex().expect("non-spatial object should reindex");

    let conn = Connection::open(&index_path).expect("index should open with SQLite");
    assert_eq!(
        scalar(
            &conn,
            "SELECT COUNT(*) FROM cityobjects WHERE external_id = 'addressable'"
        ),
        1
    );
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM cityobject_bbox"), 0);
}

/// Input: a spatial root-child hierarchy with non-zero Z coordinates.
/// Assertions: package_bbox and cityobject_bbox expose six XYZ columns while packages and cityobjects do not duplicate min_z or max_z.
#[test]
fn bbox_rtrees_store_complete_xyz_bounds() {
    let index_path = temp_index_path("xyz-rtrees");
    let mut index = open_cityjson_index(
        &write_cityjson_fixture("xyz-rtrees", hierarchy_cityobjects(), hierarchy_vertices()),
        &index_path,
    );
    index.reindex().expect("spatial hierarchy should reindex");

    let conn = Connection::open(&index_path).expect("index should open with SQLite");
    for table in ["packages", "cityobjects"] {
        let columns = table_columns(&conn, table);
        assert!(
            !columns
                .iter()
                .any(|column| column == "min_z" || column == "max_z")
        );
    }
    assert_eq!(
        table_columns(&conn, "package_bbox"),
        [
            "package_id",
            "min_x",
            "max_x",
            "min_y",
            "max_y",
            "min_z",
            "max_z"
        ]
    );
    assert_eq!(
        table_columns(&conn, "cityobject_bbox"),
        [
            "cityobject_id",
            "min_x",
            "max_x",
            "min_y",
            "max_y",
            "min_z",
            "max_z"
        ]
    );
}

/// Input: a CityJSON document with two root Buildings that share one BuildingPart child.
/// Assertions: reindex creates two packages, three CityObjects, four memberships, two relationships, and two memberships for the shared child.
#[test]
fn cityjson_shared_child_has_multiple_package_memberships() {
    let index_path = temp_index_path("shared-child");
    let mut index = open_cityjson_index(
        &write_cityjson_fixture(
            "shared-child",
            shared_child_cityobjects(),
            hierarchy_vertices(),
        ),
        &index_path,
    );
    index.reindex().expect("shared child should reindex");

    assert_normalized_counts(&index_path, 2, 3, 4, 2);
    let conn = Connection::open(index_path).expect("index should open with SQLite");
    assert_eq!(
        scalar(
            &conn,
            "SELECT COUNT(*) FROM package_cityobjects pc JOIN cityobjects c ON c.id = pc.cityobject_id WHERE c.external_id = 'shared-part'",
        ),
        2,
    );
}

/// Input: two CityJSONSeq feature lines that reuse the external CityObject id duplicate.
/// Assertions: reindex stores two packages and two distinct CityObject records for the duplicated id.
#[test]
fn duplicate_external_ids_remain_distinct_occurrences() {
    let index_path = temp_index_path("duplicate-cityobject-id");
    let root = write_cityjson_seq_fixture(
        "duplicate-cityobject-id",
        &[
            cityjson_feature(
                "first",
                json!({"duplicate": spatial_object(0)}),
                json!([[0, 0, 0], [1, 0, 1], [0, 1, 2]]),
            ),
            cityjson_feature(
                "second",
                json!({"duplicate": spatial_object(0)}),
                json!([[10, 0, 0], [11, 0, 1], [10, 1, 2]]),
            ),
        ],
    );
    let mut index = open_cityjson_seq_index(&root, &index_path);
    index
        .reindex()
        .expect("duplicate external IDs should reindex");

    let conn = Connection::open(index_path).expect("index should open with SQLite");
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM packages"), 2);
    assert_eq!(
        scalar(
            &conn,
            "SELECT COUNT(*) FROM cityobjects WHERE external_id = 'duplicate'"
        ),
        2
    );
}

/// Input: a CityJSON hierarchy whose child reference points to a missing CityObject id.
/// Assertions: reindex fails and the error identifies the missing target id.
#[test]
fn reindex_rejects_missing_relationship_target() {
    let index_path = temp_index_path("missing-relationship-target");
    let mut index = open_cityjson_index(
        &write_cityjson_fixture(
            "missing-relationship-target",
            json!({"building": {"type": "Building", "children": ["missing-part"], "geometry": [triangle_geometry(0, "1.0")]}}),
            json!([[0, 0, 0], [1, 0, 1], [0, 1, 2]]),
        ),
        &index_path,
    );

    assert_error_contains(index.reindex(), "missing-part");
}

/// Input: a CityJSON hierarchy where two spatial CityObjects reference each other as parent and child.
/// Assertions: reindex fails before replacement and reports a relationship cycle.
#[test]
fn reindex_rejects_relationship_cycle() {
    let index_path = temp_index_path("relationship-cycle");
    let mut index = open_cityjson_index(
        &write_cityjson_fixture(
            "relationship-cycle",
            json!({
                "a": {"type": "Building", "children": ["b"], "parents": ["b"], "geometry": [triangle_geometry(0, "1.0")]},
                "b": {"type": "BuildingPart", "children": ["a"], "parents": ["a"], "geometry": [triangle_geometry(3, "1.0")]}
            }),
            json!([
                [0, 0, 0],
                [1, 0, 1],
                [0, 1, 2],
                [10, 0, 0],
                [11, 0, 1],
                [10, 1, 2]
            ]),
        ),
        &index_path,
    );

    assert_error_contains(index.reindex(), "cycle");
}

/// Input: a valid indexed hierarchy followed by an invalid source edit with a missing child reference.
/// Assertions: the failed reindex reports the missing id and leaves the previous CityObject records intact.
#[test]
fn failed_reindex_preserves_previous_index() {
    let index_path = temp_index_path("failed-reindex-preserves-index");
    let root = write_cityjson_fixture(
        "failed-reindex-preserves-index",
        hierarchy_cityobjects(),
        hierarchy_vertices(),
    );
    let source = root.join("fixture.city.json");
    let mut index = open_cityjson_index(&root, &index_path);
    index.reindex().expect("valid source should reindex");
    let before = scalar(
        &Connection::open(&index_path).expect("SQLite open"),
        "SELECT COUNT(*) FROM cityobjects",
    );

    fs::write(
        source,
        serde_json::to_vec(&json!({
            "type": "CityJSON",
            "version": "2.0",
            "transform": {"scale": [1.0, 1.0, 1.0], "translate": [0.0, 0.0, 0.0]},
            "CityObjects": {"building": {"type": "Building", "children": ["missing-part"], "geometry": [triangle_geometry(0, "1.0")]}},
            "vertices": [[0, 0, 0], [1, 0, 1], [0, 1, 2]]
        }))
        .expect("invalid fixture should serialize"),
    )
    .expect("invalid source should be writable");

    assert_error_contains(index.reindex(), "missing-part");
    assert_eq!(
        scalar(
            &Connection::open(index_path).expect("SQLite open"),
            "SELECT COUNT(*) FROM cityobjects"
        ),
        before
    );
}

/// Input: an existing legacy sidecar containing features, feature_bbox, and bbox_map tables.
/// Assertions: opening the index marks schema_state.needs_reindex so legacy data is rebuilt rather than migrated in place.
#[test]
fn legacy_sidecar_requires_reindex() {
    let root = write_cityjson_fixture(
        "legacy-sidecar",
        hierarchy_cityobjects(),
        hierarchy_vertices(),
    );
    let index_path = temp_index_path("legacy-sidecar");
    create_legacy_sidecar(&index_path);

    let _index = open_cityjson_index(&root, &index_path);
    let conn = Connection::open(index_path).expect("legacy sidecar should remain inspectable");
    assert_eq!(
        scalar(&conn, "SELECT needs_reindex FROM schema_state WHERE id = 1"),
        1
    );
}

/// Input: a sidecar with schema_state.schema_version set to 999.
/// Assertions: opening the index fails and the unsupported version appears in the error message.
#[test]
fn future_schema_version_is_rejected() {
    let root = write_cityjson_fixture(
        "future-schema",
        hierarchy_cityobjects(),
        hierarchy_vertices(),
    );
    let index_path = temp_index_path("future-schema");
    let conn = Connection::open(&index_path).expect("future sidecar should be creatable");
    conn.execute_batch(
        "CREATE TABLE schema_state (id INTEGER PRIMARY KEY, schema_version INTEGER NOT NULL, needs_reindex INTEGER NOT NULL); INSERT INTO schema_state VALUES (1, 999, 0);",
    )
    .expect("future schema marker should be writable");
    drop(conn);

    let error = CityIndex::open(
        cityjson_index::StorageLayout::CityJson { paths: vec![root] },
        &index_path,
    )
    .err()
    .expect("future schema should be rejected");
    assert!(error.to_string().contains("999"));
}

fn assert_normalized_counts(
    index_path: &Path,
    packages: i64,
    cityobjects: i64,
    memberships: i64,
    relationships: i64,
) {
    let conn = Connection::open(index_path).expect("index should open with SQLite");
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM packages"), packages);
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cityobjects"),
        cityobjects
    );
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM package_cityobjects"),
        memberships
    );
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cityobject_relationships"),
        relationships
    );
}

fn scalar(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0))
        .expect("scalar SQL query should succeed")
}

fn cityobject_bounds(conn: &Connection, external_id: &str) -> (f64, f64, f64, f64, f64, f64) {
    conn.query_row(
        "SELECT b.min_x, b.max_x, b.min_y, b.max_y, b.min_z, b.max_z FROM cityobjects c JOIN cityobject_bbox b ON b.cityobject_id = c.id WHERE c.external_id = ?1",
        [external_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    )
    .expect("CityObject bounds should exist")
}

fn package_bounds(conn: &Connection) -> (f64, f64, f64, f64, f64, f64) {
    conn.query_row(
        "SELECT min_x, max_x, min_y, max_y, min_z, max_z FROM package_bbox",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        },
    )
    .expect("package bounds should exist")
}

fn table_columns(conn: &Connection, table: &str) -> Vec<String> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("table info should prepare");
    statement
        .query_map([], |row| row.get(1))
        .expect("table info should query")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("table columns should decode")
}

fn spatial_object(start: usize) -> serde_json::Value {
    json!({"type": "Building", "geometry": [triangle_geometry(start, "1.0")]})
}

fn assert_error_contains<T>(result: cityjson_lib::Result<T>, expected: &str) {
    let error = result.err().expect("operation should fail");
    assert!(
        error.to_string().contains(expected),
        "expected error containing {expected:?}, got {error}"
    );
}

fn create_legacy_sidecar(path: &Path) {
    Connection::open(path)
        .expect("legacy sidecar should open")
        .execute_batch(
            r#"
            CREATE TABLE sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                metadata TEXT NOT NULL,
                vertices_offset INTEGER,
                vertices_length INTEGER,
                source_size INTEGER,
                source_mtime_ns INTEGER
            );
            CREATE TABLE features (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                feature_id TEXT NOT NULL,
                source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                file_size INTEGER,
                file_mtime_ns INTEGER,
                offset INTEGER NOT NULL,
                length INTEGER NOT NULL,
                min_z REAL,
                max_z REAL,
                cityobject_count INTEGER,
                member_ranges TEXT
            );
            CREATE VIRTUAL TABLE feature_bbox USING rtree(feature_rowid, min_x, max_x, min_y, max_y);
            CREATE TABLE bbox_map (feature_rowid INTEGER PRIMARY KEY, feature_id TEXT NOT NULL);
            "#,
        )
        .expect("legacy sidecar schema should be writable");
}
