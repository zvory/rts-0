# Phase 07: Deployment And Rollout

Purpose: prove WebTransport works outside localhost and roll it out without removing WebSocket
fallback.

## Deployment Questions

The existing `docs/fly.md` says Fly proxies HTTPS and WSS traffic to the container on port 8080.
That is enough for WebSocket. WebTransport needs HTTP/3 over QUIC, which means UDP reachability and
TLS/ALPN behavior must be proven.

Questions for the deployment investigation:

- Can the deployed environment pass browser WebTransport traffic to the Rust process?
- Does it support UDP services on the desired public port?
- If the platform terminates TLS, can it proxy WebTransport sessions and datagrams upstream?
- If the Rust process terminates TLS itself, how will it get a valid certificate?
- Can the static site and WebTransport endpoint share an origin?
- Will CORS, certificate, or port restrictions make a second endpoint awkward?
- How does fallback to WebSocket behave when UDP is blocked?

Do not remove WebSocket until these are answered in production-like conditions.

## Client Rollout

Feature flags:

- `?transport=ws`: force WebSocket.
- `?transport=webtransport`: force WebTransport.
- default initially: WebSocket.
- later default: try WebTransport, fall back to WebSocket.

The client should expose enough debug state to know which transport is active. This can be a console
log or a small dev-only status field. Avoid permanent noisy UI unless the user asks for it.

## Server Metrics

Add metrics/logs that distinguish:

- WebSocket sessions;
- WebTransport sessions;
- fallback attempts;
- fallback successes;
- WebTransport setup failures;
- datagram snapshots sent;
- datagram snapshots skipped for size;
- stream fallback snapshots sent;
- reliable control messages sent;
- connection close reasons.

Without this, rollout will be guesswork.

## Rollout Stages

1. Local forced WebTransport.
2. Local default WebSocket with forced WebTransport available.
3. Deployed forced WebTransport on a test URL or query flag.
4. Deployed try-WebTransport-with-WebSocket-fallback for a small group.
5. Default WebTransport attempt with fallback.
6. Keep WebSocket fallback indefinitely unless the user explicitly decides otherwise.

## Tests

Keep running the current WebSocket suite:

```bash
tests/run-all.sh
```

Add deployed smoke checks:

- browser can connect using forced WebTransport;
- browser can connect using forced WebSocket;
- fallback works when WebTransport is blocked or unsupported;
- a match can start and receive snapshots;
- pings and close handling work;
- reconnect/reload does not strand a room.

## Done Criteria

- WebTransport works in a real deployed browser session.
- WebSocket fallback works in the same deployment.
- Metrics show transport choice and failure modes.
- The rollout can be disabled quickly by config or query flag.
- No PR or release notes claim WebTransport fixes stutter without measured before/after data.
