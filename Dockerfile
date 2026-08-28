# Runtime image for the torimemo server.
#
# One binary serves every role — `serve`, `enrich`, `embed`, `label` — so the
# API and the offline workers can never drift onto different code. The
# deployment overrides the command per role; the default is the API, since
# that is the only long-running one.
#
# The embedding model is **not** baked in. fastembed fetches it on first use
# into FASTEMBED_CACHE_PATH, which the deployment mounts as a volume: baking
# 127MB of weights into the image would triple its size and pin the model
# version to the image tag, so re-embedding with a newer model would require a
# rebuild rather than a config change.

FROM rust:1.96-slim AS build

# `bundled` builds SQLite from source, so no libsqlite3-dev is needed; the TLS
# stack is rustls, so no libssl-dev either. What remains is what cc-rs needs to
# compile the bundled C.
RUN apt-get update && \
    apt-get install --no-install-recommends --assume-yes build-essential && \
    rm --recursive --force /var/lib/apt/lists/*

WORKDIR /build

# Manifests first, so a source-only change does not re-resolve the graph.
# Every crate needs a stub source file or cargo refuses to plan the build, and
# a `[[bench]]` target needs its file to exist even when nothing builds it.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/api/Cargo.toml crates/api/
COPY crates/classify/Cargo.toml crates/classify/
COPY crates/cli/Cargo.toml crates/cli/
COPY crates/core/Cargo.toml crates/core/
COPY crates/embed/Cargo.toml crates/embed/
COPY crates/enrich/Cargo.toml crates/enrich/
RUN for crate in api classify cli core embed enrich; do \
        mkdir -p "crates/$crate/src" && \
        echo "" > "crates/$crate/src/lib.rs"; \
    done && \
    echo "fn main() {}" > crates/cli/src/main.rs && \
    mkdir -p crates/embed/benches && \
    echo "fn main() {}" > crates/embed/benches/recall.rs && \
    cargo build --release --bin torimemo && \
    rm --recursive --force crates

COPY crates ./crates
# Cargo caches by mtime, and the stubs above are newer than the real sources
# COPY just laid down; without this the real code is not rebuilt.
RUN touch crates/*/src/*.rs && cargo build --release --bin torimemo


FROM debian:trixie-slim

# ca-certificates is required, not decorative: `enrich` fetches page metadata
# and the labelling model over TLS, and rustls verifies against the system
# roots.
RUN apt-get update && \
    apt-get install --no-install-recommends --assume-yes ca-certificates && \
    rm --recursive --force /var/lib/apt/lists/* && \
    groupadd --gid 10001 torimemo && \
    useradd --uid 10001 --gid torimemo --home-dir /data --no-create-home torimemo

COPY --from=build /build/target/release/torimemo /usr/local/bin/torimemo

# Both are volumes in the deployment: the archive is the data, and the model
# cache is a 127MB download that must survive a restart.
RUN mkdir --parents /data /models && chown torimemo:torimemo /data /models

ENV TORIMEMO_DB=/data/torimemo.db \
    FASTEMBED_CACHE_PATH=/models

WORKDIR /data
USER torimemo

EXPOSE 7645

# `serve` binds every interface only once a service token exists, so a
# container started against an archive with no token will come up unreachable
# on purpose. Mint one into the data volume before deploying:
#
#   docker run --rm -v torimemo-data:/data IMAGE torimemo token issue --name odin
CMD ["torimemo", "serve"]
