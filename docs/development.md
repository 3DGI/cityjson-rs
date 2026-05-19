# Development contract

This is the workspace-wide baseline: the versions, flags, and conventions
every crate here follows. Individual crates can deviate — they usually have
a reason — but they should call out the deviation explicitly in their own
README rather than letting it drift silently.

If you're about to write code in this repo, this is the document to read.

## Toolchain

| Thing                  | Value                                                 | Why                                                       |
|------------------------|-------------------------------------------------------|-----------------------------------------------------------|
| Rust channel           | `stable`, pinned via `rust-toolchain.toml`            | Reproducible local builds                                 |
| MSRV                   | `1.93`                                                | Set in `[workspace.package].rust-version`, verified in CI |
| Edition                | `2024`                                                | Inherited from workspace                                  |
| Cargo resolver         | `3`                                                   | Workspace-level                                           |
| Nightly                | Used only for `just doc` (docsrs cfg) and `just miri` | Not required for day-to-day work                          |
| Python                 | `>=3.11`; supported `3.11`, `3.12`, `3.13`            | Matches PyPI wheel matrix                                 |
| Python package manager | `uv`                                                  | Per-crate `uv.lock` is committed                          |
| `just`                 | Any recent version                                    | Recipes live at repo root and per-crate                   |

Install once:

```sh
rustup show                    # respects rust-toolchain.toml
cargo install just
curl -LsSf https://astral.sh/uv/install.sh | sh
```

## Dev containers

The committed root devcontainer is the only supported repo-local entry point.
It owns the dependencies required to build and test this workspace: Rust
toolchains, nightly + Miri support, the wasm target, `uv`, `cbindgen`, `just`,
native build libraries, profiling dependencies, the shared cargo cache, and the
`cityjson-corpus` mount exposed through `CITYJSON_SHARED_CORPUS_ROOT`.

Keep personal tooling separate from project-specific dependencies. The public
`ghcr.io/balazsdukai/devcontainer-tools/tools:1` Dev Container Feature is the
intended layer for language-neutral workflow tools such as Codex, Claude, `gh`,
`ripgrep`, `fzf`, and shell helpers. That Feature should not own compilers,
runtimes, package managers, or native libraries needed by a specific project.
Auth and local state also stay outside the image: mount items such as `~/.codex`,
GitHub CLI config, SSH keys, or GPG material only at runtime when a user needs
them.

Use one of three modes:

1. Shared project mode: open this repository with `.devcontainer/devcontainer.json`
   as committed. This is the contributor baseline and must work without personal
   mounts or credentials.
2. Personal mode for projects you control: use the committed `tools` Feature
   and add private mounts through a local/private config path that is not
   committed to the shared project.
3. Personal mode for projects you do not want to modify: keep a thin private
   wrapper devcontainer outside the project that points at this checkout and adds
   only the tools Feature plus your runtime mounts.

The tools Feature contract should stay stable and language-neutral so the same
layer can sit on top of Rust, Python, and C++ project images. Validate it against
at least one project from each language family, verify the expected tools are
installed, and verify auth/config appears only when runtime mounts are supplied.

## Layout

```
Cargo.toml                  # workspace manifest — version, package metadata, shared deps, lints
justfile                    # canonical recipes (check/build/lint/fmt/test/doc/ci + python/miri/ffi)
rust-toolchain.toml
release.toml                # cargo-release config
CHANGELOG.md                # Keep a Changelog; promoted manually at release time
crates/
  cityjson-types/           # core types
  cityjson-json/            # serde adapter
  cityjson-arrow/           # Arrow IPC transport
  cityjson-parquet/         # Parquet over cityjson-arrow
  cityjson-lib/             # higher-level facade (+ PyPI wheel)
    ffi/core/               # Rust FFI crate
    ffi/python/             # Python package sources + tests + tox
    ffi/wasm/               # wasm bindings
  cityjson-fake/            # synthetic data + cjfake CLI
  cityjson-index/           # SQLite index + cjindex CLI (+ PyPI wheel)
    ffi/core/
    ffi/python/
```

Shared test fixtures and benchmark data live outside this repo in
[`cityjson-corpus`](https://github.com/3DGI/cityjson-corpus). Point at a
local checkout via `CITYJSON_SHARED_CORPUS_ROOT`. Individual crates may also
honour more specific env vars (e.g. `CITYJSON_JSON_BENCHMARK_INDEX`); those
are documented in the crate's own dev notes.

## Cargo metadata

Every crate inherits from `[workspace.package]`:

```toml
[package]
name = "cityjson-<thing>"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "<one line, ends with a period>"
keywords = [...]      # per-crate, <=5, lowercase
categories = [...]      # per-crate, from the crates.io list
readme = "README.md"
```

Internal dependencies always go through `[workspace.dependencies]` — no
`path = "../foo"` in a crate's own `Cargo.toml`. When a crate adds a new
internal dep, add it to the workspace table first, then reference it as
`foo.workspace = true`.

Lints are inherited workspace-wide (see below):

```toml
[lints]
workspace = true
```

If the crate publishes to docs.rs, include:

```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

## Lints

Clippy configuration lives in the root `Cargo.toml` under
`[workspace.lints]`, not in justfile flags. That way `cargo clippy` in an
editor, in CI, or at the shell all see the same rules:

```toml
[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
```

Each crate opts in with `[lints] workspace = true`. Targeted
`#[allow(clippy::…)]` is fine when needed, but attach a one-line comment
with the reason.

`rustc` warnings are promoted to errors in CI via `RUSTFLAGS=-Dwarnings`.
Locally, it's up to you — most people work with warnings as warnings and
rely on `just ci` to catch them.

## Formatting

Plain `cargo fmt --all` with **no** `rustfmt.toml`. The defaults for
edition 2024 are what we use. `just fmt-check` in CI is the enforcement
point.

## Build and test flags

The baseline, used by the root justfile:

```sh
cargo check --workspace --all-targets --all-features
cargo build --workspace --all-targets --all-features
cargo test  --workspace --all-features
```

`--all-features` is deliberate. Feature combinations that don't compile
together are a bug; fix them or mark the features as mutually exclusive at
the manifest level.

Per-crate recipes that scope down use `-p <crate>` rather than changing
flags.

Benches are Criterion. They live under `benches/` in each crate and are run
by `just bench-*` recipes — those are crate-specific, because what a
meaningful benchmark looks like depends on the crate.

## Docs

```sh
RUSTDOCFLAGS="--cfg docsrs -Dwarnings" cargo +nightly doc --workspace --all-features --no-deps
```

Wrapped as `just doc`. Nightly is required for the `docsrs` cfg and for
scrape-examples. Broken intra-doc links or rustdoc warnings fail the
build — that's intentional; the docs are a feature.

## Miri

```sh
cargo +nightly miri test -p cityjson-types <module>
```

with `MIRIFLAGS=-Zmiri-strict-provenance`. Only runs for the `cityjson-types`
core crate, and only across modules that touch `unsafe` (boundary, vertex,
vertices, handles, raw_access, geometry). `just miri` runs the lot.

If you add `unsafe` anywhere, extend `just miri` to cover it.

## justfile recipes

Every crate should expose these names, either as its own recipe or by
delegating to the workspace justfile. Naming is the contract; what the
recipe does inside is the crate's business.

| Recipe      | What it does                                                                       |
|-------------|------------------------------------------------------------------------------------|
| `check`     | `cargo check` over the relevant scope with `--all-targets --all-features`          |
| `build`     | `cargo build` over the relevant scope with `--all-targets --all-features`          |
| `lint`      | `cargo clippy` — flags come from `[workspace.lints]`, not the recipe               |
| `fmt`       | `cargo fmt --all`                                                                  |
| `fmt-check` | `cargo fmt --all --check`                                                          |
| `test`      | `cargo test` with `--all-features`                                                 |
| `doc`       | Nightly docsrs build (usually delegated to workspace)                              |
| `ci`        | `fmt-check` + `lint` + `check` + `test` + `doc` — the one command that has to pass |

Optional recipes, used where they apply — stick to these names when you do
add them:

- `bench-*` — Criterion benches; name by what they measure (`bench-read`,
  `bench-write`, `bench-index`).
- `perf` — one-shot perf profile (pprof/dhat/etc.).
- `miri` — as above.
- `ffi` — FFI build/test helper for crates with FFI subcrates.
- `test-python` / `build-python` — run tox / build wheels for the two
  Python-shipping crates.

## Python packaging

`cityjson-lib` and `cityjson-index` each ship a Python wheel. Their
`ffi/python/pyproject.toml` files follow the same shape:

```toml
[build-system]
requires = ["setuptools>=70.1", "wheel>=0.43"]
build-backend = "setuptools.build_meta"

[project]
name = "<crate-name>"
version = "<synced from Cargo via cargo-release>"
requires-python = ">=3.11"
license = "MIT OR Apache-2.0"
license-files = ["LICENSE", "LICENSE-APACHE"]
classifiers = [
    "Development Status :: 4 - Beta",
    "Intended Audience :: Developers",
    "Intended Audience :: Science/Research",
    "Operating System :: POSIX :: Linux",
    "Operating System :: MacOS",
    "Operating System :: Microsoft :: Windows",
    "Programming Language :: Python :: 3.11",
    "Programming Language :: Python :: 3.12",
    "Programming Language :: Python :: 3.13",
    "Topic :: Scientific/Engineering :: GIS",
]

[dependency-groups]
dev = ["tox>=4", "tox-uv>=1"]

[tool.uv]
default-groups = ["dev"]
```

`tox.toml` uses the `wheel` env (installs the built wheel and runs the test
suite against it — no editable installs; the wheel is what users get):

```toml
requires = ["tox>=4", "tox-uv>=1"]
env_list = ["wheel"]

[env_run_base]
commands = [["python", "-m", "unittest", "tests.test_api"]]

[env.wheel]
package = "wheel"
```

### cibuildwheel

Published wheels are built with `cibuildwheel@v2.21` from `release.yml`.
The matrix is shared between both Python crates:

- `CIBW_BUILD: "cp311-*"` (Python 3.11+, CPython only)
- `CIBW_SKIP: "*-musllinux* *_i686 *_ppc64le *_s390x *_aarch64"`
- `CIBW_ARCHS_LINUX: "x86_64"`
- `CIBW_ARCHS_MACOS: "x86_64 arm64"`
- `CIBW_ARCHS_WINDOWS: "AMD64"`
- Linux installs a stable Rust toolchain before the build via
  `CIBW_BEFORE_ALL_LINUX`.

Don't widen the matrix (musl, 32-bit, exotic archs) without checking that
the corresponding Rust targets compile cleanly.

### Python version lockstep

Python package versions track the Rust workspace version. `cargo-release`
substitutes the version into `ffi/python/pyproject.toml` during a release
via `[package.metadata.release.pre-release-replacements]` in the owning
crate's `Cargo.toml`. Don't hand-edit the Python `version` field.

## README contract for crates

Each crate README should have — in this order — something close to:

1. Title + badges (crates.io, docs.rs, and PyPI if applicable).
2. One-paragraph description.
3. Install / quick start.
4. Usage examples.
5. Features table (if the crate has Cargo features).
6. MSRV line.
7. Link to crate-specific dev notes if they exist (`docs/development.md`
   inside the crate — only for things that are genuinely crate-local,
   like benchmark env vars).
8. Contributing — a single short section pointing here:

   ```markdown
   ## Contributing

   This crate follows the workspace contract. See
   [`CONTRIBUTING.md`](../../CONTRIBUTING.md) for PR guidelines and
   [`docs/development.md`](../../docs/development.md) for tooling,
   lints, and release flow.
   ```

   Add a bullet under that heading only when the crate deviates (extra
   recipes, extra env vars, a relaxed license, etc.).

9. License.

The individual-crate "Use of AI in this project" sections have been
consolidated into `CONTRIBUTING.md` — don't re-introduce them per crate.

## CI

`.github/workflows/ci.yml` runs:

- **Pull requests** — full matrix. Every crate and every Python build runs
  regardless of which files changed. The PR gate is non-negotiable.
- **Pushes to `main`** — selective. A leading `affected` job inspects the
  diff range and emits the set of crates to test. Downstream jobs (`test`,
  `test-python`, `lint`, `doc`, `build-msrv`, `miri`) consume that set.
  Docs-only pushes (Markdown, `LICENSE*`, `CHANGELOG.md`, `docs/`) skip
  everything except `fmt`.

The classifier lives in `.github/scripts/affected-crates.sh`. It reads the
diff range from `GITHUB_EVENT_BEFORE` / `GITHUB_SHA` and walks the
dependency graph to expand each changed crate into its downstream closure:

```
cityjson         → + cityjson-json, cityjson-arrow, cityjson-parquet,
                     cityjson-lib, cityjson-fake, cityjson-index
cityjson-json    → + cityjson-lib, cityjson-fake, cityjson-index
cityjson-arrow   → + cityjson-parquet, cityjson-lib, cityjson-fake, cityjson-index
cityjson-parquet → + cityjson-lib, cityjson-fake, cityjson-index
cityjson-lib     → + cityjson-fake, cityjson-index
cityjson-fake    → cityjson-fake
cityjson-index   → cityjson-index
```

Workspace-level changes (root `Cargo.toml`, `Cargo.lock`,
`rust-toolchain.toml`, `justfile`, `release.toml`, the CI workflow itself,
or `.github/scripts/`) trigger the full suite. So does any path the
classifier doesn't recognise (conservative default).

**Adding a new crate.** Edit `affected-crates.sh` in two places:

1. Append the crate name to `ALL_CRATES` (and `PYTHON_CRATES` if it ships
   Python bindings).
2. Add a `CLOSURE[<name>]=...` line listing the crate plus all crates that
   depend on it transitively. Update the closures of its upstream crates
   too — this is the step that's easy to miss.

You can exercise the script locally:

```sh
GITHUB_EVENT_NAME=push \
GITHUB_EVENT_BEFORE=$(git rev-parse HEAD~1) \
GITHUB_SHA=$(git rev-parse HEAD) \
bash .github/scripts/affected-crates.sh
```

It prints `matrix=…`, `any=…`, `run_python=…` to stdout.

**Release flow interaction.** `release.yml` (tag-triggered) does **not**
re-run the full test suite. It checks that `ci.yml` succeeded on the
tagged commit via `gh run list` and fails fast otherwise. Because
`cargo release` pushes `main` before tagging, CI has already run on that
exact commit. If you want to release a commit that CI hasn't seen, push
it to `main` and wait for green first.

## Release

From a clean `main`:

```sh
cargo release patch --execute    # or minor / major
```

`cargo-release` bumps every crate in lockstep (`shared-version = true`),
creates one commit (`consolidate-commits = true`), tags `v<x.y.z>`, pushes,
and publishes to crates.io in dependency order. The tag push triggers
`release.yml`, which builds and publishes the Python wheels.

Two manual steps before you run `cargo release`:

1. Promote the `## [Unreleased]` section of `CHANGELOG.md` to
   `## [x.y.z] — <date>`. `cargo-release` 1.1 can't do this for us because
   its replacement mechanism is per-manifest and CHANGELOG is at the root.
2. Sanity-check `git status` is clean and `just ci` is green.

Don't run `cargo release` from a branch; `allow-branch = ["main"]` will
reject it, which is the point.

## Verification — how to check your setup

Everything below should run clean before you open a PR:

```sh
just ci                                           # fmt-check + lint + check + test + doc
just miri                                         # if you touched unsafe
just test-python                                  # if you touched cityjson-lib or cityjson-index
```

If `just ci` fails on `doc`, you're probably missing a nightly toolchain:

```sh
rustup toolchain install nightly
```

If tests complain about a missing corpus, set
`CITYJSON_SHARED_CORPUS_ROOT` to a local checkout of
[`cityjson-corpus`](https://github.com/3DGI/cityjson-corpus), or point at
individual indices as documented in the relevant crate's dev notes.

The CI jobs in `.github/workflows/ci.yml` are the source of truth for what
"passing" means — if something is green locally but red in CI, CI wins and
we fix the local story.
