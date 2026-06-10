#!/usr/bin/env just --justfile
# Root workspace recipes. Crate-specific recipes (bench, profile, ffi) remain
# under crates/<name>/justfile and can be invoked with:
#   just -f crates/<name>/justfile <recipe>

_default:
    just --list

# cargo clean across the workspace
clean:
    cargo clean

# cargo check, all targets, all features, across the workspace
check:
    cargo check --workspace --all-targets --all-features

# cargo build, all targets, all features, across the workspace
build *args:
    cargo build --workspace --all-targets --all-features {{args}}

# Strict clippy across the workspace (flags live in [workspace.lints])
lint:
    cargo clippy --workspace --all-targets --all-features

# cargo fmt across the workspace
fmt:
    cargo fmt --all

# Verify formatting
fmt-check:
    cargo fmt --all --check

# cargo test across the workspace
test:
    cargo test --workspace --all-features

# Build docs (nightly, docsrs cfg, deny warnings)
doc:
    RUSTDOCFLAGS="--cfg docsrs -Dwarnings" cargo +nightly doc --workspace --all-features --no-deps

# Miri on the cityjson-types crate's unsafe-touching test suites
miri:
    MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p cityjson-types boundary
    MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p cityjson-types vertex
    MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p cityjson-types vertices
    MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p cityjson-types handles
    MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p cityjson-types raw_access
    MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p cityjson-types geometry

# Run the Python binding test suites (tox smoke) for both crates
test-python:
    cd crates/cityjson-lib/ffi/python && uv run tox run
    cd crates/cityjson-index/ffi/python && uv run tox run

# Build the Python wheels for both crates
build-python:
    cd crates/cityjson-lib/ffi/python && uv build --wheel
    cd crates/cityjson-index/ffi/python && uv build --wheel

# Delegate to the FFI helpers in the workspace crates.
ffi *args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{args}}" in
      "build header"|"build cpp"*|"build wasm"*|"bench"|"bench "*)
        cd crates/cityjson-lib
        ./tools/ffi.sh {{args}}
        ;;
      "build core")
        cd crates/cityjson-index
        ./tools/ffi.sh {{args}}
        ;;
      "check"|"fmt"|"doc"|"clean"|"test"|"ci"|"build python"|"build python "*)
        cd crates/cityjson-lib
        ./tools/ffi.sh {{args}}
        cd ../../crates/cityjson-index
        ./tools/ffi.sh {{args}}
        ;;
      "test python")
        cd crates/cityjson-lib
        ./tools/ffi.sh build python
        cd ../../crates/cityjson-index
        ./tools/ffi.sh test python
        ;;
      *)
        echo "Unsupported root FFI arguments: {{args}}" >&2
        echo "Use crate-local helpers for crate-specific commands:" >&2
        echo "  cd crates/cityjson-lib && ./tools/ffi.sh --help" >&2
        echo "  cd crates/cityjson-index && ./tools/ffi.sh --help" >&2
        exit 1
        ;;
    esac

# Install the Starlight documentation POC dependencies.
docs-poc-install:
    cd docs-site && npm install
    cd docs-site && uv sync --locked

# Build the Starlight documentation POC and Pagefind index.
docs-poc-build:
    cd docs-site && npm run build

# Serve the built Starlight documentation POC locally.
docs-poc-serve *args:
    cd docs-site && npm run preview -- {{args}}

# Validate, build, and smoke-test the Starlight documentation POC.
docs-poc-check:
    cd docs-site && npm run check
    cd docs-site && npm run test

# Full local CI (fmt + lint + check + test + doc)
ci: fmt-check lint check test doc
