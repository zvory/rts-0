FROM rust:1-bookworm AS builder

WORKDIR /app

ARG WASM_BINDGEN_CLI_VERSION=0.2.123

RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version "${WASM_BINDGEN_CLI_VERSION}" --locked

# Git metadata is not needed during compilation; deploy metadata is injected into
# the runtime image below so Rust artifacts stay reusable across commits.
COPY server/Cargo.toml server/Cargo.lock ./server/
COPY server/crates ./server/crates
COPY server/src ./server/src
COPY server/assets ./server/assets
COPY server/migrations ./server/migrations
COPY client ./client
COPY scripts/build-sim-wasm.sh ./scripts/build-sim-wasm.sh

RUN ./scripts/build-sim-wasm.sh \
    && test -s ./client/vendor/sim-wasm/rts_sim_wasm.js \
    && test -s ./client/vendor/sim-wasm/rts_sim_wasm_bg.wasm

WORKDIR /app/server
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
COPY docs/context ./docs/context
COPY docs/design ./docs/design

ENV RTS_ADDR=0.0.0.0:8080
ENV RUST_LOG=info
ARG COMMIT_HASH
ENV COMMIT_HASH=${COMMIT_HASH}

EXPOSE 8080

CMD ["rts-server"]
