# Glossary of cityjson-index Terms

This document clarifies the terminology used throughout the `cityjson-index` codebase, ADRs, and documentation. Many terms are specific to CityJSON and the indexing strategies used by this crate.

---

## Core CityJSON Concepts

### CityJSON

A JSON-based encoding standard for 3D city models and landscapes. A CityJSON file contains a `CityObjects` dictionary where each key is a unique identifier for a city object (building, road, tree, etc.).

### CityJSONFeature

A variant of CityJSON designed for streaming and modular datasets. Each CityJSONFeature is a self-contained JSON object that typically contains its own `CityObjects` dictionary. Multiple CityJSONFeatures can be stored in:
- A line-delimited JSON file (`.city.jsonl` or NDJSON format)
- Individual `.city.json` files in a directory structure

### Root CityObject

A CityObject that is **not listed as a child** of another CityObject. In CityJSON, objects can form hierarchies where one CityObject references others via its `children` array.

**Example:**
```json
{
  "CityObjects": {
    "building-1": {
      "type": "Building",
      "children": ["building-1-roof", "building-1-wall"]
    },
    "building-1-roof": {
      "type": "BuildingPart"
    },
    "building-1-wall": {
      "type": "BuildingPart"
    }
  }
}
```

Here, `building-1` is a **root CityObject** (it is not a child of any other object). The objects `building-1-roof` and `building-1-wall` are its **child members**.

### Child Members / Child CityObjects

CityObjects that are listed in another CityObject's `children` array. They represent parts or components of a parent object.

In the example above, `building-1-roof` and `building-1-wall` are **child members** of the `building-1` root CityObject.

---

## Index-Specific Concepts

### Input Package

A single unit of data that the indexer processes. The meaning depends on the storage layout:

| Layout | Input Package |
|--------|---------------|
| Regular CityJSON | One `.city.json` file containing many CityObjects |
| CityJSONFeature (NDJSON) | One line in a `.city.jsonl` file (one CityJSONFeature) |
| Feature-file directory | One standalone `.city.json` file (one CityJSONFeature) |

### Indexed Feature Row

A single row in the SQLite `features` table that represents one **addressable unit** that can be looked up and retrieved via the index.

The relationship between CityObjects and indexed feature rows depends on the layout:

| Layout | CityObjects → Feature Rows |
|--------|-----------------------------|
| Regular CityJSON | One row per **root CityObject** only |
| CityJSONFeature (NDJSON) | One row per **CityObject key** (both roots and children) |

### member_ranges Field

A field in the `features` table that stores JSON-encoded byte range information for child CityObjects. Each entry is an array of objects with:
- `id`: The CityObject identifier
- `offset`: Byte offset in the source file
- `length`: Byte length in the source file

**Purpose:** Enables efficient partial loading of a root CityObject's children without reading the entire source file.

**When populated:**
- ✅ **Regular CityJSON**: Populated for root CityObjects to track their child members
- ❌ **CityJSONFeature/NDJSON**: Always `NULL` — each CityObject gets its own feature row

**Example value:**
```json
[
  {"id": "building-1-roof", "offset": 1024, "length": 512},
  {"id": "building-1-wall", "offset": 2048, "length": 768}
]
```

### cityobject_count Field

The number of CityObjects associated with a feature row:

| Layout | Meaning |
|--------|---------|
| Regular CityJSON | Number of child members (from `children` array) + 1 (the root itself) |
| CityJSONFeature/NDJSON | Number of CityObjects in that specific feature package |

---

## Storage Layouts

`cityjson-index` supports three distinct storage layouts, each with different indexing behavior:

### 1. Regular CityJSON Layout
- **File format:** Single `.city.json` file
- **Content:** One document with shared `CityObjects`, `vertices`, `metadata`
- **Indexing:**
  - One feature row per **root CityObject**
  - `member_ranges` tracks byte offsets of child members
  - Parent-child relationships are indexed at the database level

### 2. CityJSONFeature NDJSON Layout
- **File format:** `.city.jsonl` file (line-delimited JSON)
- **Content:** Multiple CityJSONFeature objects, one per line
- **Indexing:**
  - One feature row per **CityObject key** in each feature
  - `member_ranges` is always `NULL`
  - Each CityObject is indexed independently

### 3. Feature-file Directory Layout
- **File format:** Directory of individual `.city.json` files
- **Content:** Each file is a standalone CityJSONFeature
- **Indexing:**
  - One feature row per **CityObject key** in each file
  - `member_ranges` is always `NULL`
  - Same behavior as NDJSON layout

---

## Key Architectural Decision

The different handling of `member_ranges` stems from **ADR-006**: the index treats regular CityJSON and feature-based layouts differently:

- **Regular CityJSON** preserves parent-child relationships at the index level (via `member_ranges`)
- **Feature layouts** (NDJSON, feature-file) flatten the hierarchy — every CityObject key becomes an independent index entry

This design allows:
- Efficient loading of building complexes in regular CityJSON (fetch parent + all children in one operation)
- Flexible addressing in feature layouts (each CityObject is directly addressable)
