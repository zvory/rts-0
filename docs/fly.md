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
flyctl scale count 0
```

To bring it back:

```bash
flyctl scale count 1
```
