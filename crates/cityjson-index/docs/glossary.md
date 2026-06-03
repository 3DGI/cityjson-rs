# Glossary of cityjson-index Terms

This document defines the normalized terminology used by `cityjson-index`.

## CityJSON Terms

### CityJSON

A JSON encoding for 3D city models. A CityJSON document contains a `CityObjects` dictionary and shared arrays such as `vertices`.

### CityJSONFeature

A valid package-sized CityJSON payload. `cjindex get`, `cjindex query`, library package reads, and FFI/Python package reads return valid CityJSONFeature models.

### CityJSONSeq

Line-delimited CityJSON. A `.city.jsonl` stream has one CityJSON metadata header line followed by one CityJSONFeature per line. User-facing APIs use `cityjson-seq`; the old `ndjson` CLI alias is intentionally not accepted.

### Feature Files

A storage layout where each CityJSONFeature is stored in its own file and metadata is inherited from ancestor metadata files. The package type string is `feature-files`.

### CityObject Hierarchy

CityObjects can reference child CityObjects through `children` arrays and parent CityObjects through `parents` arrays. The normalized index stores these parent/child relationships in `cityobject_relationships`.

## Index Terms

### Sidecar

The SQLite database stored beside a dataset, normally `.cityjson-index.sqlite`.

### Source

A physical input context used for freshness checks and reconstruction. A source stores metadata and source-file state; it is not a public return unit.

### Package

The public return unit of the index. Every package record must reconstruct to a valid CityJSONFeature. Package types are `cityjson`, `cityjson-seq`, and `feature-files`.

### CityObject Record

A normalized occurrence of a CityObject key in a source package. Duplicate external ids are represented as distinct records and returned by plural lookup APIs in record-id order.

### Package Membership

The association between a package and the CityObjects it contains. Shared children can belong to more than one package.

### Bounds

3D package and CityObject bounds are stored in `package_bbox` and `cityobject_bbox`. CityObjects without geometry remain addressable and simply have no bbox record.

## Normalized Tables

| Table | Meaning |
|-------|---------|
| `schema_state` | Schema version and reindex requirements. |
| `sources` | Physical source metadata and freshness state. |
| `packages` | Public CityJSONFeature return units. |
| `cityobjects` | CityObject occurrences keyed by external id and type. |
| `package_cityobjects` | Package membership for CityObject occurrences. |
| `cityobject_relationships` | Parent/child CityObject hierarchy. |
| `package_bbox` | 3D bounds for package records. |
| `cityobject_bbox` | 3D bounds for CityObject records. |

ADR-008 supersedes the older feature-alias terminology from ADR-006.
