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
        jq \
        just \
        libproj-dev \
        libsqlite3-dev \
        libtiff-dev \
        pkg-config \
        proj-bin \
        sqlite3 \
        valgrind \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

RUN rustup toolchain install stable \
        --component rustfmt \
        --component clippy \
    && rustup target add --toolchain stable wasm32-unknown-unknown \
    && rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && rustup toolchain install nightly \
        --component rustfmt \
        --component clippy \
        --component miri

RUN UV_INSTALL_DIR=/usr/local/bin sh -c "$(curl -LsSf https://astral.sh/uv/install.sh)"

RUN cargo install cbindgen --version 0.29.2 --locked --root /usr/local \
    && cbindgen --version

RUN cargo install cargo-tarpaulin --locked --root /usr/local
