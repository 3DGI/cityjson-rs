# Streaming rebuild comparison

Date: 2026-06-04

Previous implementation: `fffaf1e` (`/tmp/cityjson-rs-prev`, direct targeted checks and `/tmp/cityjson-index-prev-targeted.json`)
Current implementation: working tree after streaming rebuild changes (`2026-06-04-streaming-rebuild.json` and `/tmp/cityjson-index-current-streaming-targeted.json`)

Primary benchmark command for the recorded artifact:

```sh
just bench-index-json --workers 1 --workers 4
```

Targeted current-vs-previous benchmark command:

```sh
just bench-index-json --case single-tile-subsets --layout city-json-seq --layout feature-files --workers 1 --workers 4
```

## Summary

- The rebuild path no longer materializes every scanned source and then clones all features into a second whole-index insertion vector.
- SQLite population remains single-writer/single-transaction; backend scanning and row preparation still use `CITYJSON_INDEX_WORKERS`.
- Targeted release benchmarks across `city-json-seq` and `feature-files` subset rebuilds show `index_reindex` averaging `0.880x` of `fffaf1e` elapsed time, with 12/16 rows faster.
- Direct CLI peak RSS checks on the 25k subsets show the memory improvement more clearly than harness VmHWM snapshots:
  - `cityjson-seq`: `132124 KB -> 33004 KB` max RSS, `1.21s -> 1.14s` elapsed.
  - `feature-files`: `73072 KB -> 42904 KB` max RSS, `1.46s -> 1.43s` elapsed.

## Targeted `index_reindex` Details

| layout | subset | workers | previous ms | current ms | ratio | previous RSS MiB | current RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| `city-json-seq` | 1000 | 1 | 68.9 | 76.4 | 1.110x | 3919.9 | 3922.8 |
| `city-json-seq` | 1000 | 4 | 67.9 | 72.6 | 1.070x | 3920.2 | 3923.1 |
| `city-json-seq` | 5000 | 1 | 155.2 | 129.9 | 0.837x | 3920.2 | 3923.4 |
| `city-json-seq` | 5000 | 4 | 153.7 | 137.6 | 0.895x | 3920.2 | 3923.4 |
| `city-json-seq` | 10000 | 1 | 326.0 | 236.8 | 0.727x | 3920.2 | 3935.6 |
| `city-json-seq` | 10000 | 4 | 322.0 | 236.4 | 0.734x | 3920.2 | 3935.7 |
| `city-json-seq` | 25000 | 1 | 1092.5 | 874.8 | 0.801x | 3920.2 | 3937.9 |
| `city-json-seq` | 25000 | 4 | 1094.2 | 882.1 | 0.806x | 3920.2 | 3937.9 |
| `feature-files` | 1000 | 1 | 71.5 | 85.1 | 1.191x | 3959.4 | 3977.1 |
| `feature-files` | 1000 | 4 | 44.4 | 49.2 | 1.107x | 3965.3 | 3980.2 |
| `feature-files` | 5000 | 1 | 197.5 | 159.0 | 0.805x | 3965.3 | 3981.2 |
| `feature-files` | 5000 | 4 | 158.1 | 135.0 | 0.854x | 3967.0 | 3981.6 |
| `feature-files` | 10000 | 1 | 397.8 | 312.8 | 0.786x | 3967.0 | 3995.3 |
| `feature-files` | 10000 | 4 | 268.6 | 224.3 | 0.835x | 3986.0 | 4009.7 |
| `feature-files` | 25000 | 1 | 1305.2 | 1061.6 | 0.813x | 3986.0 | 4013.4 |
| `feature-files` | 25000 | 4 | 793.9 | 565.0 | 0.712x | 4022.1 | 4029.6 |

## Notes

- The benchmark harness RSS columns are process snapshots and VmHWM values for the full harness process, so they are not good isolated per-rebuild peak measurements. The direct `/usr/bin/time` checks above are the better memory signal for this change.
- Regular `city-json` still parses a whole source document while computing package bounds; this change removes whole-rebuild accumulation but does not yet replace that per-source parser.
