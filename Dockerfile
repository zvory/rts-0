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

RUN test -s ./client/assets/snapshot-streams/supply-300-hellhole.rtsstream \
    && test -s ./client/assets/rigs/anti-tank-gun-noshield-lowdetail/anti-tank-gun-noshield-lowdetail-white-v1-alpha.png \
    && test -s ./client/assets/rigs/artillery-a19-pass-02/generated/artillery-a19-components-pass-02-alpha-debug.png \
    && test -s ./client/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png \
    && test -s ./client/assets/rigs/mortar-png-pass-01/generated/mortar-m2-wheeled-pass-01-alpha.png \
    && test -s ./client/assets/rigs/mortar-png-pass-04/generated/mortar-m2-wheeled-baseplate-pass-04-alpha.png \
    && test -s ./client/assets/rigs/rifleman-pass-02/generated/rifleman-pass-02-recoil-strip.png \
    && test -s ./client/assets/rigs/rifleman-pass-02/generated/rifleman-down-rifle-iteration/rifleman-down-rifle-strip.png \
    && test -s ./client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/no-pack/rifleman-no-pack-white-review-strip.png \
    && test -s ./client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/panzerfaust-composited/rifleman-panzerfaust-composited-strip.png \
    && test -s ./client/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-atlas.png \
    && test -s ./client/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-alpha.png \
    && test -s ./client/assets/rigs/tank-ps1/tank-atlas.png

WORKDIR /app/server
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates linux-perf \
    && perf --version \
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
