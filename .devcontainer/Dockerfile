FROM mcr.microsoft.com/devcontainers/rust:1-trixie

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        g++ \
        just \
        libproj-dev \
        libsqlite3-dev \
        libtiff-dev \
        pkg-config \
        proj-bin \
        sqlite3 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
