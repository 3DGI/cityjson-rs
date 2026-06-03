# ADR-008: Normalized Package And CityObject Indexing

## Status

Accepted. Supersedes ADR-006 for public API, schema terminology, and duplicate identifier semantics.

## Context

The previous feature-alias index made CityObject identifiers look like feature identifiers. That hid duplicate occurrences, mixed physical source details into public references, and made it difficult to guarantee that reads from every layout returned valid CityJSONFeature payloads.

## Decision

The index is normalized around sources, packages, CityObjects, package membership, CityObject hierarchy, and 3D bbox tables. Public lookup APIs are plural where duplicate external CityObject ids can exist. Public package reads return valid CityJSONFeature models for `cityjson`, `cityjson-seq`, and `feature-files` package types.

A source is physical reconstruction and freshness context. A package is the public return unit. CityObject records are normalized occurrences and may share the same external id.

## Consequences

- `cityjson-seq` replaces user-facing `ndjson` terminology.
- `lookup_cityobject_refs` is plural and returns every matching CityObject occurrence.
- `package_refs_for_cityobject`, `read_package`, `read_packages`, and `get_packages` are the package-oriented read path.
- `package_bbox` and `cityobject_bbox` carry complete XYZ bounds; non-spatial CityObjects remain addressable without bbox records.
- Legacy sidecars are opened only far enough to report that a rebuild is required.
- CLI and bindings must not expose first-match feature lookup conveniences.
