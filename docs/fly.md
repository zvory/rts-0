# Fly.io deploy

This app runs as one Rust process that serves the static client and upgrades `/ws` to a WebSocket.
Fly proxies HTTPS and WSS traffic to the container on port 8080.

`fly.toml` enables basic production performance tracing with `RTS_PERF=spikes` and
`RTS_PERF_SLOW_TICK_MS=40`. Fly logs will include a `performance tick summary` row only when a
server tick takes at least 40 ms.

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
