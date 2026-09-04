# Reproducible Linux build and test environment, mirroring .github/workflows/ci.yml.
#
#   docker build -t schemamama_rusqlite .          # runs the full check on build
#   docker run --rm -it schemamama_rusqlite        # shell in the same environment
#   docker build --build-arg RUST_VERSION=1.85 .   # pin an older stable
#
# RUST_VERSION selects a tag of the official `rust` image, which ships stable
# only. CI covers beta; this image does not.
#
# This is a development tool. The crate is a library, so there is no runtime
# artifact to ship and no second stage.

ARG RUST_VERSION=1
FROM rust:${RUST_VERSION}-bookworm

# rusqlite links the system SQLite; the `bundled` feature is not enabled.
RUN apt-get update \
 && apt-get install -y --no-install-recommends libsqlite3-dev \
 && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

WORKDIR /src

# Fetch dependencies in their own layer so edits under src/ don't refetch them.
COPY Cargo.toml ./
RUN mkdir -p src && touch src/lib.rs && cargo fetch

COPY . .

# The same gates CI enforces, in the same order.
ENV RUSTFLAGS="-D warnings"
RUN cargo fmt --all --check \
 && cargo clippy --all-targets -- -D warnings \
 && cargo build \
 && cargo test \
 && cargo doc --no-deps

CMD ["/bin/bash"]
