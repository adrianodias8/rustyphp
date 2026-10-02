# Fork dev/measurement image (ours, not upstream's): oracle PHP + pinned Rust
# toolchain + profiling tools in one place, so nothing is installed on the host.
#
# Oracle is pinned to the exact patch release upstream verified against
# (PHP 8.5.7); ../php-src must be checked out at the matching php-8.5.7 tag.
ARG PHP_VERSION=8.5.7
FROM composer:2 AS composer

FROM php:${PHP_VERSION}-cli

# System libs: zlib/gd/tidy/xslt are linked by ferro via FFI (same list as
# upstream's .github/workflows/ci.yml); the rest is build + measurement tooling.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config cmake clang git curl ca-certificates unzip \
        zlib1g-dev libgd-dev libtidy-dev libxslt1-dev libssl-dev libsqlite3-dev \
        time linux-perf procps python3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=composer /usr/bin/composer /usr/local/bin/composer

# Toolchain pinned by php-rust/rust-toolchain.toml (upstream: never change it
# during a measurement arc).
ARG RUST_TOOLCHAIN=1.98.1
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
        --default-toolchain ${RUST_TOOLCHAIN} --component clippy \
    && cargo install --locked inferno samply \
    && rm -rf /opt/cargo/registry /opt/cargo/git

# perf 6.12 (Debian) leaves Rust v0 symbols mangled; rustfilt demangles the
# folded stacks. Separate layer so adding it did not rebuild the toolchain.
RUN cargo install --locked rustfilt \
    && rm -rf /opt/cargo/registry /opt/cargo/git

# Build artefacts live on a named volume, never in the bind-mounted repo
# (upstream rule: no in-repo target/).
ENV CARGO_TARGET_DIR=/target \
    PHP_ORACLE=/usr/local/bin/php
WORKDIR /work/php-rust/php-rust

# Session 5: wrk for the worker-mode benchmarks (bench/worker/bench-wrk.sh).
RUN apt-get update && apt-get install -y --no-install-recommends wrk \
    && rm -rf /var/lib/apt/lists/*

# Session 7: ext/gd in the oracle — Drupal requires it, and ferro implements
# gd, so the oracle must have it for parity checks. Bundled libgd, with the
# formats Drupal's image toolkit uses.
RUN apt-get update && apt-get install -y --no-install-recommends \
        libpng-dev libjpeg-dev libfreetype-dev libwebp-dev \
    && docker-php-ext-configure gd --with-freetype --with-jpeg --with-webp \
    && docker-php-ext-install -j"$(nproc)" gd \
    && rm -rf /var/lib/apt/lists/*

# Session 10: heaptrack for the per-worker heap profile (MEMORY_DRUPAL.md),
# run on a `--features system-alloc` build (mimalloc bypasses malloc).
RUN apt-get update && apt-get install -y --no-install-recommends heaptrack \
    && rm -rf /var/lib/apt/lists/*
