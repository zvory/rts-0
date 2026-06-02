# Fly.io deploy

This app runs as one Rust process that serves the static client and upgrades `/ws` to a WebSocket.
Fly proxies HTTPS and WSS traffic to the container on port 8080.

## First deploy

```bash
flyctl auth login
flyctl apps create rts-0-zvorygin
flyctl deploy --ha=false
```

If the app name is already taken, choose another globally unique name and update `app` in
`fly.toml` before running `flyctl apps create`.

After deploy, open:

```text
https://rts-0-zvorygin.fly.dev
```

Run one machine only. Game rooms live in server memory, so multiple machines can split players
between different lobbies.

## Stop spending after game night

```bash
flyctl scale count 0 -a rts-0-zvorygin
```

To bring it back:

```bash
flyctl scale count 1 -a rts-0-zvorygin
```

The `-a rts-0-zvorygin` flag makes these commands work from any directory. If you are already in
the repo directory with `fly.toml`, the `-a` flag is optional.

## Redeploy after changes

From the repo root, run:

```bash
flyctl deploy --ha=false
```

That updates the live Fly app with the current branch state. No GitHub push is required for the
deployment itself.

## Datadog lag telemetry

The server always writes structured lag logs to stdout. On Fly, stdout is captured as app logs; to
ship those logs to Datadog, run Fly's Log Shipper in the same Fly organization and configure its
Datadog sink with a Datadog API key.

The app can also emit custom metrics without a local Datadog Agent. Set these secrets on the Fly
app:

```bash
fly secrets set DD_API_KEY=<datadog_api_key> DD_SITE=datadoghq.com -a rts-0-zvorygin
```

`DD_SITE` defaults to `datadoghq.com`; use the site for the Datadog account, for example
`datadoghq.eu` when appropriate. `fly.toml` enables `RTS_DATADOG_METRICS=1` only for the Fly app.
Local development does not send direct Datadog API metrics unless both `RTS_DATADOG_METRICS=1` and
`DD_API_KEY` are explicitly set in the local environment.

If a Datadog Agent or DogStatsD endpoint is available instead, configure:

```bash
fly secrets set RTS_DOGSTATSD_ADDR=<host>:8125 -a rts-0-zvorygin
```

`RTS_DOGSTATSD_ADDR` is treated as an explicit local or deploy-time opt-in.

Metric names:

| Metric | Type | Meaning |
|--------|------|---------|
| `rts.tick.duration_ms` | gauge/timer | Time spent in one authoritative `Game::tick()` call. |
| `rts.tick.slow` | count | Tick exceeded `RTS_SLOW_TICK_MS` (default: one tick budget). |
| `rts.snapshot.fanout_duration_ms` | gauge/timer | Time spent building/sending snapshots for one room tick. |
| `rts.snapshot.recipients` | gauge | Connected snapshot recipients for a room tick. |
| `rts.snapshot.entities_sent` | gauge | Total entity views sent in that room tick. |
| `rts.connections.active` | gauge | Open WebSocket connections. |
| `rts.players.connected` | gauge | Players currently seated in rooms. |
| `rts.rooms.active` | gauge | Rooms with at least one connected player. |
| `rts.matches.active` | gauge | Matches currently running. |
| `rts.outbound.dropped` | count | Nonblocking outbound send dropped a message (`reason:full` or `reason:closed`). |
| `rts.client.*` | gauge/count | Browser FPS, frame time, snapshot gap, RTT, and slow-frame reports. |

Useful log queries once Fly logs are shipped to Datadog:

```text
service:rts "excessively slow tick"
service:rts "client lag report"
service:rts "outbound queue full"
```

To tune thresholds:

```bash
fly secrets set RTS_SLOW_TICK_MS=33 RTS_SLOW_CLIENT_FPS=45 RTS_SLOW_CLIENT_RTT_MS=250 -a rts-0-zvorygin
```
