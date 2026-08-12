FROM rust:1.97.0-bookworm AS rust-base

WORKDIR /app

FROM rust-base AS wasm-toolchain

ARG WASM_BINDGEN_CLI_VERSION=0.2.123

RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version "${WASM_BINDGEN_CLI_VERSION}" --locked

FROM rust-base AS server-builder

# Git metadata is not needed during compilation; deploy metadata is injected into
# the runtime image below so Rust artifacts stay reusable across commits.
COPY server/Cargo.toml server/Cargo.lock ./server/
COPY server/crates ./server/crates
COPY server/src ./server/src
COPY server/assets ./server/assets
COPY server/migrations ./server/migrations

WORKDIR /app/server
RUN cargo build --release --locked -p rts-server --bin rts-server

FROM wasm-toolchain AS wasm-builder

COPY server/Cargo.toml server/Cargo.lock ./server/
COPY server/crates ./server/crates
COPY server/src ./server/src
COPY server/assets ./server/assets
COPY server/migrations ./server/migrations
COPY scripts/build-sim-wasm.sh ./scripts/build-sim-wasm.sh

RUN RTS_SIM_WASM_OUT_DIR=/app/sim-wasm-out ./scripts/build-sim-wasm.sh \
    && test -s ./sim-wasm-out/rts_sim_wasm.js \
    && test -s ./sim-wasm-out/rts_sim_wasm_bg.wasm

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates linux-perf \
    && perf --version \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/server

COPY --from=server-builder /app/server/target/release/rts-server /usr/local/bin/rts-server
COPY --from=server-builder /app/server/assets ./server/assets
COPY client ./client
COPY --from=wasm-builder /app/sim-wasm-out/rts_sim_wasm.js ./client/vendor/sim-wasm/rts_sim_wasm.js
COPY --from=wasm-builder /app/sim-wasm-out/rts_sim_wasm_bg.wasm ./client/vendor/sim-wasm/rts_sim_wasm_bg.wasm
COPY docs/context ./docs/context
COPY docs/design ./docs/design

RUN test -s ./client/vendor/sim-wasm/rts_sim_wasm.js \
    && test -s ./client/vendor/sim-wasm/rts_sim_wasm_bg.wasm \
    && test -s ./client/assets/snapshot-streams/fixed-roster-hellhole.rtsstream \
    && test -s ./client/assets/rigs/anti-tank-gun-noshield-lowdetail/anti-tank-gun-noshield-lowdetail-white-v1-alpha.png \
    && test -s ./client/assets/rigs/artillery-a19-pass-03/generated/artillery-a19-components-pass-03-alpha.png \
    && test -s ./client/assets/rigs/building-emblems-preview/barracks-atlas-m14-team-tint.png \
    && test -s ./client/assets/rigs/building-emblems-preview/factory-atlas-team-tint.png \
    && test -s ./client/assets/rigs/building-emblems-preview/engineering_complex-atlas-team-tint.png \
    && test -s ./client/assets/rigs/building-emblems-preview/steelworks-atlas-team-tint.png \
    && test -s ./client/assets/rigs/building-emblems-preview/training_centre-atlas-mg42-panzerfaust-team-tint.png \
    && test -s ./client/assets/rigs/extractor-animation-poc/pump-jack-atlas.png \
    && test -s ./client/assets/rigs/steel-mine-jackhammer/jackhammer-1940s-white.png \
    && test -s ./client/assets/rigs/buildings-b3-corrected-preview/factory-atlas.png \
    && test -s ./client/assets/rigs/buildings-b4-selected-pass-01/engineering_complex-atlas.png \
    && test -s ./client/assets/rigs/buildings-b4-selected-pass-01/steelworks-atlas.png \
    && test -s ./client/assets/rigs/buildings-b7-team-paint-refined-preview/barracks-atlas.png \
    && test -s ./client/assets/rigs/buildings-b7-team-paint-refined-preview/training_centre-atlas.png \
    && test -s ./client/assets/rigs/resource-depot-worksite-preview/resource_depot-atlas.png \
    && test -s ./client/assets/rigs/command-car-packed-radio-preview/generated/command-car-packed-radio-stars-30-atlas-v4.png \
    && test -s ./client/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png \
    && test -s ./client/assets/rigs/mortar-png-pass-01/generated/mortar-m2-wheeled-pass-01-alpha.png \
    && test -s ./client/assets/rigs/mortar-png-pass-04/generated/mortar-m2-wheeled-baseplate-pass-04-alpha.png \
    && test -s ./client/assets/rigs/rifleman-pass-02/generated/rifleman-pass-02-recoil-strip.png \
    && test -s ./client/assets/rigs/rifleman-pass-02/generated/rifleman-down-rifle-iteration/rifleman-down-rifle-strip.png \
    && test -s ./client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/recoil-pass-01/rifleman-recoil-review-strip.png \
    && test -s ./client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/recoil-pass-01/rifleman-panzerfaust-windup-runtime-strip.png \
    && test -s ./client/assets/rigs/scout-car-white-pass-01/generated/scout-car-white-atlas.png \
    && test -s ./client/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-alpha.png \
    && test -s ./client/assets/rigs/tank-ps1/tank-atlas.png \
    && test -s ./client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-11-white-alpha.png

ENV RTS_ADDR=0.0.0.0:8080
ENV RUST_LOG=info
ARG COMMIT_HASH
ENV COMMIT_HASH=${COMMIT_HASH}

EXPOSE 8080

CMD ["rts-server"]
