# skillhub-registry container — multi-stage build.
#
# Artifact hygiene (DEC-0009 / Phase 9 guard): the runtime DB (`*.db`), the HMAC signing
# secret, and per-owner signing keys are NEVER baked into the image. The image ships only the
# compiled binary. Secrets are injected at runtime via environment variables; the DB is a
# mounted volume.

# --- builder ---
FROM rust:1.96-bookworm AS builder
WORKDIR /build
# the manifest + source + embedded schema (via include_str!) are all required to compile
COPY apps/skillhub-registry/Cargo.toml apps/skillhub-registry/Cargo.toml
COPY apps/skillhub-registry/src apps/skillhub-registry/src
COPY shared/schemas shared/schemas
RUN cargo build --release --manifest-path apps/skillhub-registry/Cargo.toml

# --- runtime ---
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/apps/skillhub-registry/target/release/skillhub-registry /usr/local/bin/skillhub-registry

# writable, empty DB volume mounted at runtime (never baked in)
ENV SKILLHUB_REGISTRY_DB=/data/skillhub.db \
    SKILLHUB_REGISTRY_BIND=0.0.0.0 \
    SKILLHUB_REGISTRY_PORT=8787
RUN mkdir -p /data && chown -R nobody:nogroup /data
VOLUME ["/data"]
USER nobody
EXPOSE 8787
# NOTE: bind 0.0.0.0 inside the container is safe because a TLS-terminating reverse proxy sits
# in front (see README Security / TLS termination). SKILLHUB_REGISTRY_SECRET must be provided.
ENTRYPOINT ["/usr/local/bin/skillhub-registry"]
