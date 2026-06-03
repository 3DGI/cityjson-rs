# Release CityJSON Benchmark Comparison

Captured after rerunning the baseline and the implementation with optimized release binaries.

## Environment

- Captured: `2026-06-03T13:48:34+02:00`
- OS: `Linux 6.17.0-29-generic #29~24.04.1-Ubuntu SMP PREEMPT_DYNAMIC Mon May 11 10:30:58 UTC 2 x86_64 GNU/Linux`
- CPU: `AMD Ryzen 9 9900X 12-Core Processor`
- Logical CPUs: `24`
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`

## Inputs

- Baseline commit: `1c5195f89c32918be5d6a94b85b135a559292230`
- Implementation commit: `038f3edbe11eaeae2508e7a55eef3f7d698534af`
- Baseline JSON: [`2026-06-03-baseline-release-cityjson.json`](2026-06-03-baseline-release-cityjson.json)
- Implementation JSON: [`2026-06-03-implementation-release-cityjson.json`](2026-06-03-implementation-release-cityjson.json)

## Commands

Baseline was run from a detached worktree at the baseline commit:

```bash
cargo run -p cityjson-index --bin bench-index --release --target-dir target -- --json \
  > docs/benchmarks/2026-06-03-baseline-release-cityjson.json
```

Implementation was run from commit `038f3edbe11eaeae2508e7a55eef3f7d698534af`. The implementation harness supports multiple layouts, so this report uses `--layout city-json` to match the older baseline harness:

```bash
cargo run -p cityjson-index --bin bench-index --release --target-dir target -- --json --layout city-json \
  > docs/benchmarks/2026-06-03-implementation-release-cityjson.json
```

## Summary

- Compared `198` matched dataset/worker/operation groups using median elapsed time.
- `72` groups are faster than baseline, `126` are slower than baseline.
- Mean implementation/baseline elapsed-time ratio: `1.190x`.
- Implementation dataset labels ending in `-cityjson` were normalized to the older baseline labels.
- Baseline `read_feature sample-256` was compared with implementation `read_package sample-256`.

## Largest Speedups Vs Baseline

| Dataset | Workers | Operation | Baseline | Implementation | Impl/Baseline Time | Delta |
|---|---:|---|---:|---:|---:|---:|
| single-tile-subset-25000 | 24 | get 01HP91VX9EH4KBG5MP1REBVEAB | 0.0012s | 0.0001s | 0.072x | -92.8% |
| single-tile-subset-25000 | 4 | get 01HP91VX9EH4KBG5MP1REBVEAB | 0.0011s | 0.0001s | 0.076x | -92.4% |
| single-tile-full | 1 | get NL.IMBAG.Pand.1926100000570105 | 0.0019s | 0.0002s | 0.080x | -92.0% |
| single-tile-full | 4 | get NL.IMBAG.Pand.1926100000570105 | 0.0019s | 0.0002s | 0.084x | -91.6% |
| single-tile-full | 24 | get NL.IMBAG.Pand.1926100000570105 | 0.0020s | 0.0002s | 0.085x | -91.5% |
| single-tile-subset-25000 | 1 | get 01HP91VX9EH4KBG5MP1REBVEAB | 0.0011s | 0.0001s | 0.090x | -91.0% |
| single-tile-full | 24 | get 01HP6ACVDER3X6V0HWX93WAT4K | 0.0012s | 0.0002s | 0.170x | -83.0% |
| single-tile-full | 4 | get 01HP6ACVDER3X6V0HWX93WAT4K | 0.0012s | 0.0002s | 0.172x | -82.8% |
| single-tile-subset-10000 | 1 | get 01HP5C61FVCDE1SNEP2F7YDJ7Y | 0.0005s | 0.0001s | 0.175x | -82.5% |
| single-tile-full | 1 | get 01HP6ACVDER3X6V0HWX93WAT4K | 0.0012s | 0.0002s | 0.178x | -82.2% |
| single-tile-subset-10000 | 4 | get 01HP5C61FVCDE1SNEP2F7YDJ7Y | 0.0004s | 0.0001s | 0.198x | -80.2% |
| single-tile-subset-10000 | 24 | get 01HP5C61FVCDE1SNEP2F7YDJ7Y | 0.0004s | 0.0001s | 0.204x | -79.6% |

## Largest Remaining Slowdowns Vs Baseline

| Dataset | Workers | Operation | Baseline | Implementation | Impl/Baseline Time | Delta |
|---|---:|---|---:|---:|---:|---:|
| single-tile-subset-10000 | 1 | dataset_open | 0.0155s | 0.0670s | 4.338x | +333.8% |
| single-tile-subset-1000 | 1 | dataset_open | 0.0149s | 0.0639s | 4.286x | +328.6% |
| single-tile-full | 4 | dataset_open | 0.0183s | 0.0677s | 3.700x | +270.0% |
| multi-source | 24 | dataset_open | 0.0180s | 0.0646s | 3.601x | +260.1% |
| single-tile-subset-5000 | 4 | dataset_open | 0.0181s | 0.0638s | 3.519x | +251.9% |
| single-tile-subset-1000 | 4 | dataset_open | 0.0182s | 0.0639s | 3.510x | +251.0% |
| single-tile-subset-5000 | 1 | dataset_open | 0.0180s | 0.0617s | 3.437x | +243.7% |
| single-tile-full | 24 | dataset_open | 0.0184s | 0.0630s | 3.422x | +242.2% |
| single-tile-subset-5000 | 24 | dataset_open | 0.0192s | 0.0656s | 3.414x | +241.4% |
| single-tile-full | 1 | dataset_open | 0.0210s | 0.0662s | 3.147x | +214.7% |
| multi-source | 4 | dataset_open | 0.0196s | 0.0615s | 3.131x | +213.1% |
| single-tile-subset-1000 | 24 | dataset_open | 0.0204s | 0.0627s | 3.076x | +207.6% |

## Focused Cases

| Dataset | Workers | Operation | Baseline | Implementation | Impl/Baseline Time | Delta |
|---|---:|---|---:|---:|---:|---:|
| multi-source | 1 | bbox_query full | 0.1377s | 0.1989s | 1.444x | +44.4% |
| multi-source | 1 | bbox_query medium | 0.0030s | 0.0045s | 1.482x | +48.2% |
| multi-source | 1 | full_scan_reference_iteration | 0.0047s | 0.0027s | 0.563x | -43.7% |
| multi-source | 1 | index_reindex | 0.2039s | 0.2399s | 1.176x | +17.6% |
| multi-source | 1 | read_package sample-256 | 0.0275s | 0.0285s | 1.038x | +3.8% |
| single-tile-full | 1 | bbox_query full | 2.9287s | 3.9942s | 1.364x | +36.4% |
| single-tile-full | 1 | bbox_query medium | 0.0572s | 0.0807s | 1.410x | +41.0% |
| single-tile-full | 1 | full_scan_reference_iteration | 0.0480s | 0.0647s | 1.348x | +34.8% |
| single-tile-full | 1 | index_reindex | 2.3063s | 3.7703s | 1.635x | +63.5% |
| single-tile-full | 1 | read_package sample-256 | 0.0247s | 0.0290s | 1.174x | +17.4% |
