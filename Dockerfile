FROM rust:1-bookworm AS builder

WORKDIR /app

# Git metadata is copied into the builder so build.rs can resolve the commit hash
# at compile time. It stays in the builder layer and is never shipped to runtime.
COPY .git .git
COPY server/Cargo.toml server/Cargo.lock server/build.rs ./server/
COPY server/src ./server/src
COPY server/assets ./server/assets
COPY client ./client

WORKDIR /app/server
ARG COMMIT_HASH
ENV COMMIT_HASH=${COMMIT_HASH}
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/server

COPY --from=builder /app/server/target/release/rts-server /usr/local/bin/rts-server
COPY --from=builder /app/server/assets ./server/assets
COPY --from=builder /app/client ./client

ENV RTS_ADDR=0.0.0.0:8080
ENV RUST_LOG=info

EXPOSE 8080

CMD ["rts-server"]
