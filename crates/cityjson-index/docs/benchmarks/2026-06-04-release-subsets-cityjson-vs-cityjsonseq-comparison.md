# Release Subset Layout Benchmark Comparison

Captured after fixing the direct staged reconstruction path and the CityJSONSeq benchmark materialization bottleneck.

## Environment

- Captured: `2026-06-04T01:42:12+02:00`
- OS: `Linux workstation 6.17.0-29-generic #29~24.04.1-Ubuntu SMP PREEMPT_DYNAMIC Mon May 11 10:30:58 UTC 2 x86_64 GNU/Linux`
- CPU: `AMD Ryzen 9 9900X 12-Core Processor`
- Logical CPUs: `24`
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`

## Inputs

- Commit: `d3030585436f4b1377a36f1633b120506650bd79`
- Result JSON: [`2026-06-04-release-subsets-cityjson-vs-cityjsonseq.json`](2026-06-04-release-subsets-cityjson-vs-cityjsonseq.json)
- Benchmark case: `single-tile-subsets`
- Layouts: `city-json`, `city-json-seq`
- Workers: `1`
- Runs: `88`

## Command

```bash
just bench-index-json --case single-tile-subsets --layout city-json --layout city-json-seq --workers 1 \
  > /tmp/cityjson-index-release-subsets-fixed.json
cp /tmp/cityjson-index-release-subsets-fixed.json \
  docs/benchmarks/2026-06-04-release-subsets-cityjson-vs-cityjsonseq.json
```

## Method

Each row in the comparison matches one `city-json` run and one `city-json-seq` run by subset size, worker count, operation, and variant. Ratios are `city-json-seq / city-json`, so values below `1.0x` mean CityJSONSeq was faster.

## Summary

- Compared `44` matched layout pairs.
- CityJSONSeq was faster in `36` pairs, slower in `8` pairs, and equal in `0` pairs.
- Mean CityJSONSeq/CityJSON elapsed-time ratio across matched pairs: `0.564x`.
- The previous non-finishing subset run was caused by benchmark preparation for CityJSONSeq, not by measured index operations: the harness cloned the full CityJSON document per generated feature before writing `.city.jsonl`.

## Operation Summary

| Operation | Pairs | Mean Seq/CityJSON Time | Seq Faster Pairs |
|---|---:|---:|---:|
| bbox_query | 16 | 0.464x | 16 |
| dataset_open | 4 | 0.975x | 3 |
| full_scan_reference_iteration | 4 | 0.905x | 1 |
| get | 12 | 0.367x | 12 |
| index_reindex | 4 | 1.028x | 0 |
| read_package | 4 | 0.334x | 4 |

## Focused Operations

| Subset | Operation | CityJSON | CityJSONSeq | Seq/CityJSON Time | Delta | Hits |
|---:|---|---:|---:|---:|---:|---:|
| 1000 | bbox_query full | 84.3ms | 33.6ms | 0.398x | -60.2% | 1000 |
| 1000 | index_reindex | 123.8ms | 127.3ms | 1.029x | +2.9% | - |
| 1000 | read_package sample-256 | 28.1ms | 9.3ms | 0.331x | -66.9% | 256 |
| 5000 | bbox_query full | 241.3ms | 133.5ms | 0.553x | -44.7% | 5000 |
| 5000 | index_reindex | 288.1ms | 299.8ms | 1.041x | +4.1% | - |
| 5000 | read_package sample-256 | 28.4ms | 9.4ms | 0.332x | -66.8% | 256 |
| 10000 | bbox_query full | 542.8ms | 312.6ms | 0.576x | -42.4% | 10000 |
| 10000 | index_reindex | 623.0ms | 633.1ms | 1.016x | +1.6% | - |
| 10000 | read_package sample-256 | 28.2ms | 9.5ms | 0.337x | -66.3% | 256 |
| 25000 | bbox_query full | 2.021s | 1.183s | 0.585x | -41.5% | 25000 |
| 25000 | index_reindex | 2.087s | 2.146s | 1.028x | +2.8% | - |
| 25000 | read_package sample-256 | 28.5ms | 9.5ms | 0.335x | -66.5% | 256 |

## Largest CityJSONSeq Speedups

| Subset | Operation | CityJSON | CityJSONSeq | Seq/CityJSON Time | Delta |
|---:|---|---:|---:|---:|---:|
| 25000 | get 01HP4Q2S5R55GJ0EK96XKHSB7J | 31.0ms | 0.1ms | 0.004x | -99.6% |
| 10000 | get 01HP4Q2S5R55GJ0EK96XKHSB7J | 7.9ms | 0.1ms | 0.016x | -98.4% |
| 5000 | get 01HP4Q2S5R55GJ0EK96XKHSB7J | 3.8ms | 0.1ms | 0.033x | -96.7% |
| 1000 | get 01HP4Q2S5R55GJ0EK96XKHSB7J | 2.5ms | 0.1ms | 0.043x | -95.7% |
| 25000 | get 01HP5C641S6KXAJS3ETXSCA025 | 0.2ms | 0.1ms | 0.310x | -69.0% |
| 1000 | read_package sample-256 | 28.1ms | 9.3ms | 0.331x | -66.9% |
| 5000 | read_package sample-256 | 28.4ms | 9.4ms | 0.332x | -66.8% |
| 25000 | read_package sample-256 | 28.5ms | 9.5ms | 0.335x | -66.5% |
| 10000 | read_package sample-256 | 28.2ms | 9.5ms | 0.337x | -66.3% |
| 25000 | bbox_query small | 2.5ms | 0.9ms | 0.341x | -65.9% |
| 1000 | bbox_query medium | 1.2ms | 0.4ms | 0.352x | -64.8% |
| 1000 | bbox_query large | 22.0ms | 7.9ms | 0.360x | -64.0% |

## Largest CityJSONSeq Slowdowns

| Subset | Operation | CityJSON | CityJSONSeq | Seq/CityJSON Time | Delta |
|---:|---|---:|---:|---:|---:|
| 10000 | dataset_open | 63.6ms | 67.4ms | 1.059x | +5.9% |
| 5000 | index_reindex | 288.1ms | 299.8ms | 1.041x | +4.1% |
| 1000 | index_reindex | 123.8ms | 127.3ms | 1.029x | +2.9% |
| 25000 | index_reindex | 2.087s | 2.146s | 1.028x | +2.8% |
| 10000 | index_reindex | 623.0ms | 633.1ms | 1.016x | +1.6% |
| 10000 | full_scan_reference_iteration | 7.9ms | 8.0ms | 1.012x | +1.2% |
| 5000 | full_scan_reference_iteration | 3.3ms | 3.4ms | 1.010x | +1.0% |
| 25000 | full_scan_reference_iteration | 30.6ms | 30.6ms | 1.003x | +0.3% |
| 5000 | dataset_open | 61.3ms | 60.2ms | 0.982x | -1.8% |
| 1000 | dataset_open | 62.7ms | 60.9ms | 0.971x | -2.9% |
| 25000 | dataset_open | 65.6ms | 58.3ms | 0.889x | -11.1% |
| 5000 | get 01HP51HQJM7R1DVJJYTAPWGF71 | 0.1ms | 0.0ms | 0.669x | -33.1% |

## Notes

- `read_package sample-256` is the clearest reconstruction-heavy signal in this focused run: CityJSONSeq stays around `9.3-9.5ms`, while regular CityJSON stays around `28.1-28.5ms`.
- `index_reindex` is slightly slower for CityJSONSeq on these subsets because line-oriented input has different scan/materialization overheads.
- `bbox_query full` includes reconstruction of every matching package and shows CityJSONSeq faster at every subset size in this run.
