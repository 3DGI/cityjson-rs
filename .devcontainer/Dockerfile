FROM mcr.microsoft.com/devcontainers/rust:1-trixie

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        curl \
        g++ \
        gh \
        jq \
        just \
        libproj-dev \
        libsqlite3-dev \
        libtiff-dev \
        pkg-config \
        proj-bin \
        ripgrep \
        sqlite3 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && rustup toolchain install nightly \
        --component rustfmt \
        --component clippy \
        --component miri

RUN UV_INSTALL_DIR=/usr/local/bin sh -c "$(curl -LsSf https://astral.sh/uv/install.sh)"

RUN cargo install cbindgen cargo-tarpaulin --locked --root /usr/local
