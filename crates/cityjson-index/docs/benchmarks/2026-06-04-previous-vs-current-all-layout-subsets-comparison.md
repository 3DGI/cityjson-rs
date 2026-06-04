# Previous Vs Current All-Layout Subset Benchmark Comparison

Compares the previous implementation at `1f156f0` with the current optimized implementation on the same release-mode subset benchmark scope across all three storage layouts.

## Inputs

- Previous JSON: [`2026-06-04-previous-1f156f0-subsets-all-layouts.json`](2026-06-04-previous-1f156f0-subsets-all-layouts.json)
- Current JSON: [`2026-06-04-current-optimized-subsets-all-layouts.json`](2026-06-04-current-optimized-subsets-all-layouts.json)
- Previous implementation commit: `1f156f085a6fa3f2440ca6092062f40d1efa49fc`
- Current code commit for measured implementation: `d3030585436f4b1377a36f1633b120506650bd79`
- Benchmark case: `single-tile-subsets`
- Layouts: `city-json`, `city-json-seq`, `feature-files`
- Workers: `1`
- Release mode: yes

## Commands

Previous was run from a temporary worktree at `1f156f0`. The benchmark-preparation-only patch from `d303058` was applied without committing so the old harness could materialize CityJSONSeq and feature-file datasets in tractable time. The measured index and reconstruction code remained the previous implementation.

```bash
cargo run --release -p cityjson-index --bin bench-index --target-dir target -- --json \
  --case single-tile-subsets \
  --layout city-json --layout city-json-seq --layout feature-files \
  --workers 1 \
  > /tmp/cityjson-index-prev-1f156f0-subsets-all-layouts.json
```

Current was run from the optimized checkout. `city-json` and `city-json-seq` were captured together; `feature-files` was captured separately and merged into the stored current JSON.

```bash
just bench-index-json --case single-tile-subsets --layout city-json --layout city-json-seq --workers 1 \
  > /tmp/cityjson-index-release-subsets-fixed.json
just bench-index-json --case single-tile-subsets --layout feature-files --workers 1 \
  > /tmp/cityjson-index-current-feature-files-subsets.json
```

## Method

Rows are matched by layout, subset size, worker count, operation, and variant. Ratios are `current / previous`, so values below `1.0x` mean the current optimized implementation was faster.

## Summary

- Matched rows: `132`.
- Current faster rows: `70/132`.
- Current slower rows: `62/132`.
- Mean current/previous elapsed-time ratio: `0.761x`.

## Layout Summary

| Layout | Rows | Mean Current/Previous | Current Faster Rows | Current Slower Rows |
|---|---:|---:|---:|---:|
| city-json | 44 | 1.035x | 4 | 40 |
| city-json-seq | 44 | 0.631x | 32 | 12 |
| feature-files | 44 | 0.618x | 34 | 10 |

## Operation Summary

| Operation | Rows | Mean Current/Previous | Current Faster Rows |
|---|---:|---:|---:|
| bbox_query full | 12 | 0.634x | 8 |
| bbox_query large | 12 | 0.619x | 8 |
| bbox_query medium | 12 | 0.588x | 8 |
| bbox_query small | 12 | 0.538x | 8 |
| dataset_open | 12 | 1.091x | 0 |
| full_scan_reference_iteration | 12 | 1.063x | 2 |
| get 01HP4Q2S5R55GJ0EK96XKHSB7J | 12 | 0.818x | 9 |
| get 01HP4TJYA9N9QWA43R8RFQ96F0 | 3 | 0.703x | 2 |
| get 01HP51HJ28PJ6RB13DFTHWZ6XA | 3 | 0.747x | 2 |
| get 01HP51HM5HB60XT2MVB220RTJC | 3 | 0.793x | 2 |
| get 01HP51HQJM7R1DVJJYTAPWGF71 | 3 | 0.763x | 3 |
| get 01HP51HQJP4QWNSVD4A92G2ZRJ | 3 | 0.772x | 2 |
| get 01HP5C61FVCDE1SNEP2F7YDJ7Y | 3 | 0.688x | 2 |
| get 01HP5C641S6KXAJS3ETXSCA025 | 3 | 0.729x | 2 |
| get 01HP91VX9EH4KBG5MP1REBVEAB | 3 | 0.763x | 2 |
| index_reindex | 12 | 1.027x | 0 |
| read_package sample-256 | 12 | 0.506x | 10 |

## Layout And Operation Summary

| Layout | Operation | Rows | Mean Current/Previous |
|---|---|---:|---:|
| city-json | bbox_query full | 4 | 1.013x |
| city-json | index_reindex | 4 | 1.008x |
| city-json | read_package sample-256 | 4 | 1.000x |
| city-json-seq | bbox_query full | 4 | 0.443x |
| city-json-seq | index_reindex | 4 | 1.027x |
| city-json-seq | read_package sample-256 | 4 | 0.258x |
| feature-files | bbox_query full | 4 | 0.446x |
| feature-files | index_reindex | 4 | 1.046x |
| feature-files | read_package sample-256 | 4 | 0.260x |

## Focused Rows

| Layout | Subset | Operation | Previous | Current | Current/Previous | Delta | Hits |
|---|---:|---|---:|---:|---:|---:|---:|
| city-json | 1000 | bbox_query full | 83.8ms | 84.3ms | 1.005x | +0.5% | 1000 |
| city-json | 1000 | index_reindex | 123.8ms | 123.8ms | 1.000x | +0.0% | - |
| city-json | 1000 | read_package sample-256 | 28.2ms | 28.1ms | 0.998x | -0.2% | 256 |
| city-json | 5000 | bbox_query full | 238.9ms | 241.3ms | 1.010x | +1.0% | 5000 |
| city-json | 5000 | index_reindex | 284.8ms | 288.1ms | 1.011x | +1.1% | - |
| city-json | 5000 | read_package sample-256 | 28.3ms | 28.4ms | 1.002x | +0.2% | 256 |
| city-json | 10000 | bbox_query full | 532.6ms | 542.8ms | 1.019x | +1.9% | 10000 |
| city-json | 10000 | index_reindex | 612.4ms | 623.0ms | 1.017x | +1.7% | - |
| city-json | 10000 | read_package sample-256 | 28.4ms | 28.2ms | 0.993x | -0.7% | 256 |
| city-json | 25000 | bbox_query full | 1.985s | 2.021s | 1.018x | +1.8% | 25000 |
| city-json | 25000 | index_reindex | 2.081s | 2.087s | 1.003x | +0.3% | - |
| city-json | 25000 | read_package sample-256 | 28.4ms | 28.5ms | 1.005x | +0.5% | 256 |
| city-json-seq | 1000 | bbox_query full | 108.6ms | 33.6ms | 0.309x | -69.1% | 1000 |
| city-json-seq | 1000 | index_reindex | 125.3ms | 127.3ms | 1.016x | +1.6% | - |
| city-json-seq | 1000 | read_package sample-256 | 36.3ms | 9.3ms | 0.256x | -74.4% | 256 |
| city-json-seq | 5000 | bbox_query full | 285.1ms | 133.5ms | 0.468x | -53.2% | 5000 |
| city-json-seq | 5000 | index_reindex | 291.0ms | 299.8ms | 1.030x | +3.0% | - |
| city-json-seq | 5000 | read_package sample-256 | 36.8ms | 9.4ms | 0.256x | -74.4% | 256 |
| city-json-seq | 10000 | bbox_query full | 634.8ms | 312.6ms | 0.492x | -50.8% | 10000 |
| city-json-seq | 10000 | index_reindex | 613.7ms | 633.1ms | 1.032x | +3.2% | - |
| city-json-seq | 10000 | read_package sample-256 | 36.8ms | 9.5ms | 0.259x | -74.1% | 256 |
| city-json-seq | 25000 | bbox_query full | 2.362s | 1.183s | 0.501x | -49.9% | 25000 |
| city-json-seq | 25000 | index_reindex | 2.082s | 2.146s | 1.030x | +3.0% | - |
| city-json-seq | 25000 | read_package sample-256 | 36.6ms | 9.5ms | 0.261x | -73.9% | 256 |
| feature-files | 1000 | bbox_query full | 109.9ms | 34.3ms | 0.312x | -68.8% | 1000 |
| feature-files | 1000 | index_reindex | 131.8ms | 134.5ms | 1.020x | +2.0% | - |
| feature-files | 1000 | read_package sample-256 | 36.7ms | 9.4ms | 0.257x | -74.3% | 256 |
| feature-files | 5000 | bbox_query full | 289.5ms | 135.7ms | 0.469x | -53.1% | 5000 |
| feature-files | 5000 | index_reindex | 328.5ms | 348.4ms | 1.061x | +6.1% | - |
| feature-files | 5000 | read_package sample-256 | 36.7ms | 9.6ms | 0.262x | -73.8% | 256 |
| feature-files | 10000 | bbox_query full | 645.0ms | 321.3ms | 0.498x | -50.2% | 10000 |
| feature-files | 10000 | index_reindex | 683.9ms | 719.7ms | 1.052x | +5.2% | - |
| feature-files | 10000 | read_package sample-256 | 37.0ms | 9.6ms | 0.260x | -74.0% | 256 |
| feature-files | 25000 | bbox_query full | 2.384s | 1.201s | 0.504x | -49.6% | 25000 |
| feature-files | 25000 | index_reindex | 2.291s | 2.411s | 1.052x | +5.2% | - |
| feature-files | 25000 | read_package sample-256 | 37.0ms | 9.7ms | 0.261x | -73.9% | 256 |

## Largest Improvements

| Layout | Subset | Operation | Previous | Current | Current/Previous | Delta |
|---|---:|---|---:|---:|---:|---:|
| city-json-seq | 5000 | read_package sample-256 | 36.8ms | 9.4ms | 0.256x | -74.4% |
| city-json-seq | 1000 | read_package sample-256 | 36.3ms | 9.3ms | 0.256x | -74.4% |
| feature-files | 1000 | read_package sample-256 | 36.7ms | 9.4ms | 0.257x | -74.3% |
| city-json-seq | 10000 | read_package sample-256 | 36.8ms | 9.5ms | 0.259x | -74.1% |
| feature-files | 10000 | read_package sample-256 | 37.0ms | 9.6ms | 0.260x | -74.0% |
| city-json-seq | 25000 | read_package sample-256 | 36.6ms | 9.5ms | 0.261x | -73.9% |
| feature-files | 25000 | read_package sample-256 | 37.0ms | 9.7ms | 0.261x | -73.9% |
| feature-files | 5000 | read_package sample-256 | 36.7ms | 9.6ms | 0.262x | -73.8% |
| feature-files | 1000 | bbox_query small | 0.8ms | 0.2ms | 0.263x | -73.7% |
| feature-files | 1000 | bbox_query medium | 1.5ms | 0.4ms | 0.267x | -73.3% |
| feature-files | 1000 | bbox_query large | 29.4ms | 8.0ms | 0.271x | -72.9% |
| feature-files | 25000 | bbox_query small | 3.1ms | 0.9ms | 0.276x | -72.4% |
| city-json-seq | 25000 | bbox_query small | 3.1ms | 0.9ms | 0.277x | -72.3% |
| city-json-seq | 1000 | bbox_query large | 28.4ms | 7.9ms | 0.278x | -72.2% |
| city-json-seq | 1000 | bbox_query medium | 1.5ms | 0.4ms | 0.279x | -72.1% |

## Largest Regressions

| Layout | Subset | Operation | Previous | Current | Current/Previous | Delta |
|---|---:|---|---:|---:|---:|---:|
| city-json | 1000 | full_scan_reference_iteration | 0.6ms | 1.0ms | 1.667x | +66.7% |
| feature-files | 25000 | dataset_open | 53.7ms | 65.3ms | 1.217x | +21.7% |
| city-json-seq | 10000 | dataset_open | 56.4ms | 67.4ms | 1.194x | +19.4% |
| feature-files | 10000 | dataset_open | 55.5ms | 63.3ms | 1.142x | +14.2% |
| feature-files | 1000 | dataset_open | 59.9ms | 66.8ms | 1.115x | +11.5% |
| city-json-seq | 1000 | dataset_open | 55.4ms | 60.9ms | 1.099x | +9.9% |
| city-json | 10000 | dataset_open | 58.6ms | 63.6ms | 1.085x | +8.5% |
| feature-files | 5000 | dataset_open | 56.0ms | 60.4ms | 1.078x | +7.8% |
| city-json | 25000 | dataset_open | 61.1ms | 65.6ms | 1.074x | +7.4% |
| city-json | 25000 | full_scan_reference_iteration | 28.8ms | 30.6ms | 1.062x | +6.2% |
| feature-files | 5000 | index_reindex | 328.5ms | 348.4ms | 1.061x | +6.1% |
| city-json | 25000 | get 01HP91VX9EH4KBG5MP1REBVEAB | 0.1ms | 0.1ms | 1.058x | +5.8% |
| feature-files | 10000 | index_reindex | 683.9ms | 719.7ms | 1.052x | +5.2% |
| feature-files | 25000 | index_reindex | 2.291s | 2.411s | 1.052x | +5.2% |
| feature-files | 25000 | full_scan_reference_iteration | 30.6ms | 32.2ms | 1.052x | +5.2% |

## Interpretation

- Regular `city-json` is effectively unchanged: focused reconstruction rows remain around the same `read_package sample-256` time, and the layout mean is near `1.0x`.
- `city-json-seq` improves materially on read/reconstruction paths because the current code avoids the local parse/mutate/serialize/reparse cycle. The focused `read_package sample-256` rows are about one third of the previous time.
- `feature-files` sees the same reconstruction-path improvement pattern as CityJSONSeq because it uses the same indexed-id staged feature-slice path.
- `index_reindex` is not improved by this optimization and remains roughly unchanged to slightly slower depending on layout, which is expected because the optimization targets package reconstruction after lookup, not indexing.
