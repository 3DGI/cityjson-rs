---
title: Package schema
description: Imported cityjson-arrow specification page.
---

# Package schema

This document specifies the canonical table contract shared by `cityjson-arrow` streams
and `cityjson-parquet` packages.

## Terminology

The key words MUST, MUST NOT, REQUIRED, SHOULD, and OPTIONAL in this document are to be
interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## Version

| Field | Value |
|---|---|
| Schema id | `cityjson-arrow.package.v3alpha3` |
| Data model | `cityjson_types::v2_0::OwnedCityModel` |

## Canonical tables

The schema defines 24 canonical tables. Each table has a fixed integer tag used in
the live stream and a string name used in the persistent package manifest. Tables
MUST appear in tag order. A conforming producer MUST include all REQUIRED tables and
MUST NOT include any table more than once.

Tag 1 is reserved for the removed `transform` table and MUST be rejected by
`v3alpha3` readers. Coordinates are stored as materialized real-world coordinates;
CityJSON quantization metadata is not part of this transport schema.

| Tag | Name | Required | Description |
|-----|------|----------|-------------|
| 0 | `metadata` | REQUIRED | CityJSON metadata: model name, geographic extent, and spatial reference |
| 2 | `extensions` | OPTIONAL | CityJSON extension declarations referenced by city objects in this model |
| 3 | `vertices` | REQUIRED | Shared 3D vertex coordinates for all city object geometries |
| 4 | `template_vertices` | OPTIONAL | Vertex coordinates used by geometry templates |
| 5 | `texture_vertices` | OPTIONAL | UV coordinates for texture mapping |
| 6 | `semantics` | OPTIONAL | Semantic surface type definitions |
| 7 | `semantic_children` | OPTIONAL | Parent-child relationships between semantic surfaces |
| 8 | `materials` | OPTIONAL | Material definitions |
| 9 | `textures` | OPTIONAL | Texture definitions |
| 10 | `template_geometry_boundaries` | OPTIONAL | Boundary indices for geometry template surfaces |
| 11 | `template_geometry_semantics` | OPTIONAL | Semantic surface assignments for geometry templates |
| 12 | `template_geometry_materials` | OPTIONAL | Material assignments for geometry template surfaces |
| 13 | `template_geometry_ring_textures` | OPTIONAL | Texture UV assignments for geometry template rings |
| 14 | `template_geometries` | OPTIONAL | Geometry template definitions; instances reference these by ordinal |
| 15 | `geometry_boundaries` | REQUIRED | Boundary indices for city object geometry surfaces |
| 16 | `geometry_surface_semantics` | OPTIONAL | Semantic surface assignments for surface geometries |
| 17 | `geometry_point_semantics` | OPTIONAL | Semantic point assignments for point geometries |
| 18 | `geometry_linestring_semantics` | OPTIONAL | Semantic line assignments for line string geometries |
| 19 | `geometry_surface_materials` | OPTIONAL | Material assignments for geometry surfaces |
| 20 | `geometry_ring_textures` | OPTIONAL | Texture UV assignments for geometry rings |
| 21 | `geometry_instances` | OPTIONAL | Geometry instance records: template ordinal and placement transform |
| 22 | `geometries` | REQUIRED | Geometry definitions (type, LoD) linking city objects to boundary or instance tables |
| 23 | `cityobjects` | REQUIRED | City object records: identifier, type, and typed attributes |
| 24 | `cityobject_children` | OPTIONAL | Parent-child relationships between city objects |

## Header

Both live streams and persistent packages carry a `CityArrowHeader` object that
identifies the format version and the source model.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `package_version` | string | REQUIRED | Always `"cityjson-arrow.package.v3alpha3"` |
| `citymodel_id` | string | REQUIRED | Identifier for the source city model |
| `cityjson_version` | string | REQUIRED | CityJSON version of the source data, e.g. `"2.0"` |

## Projection layout

Both live streams and persistent packages carry a `ProjectionLayout` object. It records
the typed attribute column layout discovered from the model at export time. The decoder
uses it to validate column schemas and reconstruct nested dynamic attributes.

In the live stream, `ProjectionLayout` is embedded in the prelude JSON. In the persistent
package, it is embedded in the manifest JSON. A producer MUST include `ProjectionLayout`
in the prelude or manifest even when the model has no dynamic attributes; in that case
the value is an empty object (`{}`).

## Live stream prelude

The stream prelude is a UTF-8 JSON object. It MUST contain:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `header` | object | REQUIRED | A `CityArrowHeader` object |
| `projection` | object | REQUIRED | A `ProjectionLayout` object |

Minimal example:

```json
{
  "header": {
    "package_version": "cityjson-arrow.package.v3alpha3",
    "citymodel_id": "NL.IMBAG.Pand",
    "cityjson_version": "2.0"
  },
  "projection": {}
}
```

## Persistent manifest

The package manifest is a UTF-8 JSON object appended to the file after all table payloads.
It MUST contain:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `package_schema` | string | REQUIRED | Always `"cityjson-arrow.package.v3alpha3"` |
| `cityjson_version` | string | REQUIRED | CityJSON version of the source data |
| `citymodel_id` | string | REQUIRED | Identifier for the source city model |
| `projection` | object | REQUIRED | A `ProjectionLayout` object |
| `tables` | array | REQUIRED | Ordered list of table entries; see below |

Each entry in `tables` MUST contain:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | REQUIRED | Canonical table name |
| `offset` | uint64 | REQUIRED | Byte offset of the table payload from the start of the file |
| `length` | uint64 | REQUIRED | Byte length of the table payload |
| `rows` | uint64 | REQUIRED | Declared row count of the table payload |

A reader MUST reject a manifest whose `tables` array is not in canonical tag order.
A reader MUST reject a manifest that lists the same table name more than once.
A reader MUST reject a manifest that omits any REQUIRED table.

Minimal example (five required tables only):

```json
{
  "package_schema": "cityjson-arrow.package.v3alpha3",
  "cityjson_version": "2.0",
  "citymodel_id": "NL.IMBAG.Pand",
  "projection": {},
  "tables": [
    { "name": "metadata",            "offset": 22,   "length": 1024, "rows": 1   },
    { "name": "vertices",            "offset": 1046, "length": 4096, "rows": 500 },
    { "name": "geometry_boundaries", "offset": 5142, "length": 2048, "rows": 200 },
    { "name": "geometries",          "offset": 7190, "length": 512,  "rows": 50  },
    { "name": "cityobjects",         "offset": 7702, "length": 1024, "rows": 50  }
  ]
}
```

## Table schemas

Each canonical table is one Arrow `RecordBatch`. The column names, Arrow data types,
and nullability listed below are normative. A conforming producer MUST emit exactly these
columns in the order listed. A conforming reader MUST validate the schema of every payload
against the canonical schema before decoding any row data.

**Projection-dependent columns** are present only when the corresponding `ProjectionLayout`
entry in the prelude or manifest is non-null. A producer MUST include such a column if and
only if the matching projection spec is non-null. A reader MUST validate projection-dependent
columns against the projection spec received in the prelude or manifest.

**Type notation:**

| Notation | Arrow type |
|---|---|
| `utf8` | `Utf8` (32-bit offsets) |
| `large_utf8` | `LargeUtf8` (64-bit offsets) |
| `uint32` | `UInt32` |
| `uint64` | `UInt64` |
| `float32` | `Float32` |
| `float64` | `Float64` |
| `fixed_size_list<T>[N]` | `FixedSizeList` with element type `T` and size `N` |
| `list<T>` | `List` with element type `T` (32-bit offsets) |
| `struct{...}` | `Struct` with named child fields |

---

### `metadata` (tag 0)

One row per encoded city model.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `citymodel_id` | `large_utf8` | No | Unique identifier for the source city model |
| `cityjson_version` | `utf8` | No | CityJSON version string, e.g. `"2.0"` |
| `citymodel_kind` | `utf8` | No | Model kind; `"CityJSON"` or `"CityJSONFeature"` |
| `feature_root_id` | `large_utf8` | Yes | Root city object id for `CityJSONFeature` payloads; null otherwise |
| `identifier` | `large_utf8` | Yes | `metadata.identifier` value |
| `title` | `large_utf8` | Yes | `metadata.title` value |
| `reference_system` | `large_utf8` | Yes | CRS URI from `metadata.referenceSystem` |
| `geographical_extent` | `fixed_size_list<float64>[6]` | Yes | Bounding box `[minx, miny, minz, maxx, maxy, maxz]`; null if not set |
| `reference_date` | `utf8` | Yes | `metadata.referenceDate` |
| `default_material_theme` | `utf8` | Yes | Default material theme name |
| `default_texture_theme` | `utf8` | Yes | Default texture theme name |
| `point_of_contact` | `struct{...}` | Yes | Contact information; null if not set. See sub-fields below |
| `root_extra` | `struct{...}` | Yes | **Projection-dependent** (`root_extra`): extra top-level city model attributes |
| `metadata_extra` | `struct{...}` | Yes | **Projection-dependent** (`metadata_extra`): extra `metadata` attributes |

`point_of_contact` sub-fields:

| Sub-column | Type | Nullable | Description |
|------------|------|----------|-------------|
| `contact_name` | `large_utf8` | No | Contact person name |
| `email_address` | `large_utf8` | No | Email address |
| `role` | `utf8` | Yes | Contact role string (e.g. `"Author"`, `"Owner"`) |
| `website` | `large_utf8` | Yes | Website URL |
| `contact_type` | `utf8` | Yes | `"Individual"` or `"Organization"` |
| `phone` | `large_utf8` | Yes | Phone number |
| `organization` | `large_utf8` | Yes | Organization name |
| `address` | `struct{...}` | Yes | **Projection-dependent** (`metadata_point_of_contact_address`): address attributes |

### `extensions` (tag 2)

One row per CityJSON extension referenced by objects in this model.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `extension_name` | `utf8` | No | Extension name, e.g. `"+Noise"` |
| `uri` | `large_utf8` | No | Extension schema URI |
| `version` | `utf8` | Yes | Extension version string; null if not set |

---

### `vertices` (tag 3)

One row per shared vertex coordinate.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `vertex_id` | `uint64` | No | Unique vertex identifier (monotonically increasing) |
| `x` | `float64` | No | X coordinate |
| `y` | `float64` | No | Y coordinate |
| `z` | `float64` | No | Z coordinate |

---

### `template_vertices` (tag 4)

One row per vertex coordinate in a geometry template's local coordinate system.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_vertex_id` | `uint64` | No | Unique template vertex identifier (monotonically increasing) |
| `x` | `float64` | No | X coordinate in template local space |
| `y` | `float64` | No | Y coordinate in template local space |
| `z` | `float64` | No | Z coordinate in template local space |

---

### `texture_vertices` (tag 5)

One row per UV coordinate used for texture mapping.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `uv_id` | `uint64` | No | Unique UV coordinate identifier (monotonically increasing) |
| `u` | `float32` | No | U (horizontal) texture coordinate |
| `v` | `float32` | No | V (vertical) texture coordinate |

---

### `semantics` (tag 6)

One row per semantic surface definition.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `semantic_id` | `uint64` | No | Unique semantic surface identifier |
| `semantic_type` | `utf8` | No | Semantic type string (e.g. `"RoofSurface"`, `"WallSurface"`) |
| `parent_semantic_id` | `uint64` | Yes | Parent semantic surface id; null if this surface has no parent |
| `attributes` | `struct{...}` | Yes | **Projection-dependent** (`semantic_attributes`): semantic surface attributes |

---

### `semantic_children` (tag 7)

One row per parent–child relationship between semantic surfaces.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `parent_semantic_id` | `uint64` | No | Parent semantic surface identifier |
| `child_ordinal` | `uint32` | No | Ordinal position of this child within the parent's children list |
| `child_semantic_id` | `uint64` | No | Child semantic surface identifier |

---

### `materials` (tag 8)

One row per material definition. The schema varies based on `material_payload` in the
projection. A producer MUST append the payload columns from the projection spec
immediately after `material_id`, in the order they appear in the spec.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `material_id` | `uint64` | No | Unique material identifier |
| *(payload columns)* | varies | varies | **Projection-dependent** (`material_payload`): material property columns derived from the projection spec |

---

### `textures` (tag 9)

One row per texture definition. The schema varies based on `texture_payload` in the
projection.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `texture_id` | `uint64` | No | Unique texture identifier |
| `image_uri` | `large_utf8` | No | URI or path of the texture image |
| *(payload columns)* | varies | varies | **Projection-dependent** (`texture_payload`): texture property columns derived from the projection spec |

---

### `template_geometry_boundaries` (tag 10)

One row per geometry template. Encodes boundary topology using the same nested
offset-array structure as `geometry_boundaries`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier (matches `template_geometries`) |
| `vertex_indices` | `list<uint32>` | No | Flat list of vertex indices into `template_vertices` |
| `line_offsets` | `list<uint32>` | Yes | Offsets partitioning `vertex_indices` into line segments; null for geometries without explicit line structure |
| `ring_offsets` | `list<uint32>` | Yes | Offsets partitioning `line_offsets` into rings |
| `surface_offsets` | `list<uint32>` | Yes | Offsets partitioning `ring_offsets` into surfaces |
| `shell_offsets` | `list<uint32>` | Yes | Offsets partitioning `surface_offsets` into shells (for solid geometries) |
| `solid_offsets` | `list<uint32>` | Yes | Offsets partitioning `shell_offsets` into solids (for MultiSolid) |

---

### `template_geometry_semantics` (tag 11)

One row per (template geometry, primitive) pair that has a semantic surface assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier |
| `primitive_type` | `utf8` | No | Primitive kind: `"surface"`, `"point"`, or `"linestring"` |
| `primitive_ordinal` | `uint32` | No | Ordinal position of the primitive within the template geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic surface identifier; null if unassigned |

---

### `template_geometry_materials` (tag 12)

One row per (template geometry, surface, theme) material assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier |
| `primitive_type` | `utf8` | No | Primitive kind: `"surface"` |
| `primitive_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `theme` | `utf8` | No | Material theme name |
| `material_id` | `uint64` | No | Assigned material identifier |

---

### `template_geometry_ring_textures` (tag 13)

One row per (template geometry, surface, ring, theme) texture assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `ring_ordinal` | `uint32` | No | Ordinal position of the ring within the surface |
| `theme` | `utf8` | No | Texture theme name |
| `texture_id` | `uint64` | No | Assigned texture identifier |
| `uv_indices` | `list<uint64>` | No | UV coordinate indices into `texture_vertices`, one per vertex in the ring |

---

### `template_geometries` (tag 14)

One row per geometry template definition. Instances in `geometry_instances` reference
template geometries by `template_geometry_id`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Unique template geometry identifier |
| `geometry_type` | `utf8` | No | CityJSON geometry type string |
| `lod` | `utf8` | Yes | Level of detail string; null if not specified |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`geometry_extra`): extra geometry attributes |

---

### `geometry_boundaries` (tag 15)

One row per non-instance geometry. Encodes boundary topology using nested offset arrays.
Each offset list has `N+1` entries for `N` items: `offsets[i]` and `offsets[i+1]`
delimit item `i`. Null offset lists indicate the geometry type does not use that
level of nesting.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier (matches `geometries`) |
| `vertex_indices` | `list<uint32>` | No | Flat list of vertex indices into `vertices` |
| `line_offsets` | `list<uint32>` | Yes | Offsets partitioning `vertex_indices` into line segments |
| `ring_offsets` | `list<uint32>` | Yes | Offsets partitioning `line_offsets` into rings |
| `surface_offsets` | `list<uint32>` | Yes | Offsets partitioning `ring_offsets` into surfaces |
| `shell_offsets` | `list<uint32>` | Yes | Offsets partitioning `surface_offsets` into shells (for solid geometries) |
| `solid_offsets` | `list<uint32>` | Yes | Offsets partitioning `shell_offsets` into solids (for MultiSolid) |

---

### `geometry_surface_semantics` (tag 16)

One row per (geometry, surface) pair. Assigns a semantic surface to each surface of a
surface geometry.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic surface identifier; null if unassigned |

---

### `geometry_point_semantics` (tag 17)

One row per (geometry, point) pair. Assigns a semantic to each point of a
`MultiPoint` geometry.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `point_ordinal` | `uint32` | No | Ordinal position of the point within the geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic identifier; null if unassigned |

---

### `geometry_linestring_semantics` (tag 18)

One row per (geometry, line string) pair. Assigns a semantic to each line string of a
`MultiLineString` geometry.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `linestring_ordinal` | `uint32` | No | Ordinal position of the line string within the geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic identifier; null if unassigned |

---

### `geometry_surface_materials` (tag 19)

One row per (geometry, surface, theme) material assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `theme` | `utf8` | No | Material theme name |
| `material_id` | `uint64` | No | Assigned material identifier |

---

### `geometry_ring_textures` (tag 20)

One row per (geometry, surface, ring, theme) texture assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `ring_ordinal` | `uint32` | No | Ordinal position of the ring within the surface |
| `theme` | `utf8` | No | Texture theme name |
| `texture_id` | `uint64` | No | Assigned texture identifier |
| `uv_indices` | `list<uint64>` | No | UV coordinate indices into `texture_vertices`, one per vertex in the ring |

---

### `geometry_instances` (tag 21)

One row per geometry that is a template instance. References a template from
`template_geometries` and a placement origin vertex from `vertices`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Unique geometry identifier |
| `cityobject_ix` | `uint64` | No | Index of the owning city object (matches `cityobject_ix` in `cityobjects`) |
| `geometry_ordinal` | `uint32` | No | Ordinal position of this geometry within the city object's geometry list |
| `lod` | `utf8` | Yes | Level of detail string; null if not specified |
| `template_geometry_id` | `uint64` | No | References a row in `template_geometries` |
| `reference_point_vertex_id` | `uint64` | No | References a vertex in `vertices` used as the instance placement origin |
| `transform_matrix` | `fixed_size_list<float64>[16]` | Yes | Column-major 4×4 transformation matrix; null if identity |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`geometry_extra`): extra geometry attributes |

---

### `geometries` (tag 22)

One row per non-instance geometry definition. Links city objects to their boundary data
in `geometry_boundaries`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Unique geometry identifier (matches `geometry_id` in `geometry_boundaries`) |
| `cityobject_ix` | `uint64` | No | Index of the owning city object (matches `cityobject_ix` in `cityobjects`) |
| `geometry_ordinal` | `uint32` | No | Ordinal position of this geometry within the city object's geometry list |
| `geometry_type` | `utf8` | No | CityJSON geometry type string (e.g. `"Solid"`, `"MultiSurface"`) |
| `lod` | `utf8` | Yes | Level of detail string; null if not specified |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`geometry_extra`): extra geometry attributes |

---

### `cityobjects` (tag 23)

One row per city object.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `cityobject_id` | `large_utf8` | No | CityJSON city object identifier string |
| `cityobject_ix` | `uint64` | No | Sequential integer index; used as the join key by geometry and children tables |
| `object_type` | `utf8` | No | CityJSON object type string (e.g. `"Building"`, `"Road"`) |
| `geographical_extent` | `fixed_size_list<float64>[6]` | Yes | Bounding box `[minx, miny, minz, maxx, maxy, maxz]`; null if not computed |
| `attributes` | `struct{...}` | Yes | **Projection-dependent** (`cityobject_attributes`): typed city object attributes |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`cityobject_extra`): extra city object attributes not covered by `attributes` |

---

### `cityobject_children` (tag 24)

One row per parent–child relationship between city objects.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `parent_cityobject_ix` | `uint64` | No | `cityobject_ix` of the parent city object |
| `child_ordinal` | `uint32` | No | Ordinal position of this child within the parent's children list |
| `child_cityobject_ix` | `uint64` | No | `cityobject_ix` of the child city object |

## Table schemas

Each canonical table is one Arrow `RecordBatch`. The column names, Arrow data types,
and nullability listed below are normative. A conforming producer MUST emit exactly these
columns in the order listed. A conforming reader MUST validate the schema of every payload
against the canonical schema before decoding any row data.

**Projection-dependent columns** are present only when the corresponding `ProjectionLayout`
entry in the prelude or manifest is non-null. A producer MUST include such a column if and
only if the matching projection spec is non-null. A reader MUST validate projection-dependent
columns against the projection spec received in the prelude or manifest.

**Type notation:**

| Notation | Arrow type |
|---|---|
| `utf8` | `Utf8` (32-bit offsets) |
| `large_utf8` | `LargeUtf8` (64-bit offsets) |
| `uint32` | `UInt32` |
| `uint64` | `UInt64` |
| `float32` | `Float32` |
| `float64` | `Float64` |
| `fixed_size_list<T>[N]` | `FixedSizeList` with element type `T` and size `N` |
| `list<T>` | `List` with element type `T` (32-bit offsets) |
| `struct{...}` | `Struct` with named child fields |

---

### `metadata` (tag 0)

One row per encoded city model.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `citymodel_id` | `large_utf8` | No | Unique identifier for the source city model |
| `cityjson_version` | `utf8` | No | CityJSON version string, e.g. `"2.0"` |
| `citymodel_kind` | `utf8` | No | Model kind; `"CityJSON"` or `"CityJSONFeature"` |
| `feature_root_id` | `large_utf8` | Yes | Root city object id for `CityJSONFeature` payloads; null otherwise |
| `identifier` | `large_utf8` | Yes | `metadata.identifier` value |
| `title` | `large_utf8` | Yes | `metadata.title` value |
| `reference_system` | `large_utf8` | Yes | CRS URI from `metadata.referenceSystem` |
| `geographical_extent` | `fixed_size_list<float64>[6]` | Yes | Bounding box `[minx, miny, minz, maxx, maxy, maxz]`; null if not set |
| `reference_date` | `utf8` | Yes | `metadata.referenceDate` |
| `default_material_theme` | `utf8` | Yes | Default material theme name |
| `default_texture_theme` | `utf8` | Yes | Default texture theme name |
| `point_of_contact` | `struct{...}` | Yes | Contact information; null if not set. See sub-fields below |
| `root_extra` | `struct{...}` | Yes | **Projection-dependent** (`root_extra`): extra top-level city model attributes |
| `metadata_extra` | `struct{...}` | Yes | **Projection-dependent** (`metadata_extra`): extra `metadata` attributes |

`point_of_contact` sub-fields:

| Sub-column | Type | Nullable | Description |
|------------|------|----------|-------------|
| `contact_name` | `large_utf8` | No | Contact person name |
| `email_address` | `large_utf8` | No | Email address |
| `role` | `utf8` | Yes | Contact role string (e.g. `"Author"`, `"Owner"`) |
| `website` | `large_utf8` | Yes | Website URL |
| `contact_type` | `utf8` | Yes | `"Individual"` or `"Organization"` |
| `phone` | `large_utf8` | Yes | Phone number |
| `organization` | `large_utf8` | Yes | Organization name |
| `address` | `struct{...}` | Yes | **Projection-dependent** (`metadata_point_of_contact_address`): address attributes |

### `extensions` (tag 2)

One row per CityJSON extension referenced by objects in this model.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `extension_name` | `utf8` | No | Extension name, e.g. `"+Noise"` |
| `uri` | `large_utf8` | No | Extension schema URI |
| `version` | `utf8` | Yes | Extension version string; null if not set |

---

### `vertices` (tag 3)

One row per shared vertex coordinate.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `vertex_id` | `uint64` | No | Unique vertex identifier (monotonically increasing) |
| `x` | `float64` | No | X coordinate |
| `y` | `float64` | No | Y coordinate |
| `z` | `float64` | No | Z coordinate |

---

### `template_vertices` (tag 4)

One row per vertex coordinate in a geometry template's local coordinate system.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_vertex_id` | `uint64` | No | Unique template vertex identifier (monotonically increasing) |
| `x` | `float64` | No | X coordinate in template local space |
| `y` | `float64` | No | Y coordinate in template local space |
| `z` | `float64` | No | Z coordinate in template local space |

---

### `texture_vertices` (tag 5)

One row per UV coordinate used for texture mapping.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `uv_id` | `uint64` | No | Unique UV coordinate identifier (monotonically increasing) |
| `u` | `float32` | No | U (horizontal) texture coordinate |
| `v` | `float32` | No | V (vertical) texture coordinate |

---

### `semantics` (tag 6)

One row per semantic surface definition.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `semantic_id` | `uint64` | No | Unique semantic surface identifier |
| `semantic_type` | `utf8` | No | Semantic type string (e.g. `"RoofSurface"`, `"WallSurface"`) |
| `parent_semantic_id` | `uint64` | Yes | Parent semantic surface id; null if this surface has no parent |
| `attributes` | `struct{...}` | Yes | **Projection-dependent** (`semantic_attributes`): semantic surface attributes |

---

### `semantic_children` (tag 7)

One row per parent–child relationship between semantic surfaces.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `parent_semantic_id` | `uint64` | No | Parent semantic surface identifier |
| `child_ordinal` | `uint32` | No | Ordinal position of this child within the parent's children list |
| `child_semantic_id` | `uint64` | No | Child semantic surface identifier |

---

### `materials` (tag 8)

One row per material definition. The schema varies based on `material_payload` in the
projection. A producer MUST append the payload columns from the projection spec
immediately after `material_id`, in the order they appear in the spec.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `material_id` | `uint64` | No | Unique material identifier |
| *(payload columns)* | varies | varies | **Projection-dependent** (`material_payload`): material property columns derived from the projection spec |

---

### `textures` (tag 9)

One row per texture definition. The schema varies based on `texture_payload` in the
projection.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `texture_id` | `uint64` | No | Unique texture identifier |
| `image_uri` | `large_utf8` | No | URI or path of the texture image |
| *(payload columns)* | varies | varies | **Projection-dependent** (`texture_payload`): texture property columns derived from the projection spec |

---

### `template_geometry_boundaries` (tag 10)

One row per geometry template. Encodes boundary topology using the same nested
offset-array structure as `geometry_boundaries`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier (matches `template_geometries`) |
| `vertex_indices` | `list<uint32>` | No | Flat list of vertex indices into `template_vertices` |
| `line_offsets` | `list<uint32>` | Yes | Offsets partitioning `vertex_indices` into line segments; null for geometries without explicit line structure |
| `ring_offsets` | `list<uint32>` | Yes | Offsets partitioning `line_offsets` into rings |
| `surface_offsets` | `list<uint32>` | Yes | Offsets partitioning `ring_offsets` into surfaces |
| `shell_offsets` | `list<uint32>` | Yes | Offsets partitioning `surface_offsets` into shells (for solid geometries) |
| `solid_offsets` | `list<uint32>` | Yes | Offsets partitioning `shell_offsets` into solids (for MultiSolid) |

---

### `template_geometry_semantics` (tag 11)

One row per (template geometry, primitive) pair that has a semantic surface assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier |
| `primitive_type` | `utf8` | No | Primitive kind: `"surface"`, `"point"`, or `"linestring"` |
| `primitive_ordinal` | `uint32` | No | Ordinal position of the primitive within the template geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic surface identifier; null if unassigned |

---

### `template_geometry_materials` (tag 12)

One row per (template geometry, surface, theme) material assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier |
| `primitive_type` | `utf8` | No | Primitive kind: `"surface"` |
| `primitive_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `theme` | `utf8` | No | Material theme name |
| `material_id` | `uint64` | No | Assigned material identifier |

---

### `template_geometry_ring_textures` (tag 13)

One row per (template geometry, surface, ring, theme) texture assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Template geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `ring_ordinal` | `uint32` | No | Ordinal position of the ring within the surface |
| `theme` | `utf8` | No | Texture theme name |
| `texture_id` | `uint64` | No | Assigned texture identifier |
| `uv_indices` | `list<uint64>` | No | UV coordinate indices into `texture_vertices`, one per vertex in the ring |

---

### `template_geometries` (tag 14)

One row per geometry template definition. Instances in `geometry_instances` reference
template geometries by `template_geometry_id`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `template_geometry_id` | `uint64` | No | Unique template geometry identifier |
| `geometry_type` | `utf8` | No | CityJSON geometry type string |
| `lod` | `utf8` | Yes | Level of detail string; null if not specified |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`geometry_extra`): extra geometry attributes |

---

### `geometry_boundaries` (tag 15)

One row per non-instance geometry. Encodes boundary topology using nested offset arrays.
Each offset list has `N+1` entries for `N` items: `offsets[i]` and `offsets[i+1]`
delimit item `i`. Null offset lists indicate the geometry type does not use that
level of nesting.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier (matches `geometries`) |
| `vertex_indices` | `list<uint32>` | No | Flat list of vertex indices into `vertices` |
| `line_offsets` | `list<uint32>` | Yes | Offsets partitioning `vertex_indices` into line segments |
| `ring_offsets` | `list<uint32>` | Yes | Offsets partitioning `line_offsets` into rings |
| `surface_offsets` | `list<uint32>` | Yes | Offsets partitioning `ring_offsets` into surfaces |
| `shell_offsets` | `list<uint32>` | Yes | Offsets partitioning `surface_offsets` into shells (for solid geometries) |
| `solid_offsets` | `list<uint32>` | Yes | Offsets partitioning `shell_offsets` into solids (for MultiSolid) |

---

### `geometry_surface_semantics` (tag 16)

One row per (geometry, surface) pair. Assigns a semantic surface to each surface of a
surface geometry.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic surface identifier; null if unassigned |

---

### `geometry_point_semantics` (tag 17)

One row per (geometry, point) pair. Assigns a semantic to each point of a
`MultiPoint` geometry.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `point_ordinal` | `uint32` | No | Ordinal position of the point within the geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic identifier; null if unassigned |

---

### `geometry_linestring_semantics` (tag 18)

One row per (geometry, line string) pair. Assigns a semantic to each line string of a
`MultiLineString` geometry.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `linestring_ordinal` | `uint32` | No | Ordinal position of the line string within the geometry |
| `semantic_id` | `uint64` | Yes | Assigned semantic identifier; null if unassigned |

---

### `geometry_surface_materials` (tag 19)

One row per (geometry, surface, theme) material assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `theme` | `utf8` | No | Material theme name |
| `material_id` | `uint64` | No | Assigned material identifier |

---

### `geometry_ring_textures` (tag 20)

One row per (geometry, surface, ring, theme) texture assignment.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Geometry identifier |
| `surface_ordinal` | `uint32` | No | Ordinal position of the surface within the geometry |
| `ring_ordinal` | `uint32` | No | Ordinal position of the ring within the surface |
| `theme` | `utf8` | No | Texture theme name |
| `texture_id` | `uint64` | No | Assigned texture identifier |
| `uv_indices` | `list<uint64>` | No | UV coordinate indices into `texture_vertices`, one per vertex in the ring |

---

### `geometry_instances` (tag 21)

One row per geometry that is a template instance. References a template from
`template_geometries` and a placement origin vertex from `vertices`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Unique geometry identifier |
| `cityobject_ix` | `uint64` | No | Index of the owning city object (matches `cityobject_ix` in `cityobjects`) |
| `geometry_ordinal` | `uint32` | No | Ordinal position of this geometry within the city object's geometry list |
| `lod` | `utf8` | Yes | Level of detail string; null if not specified |
| `template_geometry_id` | `uint64` | No | References a row in `template_geometries` |
| `reference_point_vertex_id` | `uint64` | No | References a vertex in `vertices` used as the instance placement origin |
| `transform_matrix` | `fixed_size_list<float64>[16]` | Yes | Column-major 4×4 transformation matrix; null if identity |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`geometry_extra`): extra geometry attributes |

---

### `geometries` (tag 22)

One row per non-instance geometry definition. Links city objects to their boundary data
in `geometry_boundaries`.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `geometry_id` | `uint64` | No | Unique geometry identifier (matches `geometry_id` in `geometry_boundaries`) |
| `cityobject_ix` | `uint64` | No | Index of the owning city object (matches `cityobject_ix` in `cityobjects`) |
| `geometry_ordinal` | `uint32` | No | Ordinal position of this geometry within the city object's geometry list |
| `geometry_type` | `utf8` | No | CityJSON geometry type string (e.g. `"Solid"`, `"MultiSurface"`) |
| `lod` | `utf8` | Yes | Level of detail string; null if not specified |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`geometry_extra`): extra geometry attributes |

---

### `cityobjects` (tag 23)

One row per city object.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `cityobject_id` | `large_utf8` | No | CityJSON city object identifier string |
| `cityobject_ix` | `uint64` | No | Sequential integer index; used as the join key by geometry and children tables |
| `object_type` | `utf8` | No | CityJSON object type string (e.g. `"Building"`, `"Road"`) |
| `geographical_extent` | `fixed_size_list<float64>[6]` | Yes | Bounding box `[minx, miny, minz, maxx, maxy, maxz]`; null if not computed |
| `attributes` | `struct{...}` | Yes | **Projection-dependent** (`cityobject_attributes`): typed city object attributes |
| `extra` | `struct{...}` | Yes | **Projection-dependent** (`cityobject_extra`): extra city object attributes not covered by `attributes` |

---

### `cityobject_children` (tag 24)

One row per parent–child relationship between city objects.

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `parent_cityobject_ix` | `uint64` | No | `cityobject_ix` of the parent city object |
| `child_ordinal` | `uint32` | No | Ordinal position of this child within the parent's children list |
| `child_cityobject_ix` | `uint64` | No | `cityobject_ix` of the child city object |
