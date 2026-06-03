# Release Subset Historical Benchmark Comparison

Compares the new focused subset benchmark against the existing June 3 benchmark artifacts.

## Inputs

- Baseline JSON: [`2026-06-03-baseline-release-cityjson.json`](2026-06-03-baseline-release-cityjson.json)
- Previous implementation JSON: [`2026-06-03-implementation-release-cityjson.json`](2026-06-03-implementation-release-cityjson.json)
- Current subset JSON: [`2026-06-04-release-subsets-cityjson-vs-cityjsonseq.json`](2026-06-04-release-subsets-cityjson-vs-cityjsonseq.json)
- Baseline commit: `1c5195f89c32918be5d6a94b85b135a559292230`
- Previous implementation commit: `1f156f085a6fa3f2440ca6092062f40d1efa49fc`
- Current commit: `d3030585436f4b1377a36f1633b120506650bd79`

## Method

The comparison is limited to rows present in all three artifacts: `single-tile-subsets`, worker count `1`, regular `city-json` in the current artifact, and matching operation/variant keys. Dataset labels are normalized by removing the layout suffix. The older baseline `read_feature sample-256` operation is matched to newer `read_package sample-256`.

Ratios are always `candidate / reference`, so values below `1.0x` mean the candidate was faster.

## Summary

- Matched rows across all artifacts: `12`.
- Previous implementation vs baseline: `1.169x` mean elapsed-time ratio; faster in `1/12` rows.
- Current regular CityJSON vs previous implementation: `0.990x` mean elapsed-time ratio; faster in `11/12` rows.
- Current CityJSONSeq vs previous implementation: `0.623x` mean elapsed-time ratio; faster in `8/12` rows.
- Current CityJSONSeq vs baseline: `0.730x` mean elapsed-time ratio; faster in `9/12` rows.

## Operation Summary

| Operation | Rows | Impl/Baseline | Current CityJSON/Impl | Current Seq/Impl | Current Seq/Baseline |
|---|---:|---:|---:|---:|---:|
| bbox_query full | 4 | 1.341x | 0.997x | 0.527x | 0.714x |
| index_reindex | 4 | 1.119x | 0.984x | 1.011x | 1.131x |
| read_package/read_feature sample-256 | 4 | 1.048x | 0.990x | 0.330x | 0.346x |

## Focused Rows

| Subset | Operation | Baseline | Previous Impl | Current CityJSON | Current Seq | Impl/Base | Current City/Impl | Current Seq/Impl | Current Seq/Base |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1000 | bbox_query full | 74.2ms | 84.9ms | 84.3ms | 33.6ms | 1.144x | 0.993x | 0.395x | 0.452x |
| 1000 | index_reindex | 132.4ms | 126.1ms | 123.8ms | 127.3ms | 0.952x | 0.982x | 1.010x | 0.962x |
| 1000 | read_package/read_feature sample-256 | 27.0ms | 28.4ms | 28.1ms | 9.3ms | 1.053x | 0.990x | 0.328x | 0.345x |
| 5000 | bbox_query full | 163.1ms | 242.1ms | 241.3ms | 133.5ms | 1.484x | 0.997x | 0.552x | 0.819x |
| 5000 | index_reindex | 241.0ms | 295.6ms | 288.1ms | 299.8ms | 1.226x | 0.975x | 1.014x | 1.244x |
| 5000 | read_package/read_feature sample-256 | 27.5ms | 28.6ms | 28.4ms | 9.4ms | 1.041x | 0.992x | 0.329x | 0.343x |
| 10000 | bbox_query full | 378.8ms | 546.0ms | 542.8ms | 312.6ms | 1.442x | 0.994x | 0.572x | 0.825x |
| 10000 | index_reindex | 508.0ms | 630.1ms | 623.0ms | 633.1ms | 1.240x | 0.989x | 1.005x | 1.246x |
| 10000 | read_package/read_feature sample-256 | 27.2ms | 28.6ms | 28.2ms | 9.5ms | 1.052x | 0.988x | 0.333x | 0.350x |
| 25000 | bbox_query full | 1.560s | 2.016s | 2.021s | 1.183s | 1.292x | 1.003x | 0.587x | 0.758x |
| 25000 | index_reindex | 1.998s | 2.110s | 2.087s | 2.146s | 1.056x | 0.989x | 1.017x | 1.074x |
| 25000 | read_package/read_feature sample-256 | 27.5ms | 28.8ms | 28.5ms | 9.5ms | 1.045x | 0.991x | 0.332x | 0.347x |

## Interpretation

- Current regular `city-json` is effectively unchanged from the previous implementation on the matched subset rows: the mean ratio is close to `1.0x`, and the reconstruction-heavy `read_package sample-256` rows remain around `28ms`.
- Current `city-json-seq` is materially faster than the previous regular-CityJSON implementation for reconstruction-heavy reads: `read_package sample-256` averages roughly one third of the previous implementation time.
- `index_reindex` remains slightly slower for CityJSONSeq than regular CityJSON in this focused run, matching the current-layout comparison.
- The current artifact does not include a pre-change CityJSONSeq result, so the direct before/after CityJSONSeq speedup cannot be measured from the stored benchmark set. This comparison instead shows current CityJSONSeq against the previous regular-CityJSON implementation and baseline.
