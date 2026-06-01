# AGENTS.md

Orientation for AI coding agents working in this repo. Humans: see
[`README.md`](README.md), [`CONTRIBUTING.md`](CONTRIBUTING.md), and
[`docs/development.md`](docs/development.md) — this file is derived from
those and they remain authoritative on conflict.

## What this repo is

3DGI Rust workspace for CityJSON 2.0: native parsing, format adapters
(JSON, Arrow, Parquet), a higher-level facade with FFI, synthetic data
generation, SQLite-backed indexing. All crates release in lockstep under
a single version line.

## Layout

```
Cargo.toml              # workspace manifest — version, shared deps, lints
justfile                # canonical recipes
rust-toolchain.toml     # pins stable; respected by rustup
release.toml            # cargo-release config
CHANGELOG.md            # Keep a Changelog; promoted manually at release
crates/
  cityjson-types/       # core types
  cityjson-json/        # serde adapter
  cityjson-arrow/       # Arrow IPC transport
  cityjson-parquet/     # Parquet over cityjson-arrow
  cityjson-lib/         # higher-level facade (+ PyPI wheel)
    ffi/core/ ffi/python/ ffi/wasm/
  cityjson-fake/        # synthetic data + cjfake CLI
  cityjson-index/       # SQLite index + cjindex CLI (+ PyPI wheel)
    ffi/core/ ffi/python/
```

Dependency direction: `cityjson-types` → {`-json`, `-arrow`}; `-arrow` →
`-parquet`; `{-json,-arrow,-parquet}` → `-lib` → {`-fake`, `-index`}.

Shared test fixtures live in the separate
[`cityjson-corpus`](https://github.com/3DGI/cityjson-corpus) repo. Point
at a local checkout via `CITYJSON_SHARED_CORPUS_ROOT`.

## Crate Catalog

| Crate | Description | Crates.io | PyPI | Notes |
|-------|-------------|-----------|------|-------|
| `cityjson-types` | Core CityJSON 2.0 types and accessors | ✅ | | Foundation for all other crates |
| `cityjson-json` | Serde adapter for CityJSON 2.0 | ✅ | | JSON serialization/deserialization |
| `cityjson-arrow` | Arrow IPC and Parquet transport | ✅ | | Bridge to Arrow ecosystem |
| `cityjson-parquet` | Parquet read/write via cityjson-arrow | ✅ | | Built on cityjson-arrow |
| `cityjson-lib` | Higher-level read/write facade | ✅ | ✅ | Main API crate, includes FFI for WASM |
| `cityjson-fake` | Synthetic CityJSON data generator | ✅ | | `cjfake` CLI for test data generation |
| `cityjson-index` | SQLite-backed indexing layer | ✅ | ✅ | `cjindex` CLI, sidecar index databases |

Dependency graph: `cityjson-types` → `{json, arrow}` → `parquet` → `lib` → `{fake, index}`

For crate-specific agent guidance, see the crate-level `AGENTS.md` files
(e.g., [`crates/cityjson-index/AGENTS.md`](crates/cityjson-index/AGENTS.md)).

## Toolchain

- Rust: `stable`, pinned via `rust-toolchain.toml`. MSRV `1.93`, edition
  `2024`, resolver `3`.
- Nightly: only for `just doc` (docsrs cfg) and `just miri`.
- Python: `>=3.11`, supported 3.11–3.13. Package manager: `uv`
  (per-crate `uv.lock` committed).
- `just` for all recipes.

## Commands

Use the justfile — don't invent cargo invocations.

| Recipe      | What it does                                                              |
|-------------|---------------------------------------------------------------------------|
| `check`     | `cargo check --workspace --all-targets --all-features`                    |
| `build`     | `cargo build --workspace --all-targets --all-features`                    |
| `lint`      | `cargo clippy` — flags come from `[workspace.lints]`, not the recipe      |
| `fmt`       | `cargo fmt --all` (no `rustfmt.toml`; edition 2024 defaults)              |
| `fmt-check` | `cargo fmt --all --check`                                                 |
| `test`      | `cargo test --workspace --all-features`                                   |
| `doc`       | Nightly docsrs build, warnings as errors                                  |
| `ci`        | `fmt-check` + `lint` + `check` + `test` + `doc` — must pass before a PR   |
| `miri`      | Nightly miri on `unsafe`-touching modules of `cityjson-types`             |
| `test-python` / `build-python` | tox / wheel build for the two Python-shipping crates    |

Per-crate scoping uses `-p <crate>`, not different flags.
`--all-features` is deliberate — broken feature combinations are a bug.

## Hard rules

- **`just ci` must pass** before claiming done. If you touched
  `cityjson-lib` or `cityjson-index`, also run `just test-python`. If
  you added or edited `unsafe`, extend and run `just miri`.
- **New behaviour needs tests.** Bug fixes need a regression test that
  fails before the fix. Public API changes need doc updates.
- **Clippy is `deny` on `all` + `pedantic` workspace-wide.** Targeted
  `#[allow(clippy::…)]` is fine with a one-line reason comment.
- **Rustdoc warnings and broken intra-doc links fail the build.** Docs
  are a feature here.
- **Internal deps go through `[workspace.dependencies]`.** No
  `path = "../foo"` in a crate's own `Cargo.toml`.
- **Every crate has `[lints] workspace = true`** and inherits version,
  edition, rust-version, license, repository, authors from
  `[workspace.package]`.
- **Don't hand-edit Python `version` fields.** `cargo-release` syncs
  them from the Rust workspace version at release time.
- **Match the existing style of the crate you're editing.** The
  workspace is internally consistent on purpose.
- **Keep PRs small and focused — one topic per PR.** If you remove or
  merge tests, examples, or benchmarks, explain why in the PR.
- **Don't submit unreviewed LLM output.** Run the tests locally.
  Correctness here is guarded by a curated test suite and benches that
  exercise the full CityJSON 2.0 spec — extend them for what you
  changed.

## Crate README contract

Each crate README, in order: title + badges; one-paragraph description;
install / quick start; usage examples; features table (if any); MSRV
line; optional link to crate-local dev notes; a short Contributing
section pointing at the root `CONTRIBUTING.md` and
`docs/development.md`; license. Don't reintroduce per-crate "Use of AI"
sections — they're consolidated in `CONTRIBUTING.md`.

## CI

`.github/workflows/ci.yml`:

- **PRs** — full matrix, non-negotiable.
- **Pushes to `main`** — selective. `.github/scripts/affected-crates.sh`
  classifies the diff and emits the downstream closure of changed
  crates. Workspace-level changes (root `Cargo.toml`, `Cargo.lock`,
  `rust-toolchain.toml`, `justfile`, `release.toml`, the CI workflow,
  `.github/scripts/`) trigger the full suite, as does any unrecognised
  path.

Exercise the classifier locally:

```sh
GITHUB_EVENT_NAME=push \
GITHUB_EVENT_BEFORE=$(git rev-parse HEAD~1) \
GITHUB_SHA=$(git rev-parse HEAD) \
bash .github/scripts/affected-crates.sh
```

**Adding a new crate** means editing `affected-crates.sh` in two
places: append to `ALL_CRATES` (and `PYTHON_CRATES` if applicable), and
add a `CLOSURE[<name>]=…` line covering the crate plus its transitive
downstream — and update upstream closures too (the easy-to-miss step).

CI is the source of truth for "passing". Green locally but red in CI →
CI wins.

## Release

From a clean `main`:

```sh
cargo release patch --execute    # or minor / major
```

`shared-version = true` bumps all crates in lockstep;
`consolidate-commits = true` produces one commit; tag `v<x.y.z>` is
pushed and triggers `release.yml`, which builds and publishes Python
wheels. Crates.io publish is in dependency order.

Two manual pre-release steps:

1. Promote `## [Unreleased]` in `CHANGELOG.md` to `## [x.y.z] — <date>`.
2. `git status` clean and `just ci` green.

`allow-branch = ["main"]` — releases from branches are rejected by
design. `release.yml` does not re-run the full suite; it verifies
`ci.yml` succeeded on the tagged commit.

## Setup troubleshooting

- `just doc` fails → `rustup toolchain install nightly`.
- Tests complain about missing corpus → set
  `CITYJSON_SHARED_CORPUS_ROOT` to a local checkout of
  `cityjson-corpus`, or the crate-specific env var if one is
  documented (e.g. `CITYJSON_JSON_BENCHMARK_INDEX`).

## Debugging & Troubleshooting

### Common Issues

- **"Missing corpus" errors**: Set `CITYJSON_SHARED_CORPUS_ROOT` to point at a
  local clone of [`cityjson-corpus`](https://github.com/3DGI/cityjson-corpus)
- ** Nightly-only commands fail**: Ensure nightly toolchain is installed:
  `rustup toolchain install nightly`
- **Python FFI build failures**: Ensure Python 3.11-3.13 is available and
  `uv` is installed; see crate-specific FFI docs
- **SQLite errors in cityjson-index**: Check the sidecar database with
  `sqlite3 <dataset>/.cityjson-index.sqlite "SELECT * FROM sources;"`

### Profiling

Use `--profile` flags where available (e.g., `cjindex` CLI commands) to
emit JSON profiling data for performance analysis.

## FFI Development

Two crates ship Python bindings: `cityjson-lib` and `cityjson-index`.

- **Build system**: `maturin` for Rust-Python interop
- **Test harness**: `tox` with `uv` for isolated Python environments
- **Wheel building**: `uv build --wheel` in each crate's `ffi/python/`
- **C FFI**: Generated via `cbindgen` in crate `ffi/core/` directories
- **Version sync**: Python versions are synced from `Cargo.toml` at
  release time by `cargo-release` — do NOT hand-edit

## Licensing

Dual MIT / Apache-2.0 per the `license` field in each crate's
`Cargo.toml` (authoritative). Contributions are accepted under the same
terms as the crate being contributed to, no additional conditions.

## Glossary

| Term | Meaning |
|------|---------|
| **CityJSON** | JSON-based encoding for 3D city models (v2.0) |
| **CityJSONSeq** | Line-delimited CityJSON (NDJSON format) |
| **Sidecar** | Companion file/database alongside main data (e.g., `.cityjson-index.sqlite`) |
| **FFI** | Foreign Function Interface — bindings for non-Rust languages |
| **MSRV** | Minimum Supported Rust Version (currently 1.93) |
| **lockstep releases** | All crates version bump together, same release |
| **cargo-release** | Tool used for automated version bumps and publishing |

## See Also

- [`README.md`](README.md) — Project overview and quick start
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — PR guidelines and AI use policy
- [`docs/development.md`](docs/development.md) — Full development contract
- [`crates/cityjson-index/AGENTS.md`](crates/cityjson-index/AGENTS.md) — Crate-specific agent guidance for cityjson-index
