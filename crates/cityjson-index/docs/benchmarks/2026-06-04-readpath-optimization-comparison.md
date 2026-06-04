# Read-path optimization comparison

Date: 2026-06-04

Previous implementation: `8208c8e` (`2026-06-04-previous-8208c8e-readpath.json`)
Current implementation: working tree after direct `get` lookup and lighter scalar `read_package` location lookup (`2026-06-04-current-readpath.json`)

Benchmark command for both runs:

```sh
just bench-index-json --case single-tile-subsets --layout city-json --layout city-json-seq --layout feature-files --workers 1
```

## Summary

- Matched rows: 264
- Current faster: 185/264
- Mean current/previous elapsed ratio: 0.981x

## Operation means

| operation | rows | faster rows | mean elapsed ratio | best | worst |
|---|---:|---:|---:|---:|---:|
| `bbox_query` | 48 | 32 | 0.998x | 0.945x | 1.102x |
| `cityobject_bbox_query` | 48 | 29 | 0.993x | 0.909x | 1.146x |
| `cityobject_full_scan_reference_iteration` | 12 | 11 | 0.983x | 0.956x | 1.038x |
| `cityobject_id_lookup` | 12 | 6 | 1.004x | 0.967x | 1.050x |
| `dataset_open` | 12 | 9 | 0.950x | 0.682x | 1.064x |
| `full_scan_reference_iteration` | 12 | 9 | 0.985x | 0.960x | 1.032x |
| `get` | 36 | 29 | 0.928x | 0.713x | 1.165x |
| `index_reindex` | 12 | 6 | 0.999x | 0.971x | 1.030x |
| `package_bbox_lookup_only` | 48 | 34 | 0.996x | 0.952x | 1.115x |
| `read_package` | 12 | 12 | 0.923x | 0.901x | 0.960x |
| `read_packages` | 12 | 8 | 0.999x | 0.983x | 1.020x |

## Layout means

| layout | rows | faster rows | mean elapsed ratio | best | worst |
|---|---:|---:|---:|---:|---:|
| `city-json` | 88 | 66 | 0.979x | 0.682x | 1.064x |
| `city-json-seq` | 88 | 62 | 0.978x | 0.713x | 1.165x |
| `feature-files` | 88 | 57 | 0.985x | 0.849x | 1.102x |

## Read-path focus

- `get`: 36 rows, 29 faster, mean `0.928x`, best `0.713x`, worst `1.165x`.
- `read_package`: 12 rows, 12 faster, mean `0.923x`, best `0.901x`, worst `0.960x`.
- `read_packages`: 12 rows, 8 faster, mean `0.999x`, best `0.983x`, worst `1.020x`.

## Notes

- `get` now skips the intermediate CityObject-reference vector and looks up distinct containing package refs directly by CityObject external id.
- Scalar `read_package` now reuses the caller-provided `IndexedPackageRef` and fetches only source location columns from `packages`; batch `read_packages` keeps the existing deduped location/member path.
- Issue #2 sidecar building is already parallel at the backend scan stage through `CITYJSON_INDEX_WORKERS`; this change does not rewrite the remaining single-transaction SQLite insertion pipeline.
