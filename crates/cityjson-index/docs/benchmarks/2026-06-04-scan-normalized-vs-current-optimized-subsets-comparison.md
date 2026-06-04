# Scan-Normalized Vs Current Optimized Subset Benchmark Comparison

Compares the scan-time normalization implementation with the latest stored optimized implementation on the same release-mode subset benchmark scope across all three storage layouts.

## Inputs

- Baseline JSON: [`2026-06-04-current-optimized-subsets-all-layouts.json`](2026-06-04-current-optimized-subsets-all-layouts.json)
- New JSON: [`2026-06-04-scan-normalized-subsets-all-layouts.json`](2026-06-04-scan-normalized-subsets-all-layouts.json)
- Benchmark case: `single-tile-subsets`
- Layouts: `city-json`, `city-json-seq`, `feature-files`
- Workers: `1`
- Release mode: yes

## Command

```bash
just bench-index-json --case single-tile-subsets --layout city-json --layout city-json-seq --layout feature-files --workers 1 \
  > /tmp/cityjson-index-scan-normalized-subsets-all-layouts.json
```

## Method

Rows are matched by layout, subset size, worker count, operation, and variant. Ratios are `new / baseline`, so values below `1.0x` mean the scan-time normalization implementation was faster.

## Summary

- Matched rows: `132`.
- New faster rows: `70/132`.
- New slower rows: `62/132`.
- Mean new/baseline elapsed-time ratio: `0.975x`.

## Layout Summary

| Layout | Rows | Mean New/Baseline | New Faster Rows |
|---|---:|---:|---:|
| city-json | 44 | 0.982x | 22 |
| city-json-seq | 44 | 0.974x | 23 |
| feature-files | 44 | 0.969x | 25 |

## Operation Summary

| Operation | Rows | Mean New/Baseline | New Faster Rows |
|---|---:|---:|---:|
| bbox_query | 48 | 0.996x | 32 |
| dataset_open | 12 | 1.016x | 5 |
| full_scan_reference_iteration | 12 | 1.026x | 2 |
| get | 36 | 1.036x | 9 |
| index_reindex | 12 | 0.603x | 12 |
| read_package | 12 | 0.990x | 10 |

## Focused Reindex Rows

| Layout | Subset | Baseline | New | New/Baseline | Delta |
|---|---:|---:|---:|---:|---:|
| city-json | 1000 | 123.8ms | 92.7ms | 0.749x | -25.1% |
| city-json | 5000 | 288.1ms | 211.9ms | 0.736x | -26.4% |
| city-json | 10000 | 623.0ms | 445.1ms | 0.714x | -28.6% |
| city-json | 25000 | 2.087s | 1.538s | 0.737x | -26.3% |
| city-json-seq | 1000 | 127.3ms | 71.6ms | 0.562x | -43.8% |
| city-json-seq | 5000 | 299.8ms | 167.9ms | 0.560x | -44.0% |
| city-json-seq | 10000 | 633.1ms | 334.5ms | 0.528x | -47.2% |
| city-json-seq | 25000 | 2.146s | 1.094s | 0.510x | -49.0% |
| feature-files | 1000 | 134.5ms | 73.5ms | 0.546x | -45.4% |
| feature-files | 5000 | 348.4ms | 186.8ms | 0.536x | -46.4% |
| feature-files | 10000 | 719.7ms | 387.3ms | 0.538x | -46.2% |
| feature-files | 25000 | 2.411s | 1.254s | 0.520x | -48.0% |

## Focused Read Rows

| Layout | Operation | Rows | Mean New/Baseline |
|---|---|---:|---:|
| city-json | bbox_query | 16 | 1.003x |
| city-json | get | 12 | 1.051x |
| city-json | read_package | 4 | 0.996x |
| city-json-seq | bbox_query | 16 | 0.993x |
| city-json-seq | get | 12 | 1.018x |
| city-json-seq | read_package | 4 | 0.993x |
| feature-files | bbox_query | 16 | 0.992x |
| feature-files | get | 12 | 1.038x |
| feature-files | read_package | 4 | 0.982x |

## Interpretation

- `index_reindex` improved across all layouts because scan-time normalization removes rebuild-time feature byte rereads, JSON reparses, and repeated ad-hoc statement preparation.
- CityJSONSeq and feature-files benefit the most: their reindex rows average close to half of the previous optimized implementation.
- Regular CityJSON reindex also improves materially, while read-package and bbox-query timings stay roughly unchanged because this pass targets indexing, not reconstruction.
- Small `dataset_open`, `get`, and reference-iteration differences are not the target of this optimization and should be treated as noise unless repeated runs reproduce them.
