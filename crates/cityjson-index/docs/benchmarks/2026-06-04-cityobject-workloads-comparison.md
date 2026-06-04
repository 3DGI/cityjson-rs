# CityObject Workload Benchmark Comparison

Compares the previous implementation at `7ba8541` with the current CityObject lookup, paging, and batch-read implementation on the same release-mode subset benchmark scope across all three storage layouts.

## Inputs

- Previous JSON: [`2026-06-04-previous-7ba8541-cityobject-workloads.json`](2026-06-04-previous-7ba8541-cityobject-workloads.json)
- Current JSON: [`2026-06-04-current-cityobject-workloads.json`](2026-06-04-current-cityobject-workloads.json)
- Previous implementation commit: `7ba8541`
- Benchmark case: `single-tile-subsets`
- Layouts: `city-json`, `city-json-seq`, `feature-files`
- Workers: `1`
- Release mode: yes

## Commands

Previous was run from a detached worktree at `7ba8541`:

```bash
just bench-index-json --case single-tile-subsets --layout city-json --layout city-json-seq --layout feature-files --workers 1 \
  > docs/benchmarks/2026-06-04-previous-7ba8541-cityobject-workloads.json
```

Current was run from the optimized checkout:

```bash
just bench-index-json --case single-tile-subsets --layout city-json --layout city-json-seq --layout feature-files --workers 1 \
  > docs/benchmarks/2026-06-04-current-cityobject-workloads.json
```

## Method

Matched rows use layout, subset size, worker count, operation, and variant. Ratios are `current / previous`, so values below `1.0x` mean the current implementation was faster. New CityObject-specific benchmark rows are current-only because the previous public API did not include batch CityObject lookup or CityObject keyset paging.

## Matched Summary

- Matched rows: `132`.
- Current faster rows: `67/132`.
- Current slower rows: `65/132`.
- Mean current/previous elapsed-time ratio: `0.974x`.

## Layout Summary

| Layout | Rows | Mean Current/Previous | Current Faster Rows |
|---|---:|---:|---:|
| city-json | 44 | 0.949x | 28 |
| city-json-seq | 44 | 0.968x | 20 |
| feature-files | 44 | 1.004x | 19 |

## Operation Summary

| Operation | Rows | Mean Current/Previous | Current Faster Rows |
|---|---:|---:|---:|
| bbox_query | 48 | 0.875x | 43 |
| dataset_open | 12 | 0.949x | 9 |
| full_scan_reference_iteration | 12 | 1.012x | 2 |
| get | 36 | 1.073x | 4 |
| index_reindex | 12 | 1.002x | 9 |
| read_package | 12 | 1.027x | 0 |

## Focused Matched Rows

| Layout | Subset | Operation | Variant | Previous | Current | Current/Previous | Hits |
|---|---:|---|---|---:|---:|---:|---:|
| city-json | 1000 | bbox_query | full | 83.8ms | 71.9ms | 0.858x | 1000 |
| city-json | 1000 | bbox_query | large | 21.8ms | 19.4ms | 0.893x | 196 |
| city-json | 1000 | bbox_query | medium | 1.1ms | 1.1ms | 0.933x | 8 |
| city-json | 1000 | bbox_query | small | 565.0us | 559.5us | 0.990x | 2 |
| city-json | 1000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 2.6ms | 2.7ms | 1.017x | 1 |
| city-json | 1000 | get | 01HP4TJYA9N9QWA43R8RFQ96F0 | 102.4us | 109.5us | 1.070x | 1 |
| city-json | 1000 | get | 01HP51HJ28PJ6RB13DFTHWZ6XA | 59.6us | 62.6us | 1.050x | 1 |
| city-json | 1000 | index_reindex | - | 97.8ms | 94.8ms | 0.969x | - |
| city-json | 1000 | read_package | sample-256 | 27.8ms | 28.3ms | 1.015x | 256 |
| city-json | 5000 | bbox_query | full | 240.7ms | 178.5ms | 0.742x | 5000 |
| city-json | 5000 | bbox_query | large | 93.8ms | 67.5ms | 0.719x | 2219 |
| city-json | 5000 | bbox_query | medium | 6.0ms | 4.2ms | 0.691x | 155 |
| city-json | 5000 | bbox_query | small | 625.4us | 615.1us | 0.983x | 3 |
| city-json | 5000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 4.1ms | 4.1ms | 1.000x | 1 |
| city-json | 5000 | get | 01HP51HM5HB60XT2MVB220RTJC | 92.2us | 102.7us | 1.115x | 1 |
| city-json | 5000 | get | 01HP51HQJM7R1DVJJYTAPWGF71 | 59.6us | 63.1us | 1.058x | 1 |
| city-json | 5000 | index_reindex | - | 222.0ms | 210.9ms | 0.950x | - |
| city-json | 5000 | read_package | sample-256 | 28.2ms | 28.4ms | 1.007x | 256 |
| city-json | 10000 | bbox_query | full | 542.3ms | 418.0ms | 0.771x | 10000 |
| city-json | 10000 | bbox_query | large | 260.4ms | 195.2ms | 0.750x | 5188 |
| city-json | 10000 | bbox_query | medium | 9.5ms | 6.6ms | 0.691x | 233 |
| city-json | 10000 | bbox_query | small | 859.0us | 781.2us | 0.909x | 8 |
| city-json | 10000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 8.3ms | 8.3ms | 0.995x | 1 |
| city-json | 10000 | get | 01HP51HQJP4QWNSVD4A92G2ZRJ | 124.1us | 139.5us | 1.124x | 1 |
| city-json | 10000 | get | 01HP5C61FVCDE1SNEP2F7YDJ7Y | 84.0us | 87.3us | 1.040x | 1 |
| city-json | 10000 | index_reindex | - | 454.2ms | 446.7ms | 0.984x | - |
| city-json | 10000 | read_package | sample-256 | 28.3ms | 28.7ms | 1.013x | 256 |
| city-json | 25000 | bbox_query | full | 2.007s | 1.704s | 0.849x | 25000 |
| city-json | 25000 | bbox_query | large | 1.018s | 844.1ms | 0.829x | 14186 |
| city-json | 25000 | bbox_query | medium | 42.0ms | 33.1ms | 0.789x | 690 |
| city-json | 25000 | bbox_query | small | 2.5ms | 2.3ms | 0.923x | 16 |
| city-json | 25000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 32.9ms | 32.4ms | 0.985x | 1 |
| city-json | 25000 | get | 01HP5C641S6KXAJS3ETXSCA025 | 196.0us | 207.6us | 1.059x | 1 |
| city-json | 25000 | get | 01HP91VX9EH4KBG5MP1REBVEAB | 83.3us | 90.3us | 1.084x | 1 |
| city-json | 25000 | index_reindex | - | 1.545s | 1.538s | 0.995x | - |
| city-json | 25000 | read_package | sample-256 | 28.1ms | 28.7ms | 1.020x | 256 |
| city-json-seq | 1000 | bbox_query | full | 32.8ms | 27.8ms | 0.849x | 1000 |
| city-json-seq | 1000 | bbox_query | large | 7.8ms | 6.8ms | 0.875x | 196 |
| city-json-seq | 1000 | bbox_query | medium | 393.4us | 369.1us | 0.938x | 8 |
| city-json-seq | 1000 | bbox_query | small | 208.0us | 206.7us | 0.994x | 2 |
| city-json-seq | 1000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 106.0us | 118.4us | 1.117x | 1 |
| city-json-seq | 1000 | get | 01HP4TJYA9N9QWA43R8RFQ96F0 | 52.3us | 55.5us | 1.060x | 1 |
| city-json-seq | 1000 | get | 01HP51HJ28PJ6RB13DFTHWZ6XA | 37.6us | 39.4us | 1.047x | 1 |
| city-json-seq | 1000 | index_reindex | - | 66.1ms | 73.3ms | 1.109x | - |
| city-json-seq | 1000 | read_package | sample-256 | 9.0ms | 9.4ms | 1.038x | 256 |
| city-json-seq | 5000 | bbox_query | full | 131.1ms | 104.8ms | 0.799x | 5000 |
| city-json-seq | 5000 | bbox_query | large | 48.9ms | 37.6ms | 0.769x | 2219 |
| city-json-seq | 5000 | bbox_query | medium | 3.0ms | 2.1ms | 0.702x | 155 |
| city-json-seq | 5000 | bbox_query | small | 229.4us | 248.8us | 1.085x | 3 |
| city-json-seq | 5000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 125.1us | 121.4us | 0.970x | 1 |
| city-json-seq | 5000 | get | 01HP51HM5HB60XT2MVB220RTJC | 51.4us | 54.5us | 1.061x | 1 |
| city-json-seq | 5000 | get | 01HP51HQJM7R1DVJJYTAPWGF71 | 40.1us | 41.9us | 1.046x | 1 |
| city-json-seq | 5000 | index_reindex | - | 167.0ms | 165.6ms | 0.991x | - |
| city-json-seq | 5000 | read_package | sample-256 | 9.2ms | 9.6ms | 1.039x | 256 |
| city-json-seq | 10000 | bbox_query | full | 310.0ms | 253.7ms | 0.818x | 10000 |
| city-json-seq | 10000 | bbox_query | large | 142.8ms | 113.5ms | 0.795x | 5188 |
| city-json-seq | 10000 | bbox_query | medium | 4.7ms | 3.3ms | 0.705x | 233 |
| city-json-seq | 10000 | bbox_query | small | 367.4us | 329.6us | 0.897x | 8 |
| city-json-seq | 10000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 125.4us | 139.3us | 1.110x | 1 |
| city-json-seq | 10000 | get | 01HP51HQJP4QWNSVD4A92G2ZRJ | 66.6us | 58.2us | 0.874x | 1 |
| city-json-seq | 10000 | get | 01HP5C61FVCDE1SNEP2F7YDJ7Y | 47.0us | 49.1us | 1.045x | 1 |
| city-json-seq | 10000 | index_reindex | - | 330.6ms | 326.3ms | 0.987x | - |
| city-json-seq | 10000 | read_package | sample-256 | 9.3ms | 9.5ms | 1.027x | 256 |
| city-json-seq | 25000 | bbox_query | full | 1.171s | 1.034s | 0.883x | 25000 |
| city-json-seq | 25000 | bbox_query | large | 552.5ms | 473.5ms | 0.857x | 14186 |
| city-json-seq | 25000 | bbox_query | medium | 18.5ms | 14.8ms | 0.800x | 690 |
| city-json-seq | 25000 | bbox_query | small | 844.6us | 829.4us | 0.982x | 16 |
| city-json-seq | 25000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 137.3us | 138.1us | 1.005x | 1 |
| city-json-seq | 25000 | get | 01HP5C641S6KXAJS3ETXSCA025 | 62.1us | 64.9us | 1.045x | 1 |
| city-json-seq | 25000 | get | 01HP91VX9EH4KBG5MP1REBVEAB | 45.6us | 48.2us | 1.058x | 1 |
| city-json-seq | 25000 | index_reindex | - | 1.075s | 1.087s | 1.011x | - |
| city-json-seq | 25000 | read_package | sample-256 | 9.2ms | 9.6ms | 1.037x | 256 |
| feature-files | 1000 | bbox_query | full | 33.8ms | 29.9ms | 0.887x | 1000 |
| feature-files | 1000 | bbox_query | large | 7.9ms | 7.2ms | 0.917x | 196 |
| feature-files | 1000 | bbox_query | medium | 401.2us | 395.1us | 0.985x | 8 |
| feature-files | 1000 | bbox_query | small | 206.4us | 213.7us | 1.035x | 2 |
| feature-files | 1000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 109.2us | 115.3us | 1.056x | 1 |
| feature-files | 1000 | get | 01HP4TJYA9N9QWA43R8RFQ96F0 | 52.6us | 56.0us | 1.064x | 1 |
| feature-files | 1000 | get | 01HP51HJ28PJ6RB13DFTHWZ6XA | 38.3us | 40.2us | 1.049x | 1 |
| feature-files | 1000 | index_reindex | - | 71.4ms | 71.3ms | 1.000x | - |
| feature-files | 1000 | read_package | sample-256 | 9.2ms | 9.5ms | 1.027x | 256 |
| feature-files | 5000 | bbox_query | full | 133.2ms | 118.2ms | 0.887x | 5000 |
| feature-files | 5000 | bbox_query | large | 50.2ms | 46.3ms | 0.922x | 2219 |
| feature-files | 5000 | bbox_query | medium | 3.1ms | 2.8ms | 0.912x | 155 |
| feature-files | 5000 | bbox_query | small | 250.4us | 287.2us | 1.147x | 3 |
| feature-files | 5000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 109.4us | 153.8us | 1.406x | 1 |
| feature-files | 5000 | get | 01HP51HM5HB60XT2MVB220RTJC | 52.4us | 61.1us | 1.167x | 1 |
| feature-files | 5000 | get | 01HP51HQJM7R1DVJJYTAPWGF71 | 40.2us | 45.8us | 1.137x | 1 |
| feature-files | 5000 | index_reindex | - | 183.4ms | 193.8ms | 1.056x | - |
| feature-files | 5000 | read_package | sample-256 | 9.5ms | 9.9ms | 1.042x | 256 |
| feature-files | 10000 | bbox_query | full | 312.4ms | 276.3ms | 0.884x | 10000 |
| feature-files | 10000 | bbox_query | large | 144.4ms | 125.7ms | 0.871x | 5188 |
| feature-files | 10000 | bbox_query | medium | 4.8ms | 3.8ms | 0.795x | 233 |
| feature-files | 10000 | bbox_query | small | 341.3us | 351.3us | 1.029x | 8 |
| feature-files | 10000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 124.3us | 133.5us | 1.074x | 1 |
| feature-files | 10000 | get | 01HP51HQJP4QWNSVD4A92G2ZRJ | 54.7us | 59.0us | 1.077x | 1 |
| feature-files | 10000 | get | 01HP5C61FVCDE1SNEP2F7YDJ7Y | 45.4us | 49.4us | 1.087x | 1 |
| feature-files | 10000 | index_reindex | - | 396.9ms | 388.1ms | 0.978x | - |
| feature-files | 10000 | read_package | sample-256 | 9.4ms | 9.7ms | 1.031x | 256 |
| feature-files | 25000 | bbox_query | full | 1.186s | 1.092s | 0.921x | 25000 |
| feature-files | 25000 | bbox_query | large | 559.6ms | 507.2ms | 0.906x | 14186 |
| feature-files | 25000 | bbox_query | medium | 19.2ms | 16.0ms | 0.836x | 690 |
| feature-files | 25000 | bbox_query | small | 842.2us | 847.8us | 1.007x | 16 |
| feature-files | 25000 | get | 01HP4Q2S5R55GJ0EK96XKHSB7J | 138.0us | 153.8us | 1.115x | 1 |
| feature-files | 25000 | get | 01HP5C641S6KXAJS3ETXSCA025 | 60.1us | 71.1us | 1.183x | 1 |
| feature-files | 25000 | get | 01HP91VX9EH4KBG5MP1REBVEAB | 44.9us | 53.5us | 1.191x | 1 |
| feature-files | 25000 | index_reindex | - | 1.256s | 1.253s | 0.998x | - |
| feature-files | 25000 | read_package | sample-256 | 9.5ms | 9.7ms | 1.030x | 256 |

## Current-Only CityObject Workload Rows

| Operation | Rows | Mean Time | Min Time | Max Time | Total Hits |
|---|---:|---:|---:|---:|---:|
| cityobject_bbox_query | 48 | 27.2ms | 36.9us | 266.8ms | 191712 |
| cityobject_full_scan_reference_iteration | 12 | 11.1ms | 634.7us | 32.1ms | 123000 |
| cityobject_id_lookup | 12 | 311.0us | 265.5us | 374.2us | 3072 |
| package_bbox_lookup_only | 48 | 49.1ms | 39.8us | 491.8ms | 191712 |
| read_packages | 12 | 14.2ms | 8.5ms | 25.0ms | 3072 |

## Current-Only Focused Rows

| Layout | Subset | Operation | Variant | Current | Hits |
|---|---:|---|---|---:|---:|
| city-json | 1000 | cityobject_bbox_query | full | 961.8us | 1000 |
| city-json | 1000 | cityobject_bbox_query | large | 164.7us | 196 |
| city-json | 1000 | cityobject_bbox_query | medium | 37.1us | 8 |
| city-json | 1000 | cityobject_bbox_query | small | 41.9us | 2 |
| city-json | 5000 | cityobject_bbox_query | full | 11.7ms | 5000 |
| city-json | 5000 | cityobject_bbox_query | large | 3.5ms | 2219 |
| city-json | 5000 | cityobject_bbox_query | medium | 147.7us | 155 |
| city-json | 5000 | cityobject_bbox_query | small | 56.5us | 3 |
| city-json | 10000 | cityobject_bbox_query | full | 41.1ms | 10000 |
| city-json | 10000 | cityobject_bbox_query | large | 14.1ms | 5188 |
| city-json | 10000 | cityobject_bbox_query | medium | 243.9us | 233 |
| city-json | 10000 | cityobject_bbox_query | small | 68.3us | 8 |
| city-json | 25000 | cityobject_bbox_query | full | 259.0ms | 25000 |
| city-json | 25000 | cityobject_bbox_query | large | 95.2ms | 14186 |
| city-json | 25000 | cityobject_bbox_query | medium | 812.6us | 690 |
| city-json | 25000 | cityobject_bbox_query | small | 86.1us | 16 |
| city-json-seq | 1000 | cityobject_bbox_query | full | 951.2us | 1000 |
| city-json-seq | 1000 | cityobject_bbox_query | large | 160.2us | 196 |
| city-json-seq | 1000 | cityobject_bbox_query | medium | 36.9us | 8 |
| city-json-seq | 1000 | cityobject_bbox_query | small | 39.2us | 2 |
| city-json-seq | 5000 | cityobject_bbox_query | full | 11.8ms | 5000 |
| city-json-seq | 5000 | cityobject_bbox_query | large | 3.5ms | 2219 |
| city-json-seq | 5000 | cityobject_bbox_query | medium | 145.3us | 155 |
| city-json-seq | 5000 | cityobject_bbox_query | small | 56.9us | 3 |
| city-json-seq | 10000 | cityobject_bbox_query | full | 41.3ms | 10000 |
| city-json-seq | 10000 | cityobject_bbox_query | large | 14.1ms | 5188 |
| city-json-seq | 10000 | cityobject_bbox_query | medium | 231.0us | 233 |
| city-json-seq | 10000 | cityobject_bbox_query | small | 67.0us | 8 |
| city-json-seq | 25000 | cityobject_bbox_query | full | 264.5ms | 25000 |
| city-json-seq | 25000 | cityobject_bbox_query | large | 96.9ms | 14186 |
| city-json-seq | 25000 | cityobject_bbox_query | medium | 831.4us | 690 |
| city-json-seq | 25000 | cityobject_bbox_query | small | 81.1us | 16 |
| feature-files | 1000 | cityobject_bbox_query | full | 978.0us | 1000 |
| feature-files | 1000 | cityobject_bbox_query | large | 168.3us | 196 |
| feature-files | 1000 | cityobject_bbox_query | medium | 37.7us | 8 |
| feature-files | 1000 | cityobject_bbox_query | small | 44.1us | 2 |
| feature-files | 5000 | cityobject_bbox_query | full | 12.8ms | 5000 |
| feature-files | 5000 | cityobject_bbox_query | large | 3.8ms | 2219 |
| feature-files | 5000 | cityobject_bbox_query | medium | 165.5us | 155 |
| feature-files | 5000 | cityobject_bbox_query | small | 55.2us | 3 |
| feature-files | 10000 | cityobject_bbox_query | full | 44.9ms | 10000 |
| feature-files | 10000 | cityobject_bbox_query | large | 15.4ms | 5188 |
| feature-files | 10000 | cityobject_bbox_query | medium | 242.4us | 233 |
| feature-files | 10000 | cityobject_bbox_query | small | 68.3us | 8 |
| feature-files | 25000 | cityobject_bbox_query | full | 266.8ms | 25000 |
| feature-files | 25000 | cityobject_bbox_query | large | 97.6ms | 14186 |
| feature-files | 25000 | cityobject_bbox_query | medium | 832.9us | 690 |
| feature-files | 25000 | cityobject_bbox_query | small | 91.6us | 16 |
| city-json | 1000 | cityobject_full_scan_reference_iteration | - | 643.6us | 1000 |
| city-json | 5000 | cityobject_full_scan_reference_iteration | - | 3.6ms | 5000 |
| city-json | 10000 | cityobject_full_scan_reference_iteration | - | 8.3ms | 10000 |
| city-json | 25000 | cityobject_full_scan_reference_iteration | - | 30.7ms | 25000 |
| city-json-seq | 1000 | cityobject_full_scan_reference_iteration | - | 634.7us | 1000 |
| city-json-seq | 5000 | cityobject_full_scan_reference_iteration | - | 3.6ms | 5000 |
| city-json-seq | 10000 | cityobject_full_scan_reference_iteration | - | 8.3ms | 10000 |
| city-json-seq | 25000 | cityobject_full_scan_reference_iteration | - | 31.3ms | 25000 |
| feature-files | 1000 | cityobject_full_scan_reference_iteration | - | 644.3us | 1000 |
| feature-files | 5000 | cityobject_full_scan_reference_iteration | - | 4.3ms | 5000 |
| feature-files | 10000 | cityobject_full_scan_reference_iteration | - | 8.7ms | 10000 |
| feature-files | 25000 | cityobject_full_scan_reference_iteration | - | 32.1ms | 25000 |
| city-json | 1000 | cityobject_id_lookup | sample-256 | 276.9us | 256 |
| city-json | 5000 | cityobject_id_lookup | sample-256 | 291.9us | 256 |
| city-json | 10000 | cityobject_id_lookup | sample-256 | 338.7us | 256 |
| city-json | 25000 | cityobject_id_lookup | sample-256 | 374.2us | 256 |
| city-json-seq | 1000 | cityobject_id_lookup | sample-256 | 265.5us | 256 |
| city-json-seq | 5000 | cityobject_id_lookup | sample-256 | 280.5us | 256 |
| city-json-seq | 10000 | cityobject_id_lookup | sample-256 | 309.7us | 256 |
| city-json-seq | 25000 | cityobject_id_lookup | sample-256 | 325.5us | 256 |
| feature-files | 1000 | cityobject_id_lookup | sample-256 | 266.7us | 256 |
| feature-files | 5000 | cityobject_id_lookup | sample-256 | 323.2us | 256 |
| feature-files | 10000 | cityobject_id_lookup | sample-256 | 318.8us | 256 |
| feature-files | 25000 | cityobject_id_lookup | sample-256 | 360.9us | 256 |
| city-json | 1000 | package_bbox_lookup_only | full | 1.4ms | 1000 |
| city-json | 1000 | package_bbox_lookup_only | large | 209.7us | 196 |
| city-json | 1000 | package_bbox_lookup_only | medium | 47.0us | 8 |
| city-json | 1000 | package_bbox_lookup_only | small | 60.0us | 2 |
| city-json | 5000 | package_bbox_lookup_only | full | 19.6ms | 5000 |
| city-json | 5000 | package_bbox_lookup_only | large | 5.2ms | 2219 |
| city-json | 5000 | package_bbox_lookup_only | medium | 175.6us | 155 |
| city-json | 5000 | package_bbox_lookup_only | small | 70.5us | 3 |
| city-json | 10000 | package_bbox_lookup_only | full | 77.5ms | 10000 |
| city-json | 10000 | package_bbox_lookup_only | large | 24.1ms | 5188 |
| city-json | 10000 | package_bbox_lookup_only | medium | 322.6us | 233 |
| city-json | 10000 | package_bbox_lookup_only | small | 83.6us | 8 |
| city-json | 25000 | package_bbox_lookup_only | full | 482.0ms | 25000 |
| city-json | 25000 | package_bbox_lookup_only | large | 164.5ms | 14186 |
| city-json | 25000 | package_bbox_lookup_only | medium | 1.1ms | 690 |
| city-json | 25000 | package_bbox_lookup_only | small | 103.2us | 16 |
| city-json-seq | 1000 | package_bbox_lookup_only | full | 1.4ms | 1000 |
| city-json-seq | 1000 | package_bbox_lookup_only | large | 210.8us | 196 |
| city-json-seq | 1000 | package_bbox_lookup_only | medium | 40.1us | 8 |
| city-json-seq | 1000 | package_bbox_lookup_only | small | 57.1us | 2 |
| city-json-seq | 5000 | package_bbox_lookup_only | full | 19.7ms | 5000 |
| city-json-seq | 5000 | package_bbox_lookup_only | large | 5.2ms | 2219 |
| city-json-seq | 5000 | package_bbox_lookup_only | medium | 173.7us | 155 |
| city-json-seq | 5000 | package_bbox_lookup_only | small | 66.0us | 3 |
| city-json-seq | 10000 | package_bbox_lookup_only | full | 77.5ms | 10000 |
| city-json-seq | 10000 | package_bbox_lookup_only | large | 24.1ms | 5188 |
| city-json-seq | 10000 | package_bbox_lookup_only | medium | 299.7us | 233 |
| city-json-seq | 10000 | package_bbox_lookup_only | small | 73.8us | 8 |
| city-json-seq | 25000 | package_bbox_lookup_only | full | 491.8ms | 25000 |
| city-json-seq | 25000 | package_bbox_lookup_only | large | 166.9ms | 14186 |
| city-json-seq | 25000 | package_bbox_lookup_only | medium | 1.0ms | 690 |
| city-json-seq | 25000 | package_bbox_lookup_only | small | 84.6us | 16 |
| feature-files | 1000 | package_bbox_lookup_only | full | 1.4ms | 1000 |
| feature-files | 1000 | package_bbox_lookup_only | large | 214.0us | 196 |
| feature-files | 1000 | package_bbox_lookup_only | medium | 39.8us | 8 |
| feature-files | 1000 | package_bbox_lookup_only | small | 58.1us | 2 |
| feature-files | 5000 | package_bbox_lookup_only | full | 21.1ms | 5000 |
| feature-files | 5000 | package_bbox_lookup_only | large | 5.6ms | 2219 |
| feature-files | 5000 | package_bbox_lookup_only | medium | 218.9us | 155 |
| feature-files | 5000 | package_bbox_lookup_only | small | 74.2us | 3 |
| feature-files | 10000 | package_bbox_lookup_only | full | 78.5ms | 10000 |
| feature-files | 10000 | package_bbox_lookup_only | large | 24.4ms | 5188 |
| feature-files | 10000 | package_bbox_lookup_only | medium | 302.0us | 233 |
| feature-files | 10000 | package_bbox_lookup_only | small | 72.8us | 8 |
| feature-files | 25000 | package_bbox_lookup_only | full | 490.6ms | 25000 |
| feature-files | 25000 | package_bbox_lookup_only | large | 166.6ms | 14186 |
| feature-files | 25000 | package_bbox_lookup_only | medium | 1.0ms | 690 |
| feature-files | 25000 | package_bbox_lookup_only | small | 87.3us | 16 |
| city-json | 1000 | read_packages | sample-256 | 24.7ms | 256 |
| city-json | 5000 | read_packages | sample-256 | 24.8ms | 256 |
| city-json | 10000 | read_packages | sample-256 | 24.9ms | 256 |
| city-json | 25000 | read_packages | sample-256 | 25.0ms | 256 |
| city-json-seq | 1000 | read_packages | sample-256 | 8.5ms | 256 |
| city-json-seq | 5000 | read_packages | sample-256 | 8.8ms | 256 |
| city-json-seq | 10000 | read_packages | sample-256 | 8.6ms | 256 |
| city-json-seq | 25000 | read_packages | sample-256 | 8.6ms | 256 |
| feature-files | 1000 | read_packages | sample-256 | 8.8ms | 256 |
| feature-files | 5000 | read_packages | sample-256 | 9.2ms | 256 |
| feature-files | 10000 | read_packages | sample-256 | 9.0ms | 256 |
| feature-files | 25000 | read_packages | sample-256 | 9.0ms | 256 |

## Interpretation

- Matched `bbox_query` rows average `0.875x` of the previous implementation (`43/48` faster).
- Matched `get` rows average `1.073x` of the previous implementation (`4/36` faster).
- Matched `read_package` rows average `1.027x` of the previous implementation (`0/12` faster).
- Matched `index_reindex` rows average `1.002x` of the previous implementation (`9/12` faster).
- Matched `full_scan_reference_iteration` rows average `1.012x` of the previous implementation (`2/12` faster).
- Matched `dataset_open` rows average `0.949x` of the previous implementation (`9/12` faster).
- The new `cityobject_id_lookup`, `cityobject_bbox_query`, and `cityobject_full_scan_reference_iteration` rows establish baselines for the primary CityObject-ref workflows. They are not directly ratio-comparable to `7ba8541` because those public batch/page APIs did not exist there.
- The new `package_bbox_lookup_only` rows separate SQLite spatial lookup cost from package reconstruction cost, while existing `bbox_query` rows continue to measure lookup plus decode.
- The new `read_packages sample-256` rows measure the actual batch API separately from the existing scalar `read_package sample-256` loop.

