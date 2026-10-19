# cityjson-index

Python bindings for the [`cityjson-index`](https://github.com/3DGI/cityjson)
Rust crate — a SQLite-backed random-access index over CityJSONSeq streams
for [CityJSON](https://www.cityjson.org) 2.0 data.

The distribution name is `cityjson-index`; the import name is
`cityjson_index`. The package ships its own prebuilt native library (built
from the Rust C-ABI core) and wraps it with `ctypes` — no Rust toolchain
is required on the user side. `cityjson-index` also depends on
`cityjson-lib==0.8.0` at runtime; each package ships its own native
library (there is no shared `.so`).

## Install

```bash
pip install cityjson-index
```

Prebuilt wheels are published for:

- Linux x86_64 (manylinux)
- macOS x86_64 and arm64
- Windows AMD64

Python 3.11, 3.12, and 3.13 are supported.

## Quick start

```python
from cityjson_index import OpenedIndex

with OpenedIndex.open("city.idx") as index:
    package = index.get("building-42")
    print(package.id, len(package.payload))
```

## Package source paths

Resolve one persisted backing path per package page while preserving order and duplicates:

```python
with OpenedIndex.open("dataset") as index:
    refs = index.package_ref_page_after_record_id(None, 512)
    paths = index.package_source_paths(refs)
    for ref, source_path in zip(refs, paths, strict=True):
        print(ref.record_id, source_path)
```

## Filtered reads

```python
from cityjson_index import PackageFilter, PackageFilterSummary, LodSelection, OpenedIndex

with OpenedIndex.open("dataset") as index:
    cityobjects = index.lookup_cityobject_refs("building-42")
    refs = index.package_refs_for_cityobject(cityobjects[0])
    filter = PackageFilter(
        cityobject_types={"Building"},
        default_lod=LodSelection.HIGHEST,
        lods_by_type={"Building": LodSelection.Exact(["2.0", "2.2"])},
    )

    summary = PackageFilterSummary()
    for package in index.read_filtered_packages(refs, filter):
        summary.add(package.diagnostics)
        print(package.model.summary().model_type)

    summary.ensure_requested_lods_available(filter)
```

`LodSelection.Exact(["2.0", "2.2"])` (or `.exact(...)`) accepts a nonempty
sequence of strings and retains geometry at either exact LoD. A single value
must also be wrapped in a sequence, for example `LodSelection.Exact(["2.0"])`.
Empty sequences raise `ValueError`. Diagnostics expose `requested_lods` as a
set of unavailable values alongside `available_lods`. Summaries combine
availability across packages; `ensure_requested_lods_available` raises if any
requested value remains unavailable, even if other values matched.

Exact default LoDs are validated for retained types that own input geometry.
Geometry-less structural ancestors remain in the output hierarchy without
requiring their own geometry. Explicit type overrides always require LoDs on
that type's own geometry; descendant geometry does not satisfy them.
In `available_lods`, an absent type means no owned geometry was observed; an
empty set means geometry was observed without LoD metadata and still requires
validation. These entries are preserved when reports are merged.

## Links

- Rust workspace and docs: <https://github.com/3DGI/cityjson>
- CityJSON specification: <https://www.cityjson.org>
- Issues: <https://github.com/3DGI/cityjson/issues>

## License

Dual-licensed under MIT or Apache-2.0, at your option.
