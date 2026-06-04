# Release CityJSON Benchmark Comparison

Captured after rerunning the baseline and the implementation with optimized release binaries.

## Environment

- Captured: `2026-06-03T13:55:26+02:00`
- OS: `Linux 6.17.0-29-generic #29~24.04.1-Ubuntu SMP PREEMPT_DYNAMIC Mon May 11 10:30:58 UTC 2 x86_64 GNU/Linux`
- CPU: `AMD Ryzen 9 9900X 12-Core Processor`
- Logical CPUs: `24`
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`

## Inputs

- Baseline commit: `1c5195f89c32918be5d6a94b85b135a559292230`
- Implementation commit: `1f156f085a6fa3f2440ca6092062f40d1efa49fc`
- Baseline JSON: [`2026-06-03-baseline-release-cityjson.json`](2026-06-03-baseline-release-cityjson.json)
- Implementation JSON: [`2026-06-03-implementation-release-cityjson.json`](2026-06-03-implementation-release-cityjson.json)

## Commands

Baseline was run from a detached worktree at the baseline commit:

```bash
cargo run -p cityjson-index --bin bench-index --release --target-dir target -- --json \
  > docs/benchmarks/2026-06-03-baseline-release-cityjson.json
```

Implementation was run from commit `1f156f085a6fa3f2440ca6092062f40d1efa49fc`. The implementation harness supports multiple layouts, so this report uses `--layout city-json` to match the older baseline harness:

```bash
cargo run -p cityjson-index --bin bench-index --release --target-dir target -- --json --layout city-json \
  > docs/benchmarks/2026-06-03-implementation-release-cityjson.json
```

## Summary

- Compared `198` matched dataset/worker/operation groups using median elapsed time.
- `72` groups are faster than baseline, `126` are slower than baseline.
- Mean implementation/baseline elapsed-time ratio: `1.180x`.
- Implementation dataset labels ending in `-cityjson` were normalized to the older baseline labels.
- Baseline `read_feature sample-256` was compared with implementation `read_package sample-256`.

## Largest Speedups Vs Baseline

| Dataset | Workers | Operation | Baseline | Implementation | Impl/Baseline Time | Delta |
|---|---:|---|---:|---:|---:|---:|
| single-tile-subset-25000 | 24 | get 01HP91VX9EH4KBG5MP1REBVEAB | 0.0012s | 0.0001s | 0.073x | -92.7% |
| single-tile-subset-25000 | 4 | get 01HP91VX9EH4KBG5MP1REBVEAB | 0.0011s | 0.0001s | 0.073x | -92.7% |
| single-tile-full | 24 | get NL.IMBAG.Pand.1926100000570105 | 0.0020s | 0.0002s | 0.080x | -92.0% |
| single-tile-full | 1 | get NL.IMBAG.Pand.1926100000570105 | 0.0019s | 0.0002s | 0.083x | -91.7% |
| single-tile-full | 4 | get NL.IMBAG.Pand.1926100000570105 | 0.0019s | 0.0002s | 0.084x | -91.6% |
| single-tile-subset-25000 | 1 | get 01HP91VX9EH4KBG5MP1REBVEAB | 0.0011s | 0.0001s | 0.085x | -91.5% |
| single-tile-full | 24 | get 01HP6ACVDER3X6V0HWX93WAT4K | 0.0012s | 0.0002s | 0.162x | -83.8% |
| single-tile-full | 4 | get 01HP6ACVDER3X6V0HWX93WAT4K | 0.0012s | 0.0002s | 0.164x | -83.6% |
| single-tile-subset-10000 | 1 | get 01HP5C61FVCDE1SNEP2F7YDJ7Y | 0.0005s | 0.0001s | 0.171x | -82.9% |
| single-tile-full | 1 | get 01HP6ACVDER3X6V0HWX93WAT4K | 0.0012s | 0.0002s | 0.172x | -82.8% |
| single-tile-subset-10000 | 4 | get 01HP5C61FVCDE1SNEP2F7YDJ7Y | 0.0004s | 0.0001s | 0.198x | -80.2% |
| single-tile-subset-10000 | 24 | get 01HP5C61FVCDE1SNEP2F7YDJ7Y | 0.0004s | 0.0001s | 0.205x | -79.5% |

## Largest Remaining Slowdowns Vs Baseline

| Dataset | Workers | Operation | Baseline | Implementation | Impl/Baseline Time | Delta |
|---|---:|---|---:|---:|---:|---:|
| single-tile-subset-10000 | 1 | dataset_open | 0.0155s | 0.0650s | 4.209x | +320.9% |
| single-tile-subset-1000 | 1 | dataset_open | 0.0149s | 0.0560s | 3.755x | +275.5% |
| single-tile-subset-5000 | 24 | dataset_open | 0.0192s | 0.0669s | 3.476x | +247.6% |
| single-tile-subset-5000 | 4 | dataset_open | 0.0181s | 0.0623s | 3.436x | +243.6% |
| multi-source | 24 | dataset_open | 0.0180s | 0.0606s | 3.375x | +237.5% |
| single-tile-subset-1000 | 4 | dataset_open | 0.0182s | 0.0606s | 3.329x | +232.9% |
| single-tile-subset-5000 | 1 | dataset_open | 0.0180s | 0.0576s | 3.207x | +220.7% |
| single-tile-full | 4 | dataset_open | 0.0183s | 0.0584s | 3.194x | +219.4% |
| single-tile-full | 24 | dataset_open | 0.0184s | 0.0575s | 3.119x | +211.9% |
| single-tile-subset-10000 | 4 | dataset_open | 0.0214s | 0.0638s | 2.978x | +197.8% |
| multi-source | 4 | dataset_open | 0.0196s | 0.0584s | 2.975x | +197.5% |
| single-tile-full | 1 | dataset_open | 0.0210s | 0.0613s | 2.914x | +191.4% |

## Focused Cases

| Dataset | Workers | Operation | Baseline | Implementation | Impl/Baseline Time | Delta |
|---|---:|---|---:|---:|---:|---:|
| multi-source | 1 | bbox_query full | 0.1377s | 0.1992s | 1.446x | +44.6% |
| multi-source | 1 | bbox_query medium | 0.0030s | 0.0044s | 1.461x | +46.1% |
| multi-source | 1 | full_scan_reference_iteration | 0.0047s | 0.0027s | 0.578x | -42.2% |
| multi-source | 1 | index_reindex | 0.2039s | 0.2503s | 1.227x | +22.7% |
| multi-source | 1 | read_package sample-256 | 0.0275s | 0.0286s | 1.041x | +4.1% |
| single-tile-full | 1 | bbox_query full | 2.9287s | 4.0070s | 1.368x | +36.8% |
| single-tile-full | 1 | bbox_query medium | 0.0572s | 0.0797s | 1.394x | +39.4% |
| single-tile-full | 1 | full_scan_reference_iteration | 0.0480s | 0.0649s | 1.351x | +35.1% |
| single-tile-full | 1 | index_reindex | 2.3063s | 3.7737s | 1.636x | +63.6% |
| single-tile-full | 1 | read_package sample-256 | 0.0247s | 0.0290s | 1.177x | +17.7% |
