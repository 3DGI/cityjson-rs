# Release CityJSON Benchmark Comparison

Captured after rerunning both the pre-normalization baseline commit and the
current checkout with optimized release binaries.

## Environment

- Captured: `2026-06-03T05:09:25+02:00`
- OS: `Linux workstation 6.17.0-29-generic #29~24.04.1-Ubuntu SMP PREEMPT_DYNAMIC Mon May 11 10:30:58 UTC 2 x86_64 GNU/Linux`
- CPU: `AMD Ryzen 9 9900X 12-Core Processor`
- Logical CPUs: `24`
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`

## Inputs

- Baseline commit: `1c5195f89c32918be5d6a94b85b135a559292230`
- Current commit: `157e80fe7500aba698a6339b5cf681c55588ab1d`
- Baseline JSON: [`2026-06-03-baseline-release-cityjson.json`](2026-06-03-baseline-release-cityjson.json)
- Current JSON: [`2026-06-03-current-release-cityjson.json`](2026-06-03-current-release-cityjson.json)

## Commands

Baseline was run from a detached worktree at the baseline commit:

```bash
cargo run -p cityjson-index --bin bench-index --release --target-dir target -- --json \
  > docs/benchmarks/2026-06-03-baseline-release-cityjson.json
```

Current was run from the current checkout. The current harness benchmarks
multiple layouts by default, so this comparison uses the `city-json` layout to
match the older harness:

```bash
cargo run -p cityjson-index --bin bench-index --release --target-dir target -- --json --layout city-json \
  > docs/benchmarks/2026-06-03-current-release-cityjson.json
```

## Comparison Notes

- Compared `162` matched dataset/worker/operation groups.
- `151` groups are slower in the current checkout, `11` are faster.
- Mean current/baseline speed ratio: `0.435x`.
- Current dataset labels ending in `-cityjson` were normalized to the older
  baseline labels.
- `read_feature sample-256` in the baseline was compared with
  `read_package sample-256` in the current harness.
- Repeated `get` samples are represented by median speed.

## Largest Slowdowns

| Dataset | Workers | Operation | Baseline | Current | Ratio | Delta |
|---|---:|---|---:|---:|---:|---:|
| single-tile-full | 1 | bbox_query medium | 22086 | 645 | 0.03x | -97.1% |
| single-tile-full | 24 | bbox_query medium | 20487 | 626 | 0.03x | -96.9% |
| single-tile-full | 4 | bbox_query medium | 20587 | 631 | 0.03x | -96.9% |
| single-tile-full | 24 | bbox_query large | 16316 | 617 | 0.04x | -96.2% |
| single-tile-full | 4 | bbox_query large | 16299 | 627 | 0.04x | -96.2% |
| single-tile-full | 1 | bbox_query large | 16199 | 638 | 0.04x | -96.1% |
| single-tile-full | 24 | bbox_query full | 13816 | 617 | 0.04x | -95.5% |
| single-tile-full | 4 | bbox_query full | 13825 | 624 | 0.05x | -95.5% |
| single-tile-full | 1 | bbox_query full | 13765 | 638 | 0.05x | -95.4% |
| single-tile-subset-25000 | 4 | bbox_query medium | 20538 | 1014 | 0.05x | -95.1% |
| single-tile-subset-25000 | 1 | bbox_query medium | 20426 | 1016 | 0.05x | -95.0% |
| single-tile-subset-25000 | 24 | bbox_query medium | 20405 | 1027 | 0.05x | -95.0% |

## Largest Speedups

| Dataset | Workers | Operation | Baseline | Current | Ratio | Delta |
|---|---:|---|---:|---:|---:|---:|
| multi-source | 1 | get median | 1226 | 1570 | 1.28x | +28.1% |
| multi-source | 24 | get median | 1236 | 1581 | 1.28x | +28.0% |
| multi-source | 4 | get median | 1224 | 1566 | 1.28x | +27.9% |
| single-tile-subset-1000 | 4 | full_scan_reference_iteration | 812197 | 938431 | 1.16x | +15.5% |
| single-tile-subset-1000 | 24 | full_scan_reference_iteration | 801251 | 920837 | 1.15x | +14.9% |
| single-tile-subset-1000 | 4 | bbox_query small | 3211 | 3496 | 1.09x | +8.9% |
| single-tile-subset-1000 | 24 | index_reindex | 7454 | 7889 | 1.06x | +5.8% |
| single-tile-subset-1000 | 1 | index_reindex | 7551 | 7964 | 1.05x | +5.5% |

## Focused Cases

Units are `hits/s` for bbox queries, `opens/s` for dataset open, `gets/s` for
get, and `features/s` for the remaining operations.

| Dataset | Workers | Operation | Unit | Baseline | Current | Ratio | Delta |
|---|---:|---|---:|---:|---:|---:|---:|
| multi-source | 1 | bbox_query full | hits/s | 29044 | 7270 | 0.25x | -75.0% |
| multi-source | 1 | bbox_query medium | hits/s | 35745 | 7585 | 0.21x | -78.8% |
| multi-source | 1 | full_scan_reference_iteration | features/s | 849491 | 483250 | 0.57x | -43.1% |
| multi-source | 1 | index_reindex | features/s | 19616 | 16170 | 0.82x | -17.6% |
| multi-source | 1 | read sample-256 | features/s | 9312 | 4870 | 0.52x | -47.7% |
| multi-source | 1 | get median | gets/s | 1226 | 1570 | 1.28x | +28.1% |
| single-tile-full | 1 | bbox_query full | hits/s | 13765 | 638 | 0.05x | -95.4% |
| single-tile-full | 1 | bbox_query medium | hits/s | 22086 | 645 | 0.03x | -97.1% |
| single-tile-full | 1 | full_scan_reference_iteration | features/s | 839052 | 59995 | 0.07x | -92.8% |
| single-tile-full | 1 | index_reindex | features/s | 17479 | 10539 | 0.60x | -39.7% |
| single-tile-full | 1 | read sample-256 | features/s | 10372 | 616 | 0.06x | -94.1% |
| single-tile-full | 1 | get median | gets/s | 517 | 418 | 0.81x | -19.3% |
