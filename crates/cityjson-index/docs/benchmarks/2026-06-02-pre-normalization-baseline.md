# Pre-Normalization Index Benchmark Baseline

Captured before the normalized package and CityObject index refactor.

## Environment

- Captured: `2026-06-02T13:08:52+02:00`
- `cityjson-rs` commit: `1c5195f89c32918be5d6a94b85b135a559292230`
- `cityjson-corpus` commit: `6a63552fca210d111553cfc071706b4feafa9478`
- OS: `Linux 6.17.0-29-generic x86_64 GNU/Linux`
- CPU: `AMD Ryzen 9 9900X 12-Core Processor`
- Logical CPUs: `24`
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`

## Input

- Artifact: `artifacts/acquired/basisvoorziening-3d/2022/3d_volledig_84000_450000.city.json`
- Byte size: `177949687`
- SHA-256: `b4858c9bacdbf485a475824d1495e39843607cf584482017848fc3e066a55467`

## Command

```bash
just bench-index-json > /tmp/cityjson-index-pre-normalization-baseline.json
```

The unmodified harness used its default cases and worker counts: `1`, `4`, and
`24`. The report contains `198` operation records across the full tile, four
single-tile subsets, and the generated multi-source case.

## Raw Report

See [`2026-06-02-pre-normalization-baseline.json`](2026-06-02-pre-normalization-baseline.json).
