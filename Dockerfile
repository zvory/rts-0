FROM rust:1-bookworm AS builder

WORKDIR /app

COPY server/Cargo.toml server/Cargo.lock ./server/
COPY server/src ./server/src
COPY client ./client

WORKDIR /app/server
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/server

COPY --from=builder /app/server/target/release/rts-server /usr/local/bin/rts-server
COPY --from=builder /app/client ./client

ENV RTS_ADDR=0.0.0.0:8080
ENV RUST_LOG=info

EXPOSE 8080

CMD ["rts-server"]
