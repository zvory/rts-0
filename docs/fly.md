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
./deploy.sh mainline
```

If the app name is already taken, choose another globally unique name and pass it with
`./deploy.sh mainline --app <your-app>`.

After deploy, open:

```text
https://rts-0-zvorygin.fly.dev
```

## Release channels

The production/mainline channel uses the existing Fly app:

```text
rts-0-zvorygin
```

The beta channel is a second Fly app. Create it once before the first beta deploy:

```bash
flyctl apps create rts-0-zvorygin-beta
./deploy.sh beta
```

After deploy, open:

```text
https://rts-0-zvorygin-beta.fly.dev
```

If the beta app name is taken, choose another globally unique name and deploy with:

```bash
FLY_BETA_APP=<your-beta-app> ./deploy.sh beta
```

or:

```bash
./deploy.sh beta --app <your-beta-app>
```

Run one machine only. Game rooms live in server memory, so multiple machines can split players
between different lobbies.

Beta deploys set the machine size to `shared-cpu-4x@1024MB`, matching the mainline app. If you
override the app name for a different beta app, `./deploy.sh beta` still applies that VM size.

## Stop spending after game night

```bash
flyctl scale count 0 -a rts-0-zvorygin
flyctl scale count 0 -a rts-0-zvorygin-beta
```

To bring it back:

```bash
flyctl scale count 1 -a rts-0-zvorygin
flyctl scale count 1 -a rts-0-zvorygin-beta
```

The `-a rts-0-zvorygin` flag makes these commands work from any directory. If you are already in
the repo directory with `fly.toml`, the `-a` flag is optional.

## Redeploy after changes

From the repo root, run:

```bash
./deploy.sh mainline
```

That updates the live Fly app with the current checkout. No GitHub push is required for the
deployment itself.

To deploy beta from the current checkout:

```bash
./deploy.sh beta
```

To deploy a particular commit, pass a git revision. The script creates a temporary detached
worktree for that revision, deploys it, and removes the worktree afterward so your current checkout
does not move.

```bash
./deploy.sh mainline 5a29d29
./deploy.sh beta 5a29d29
```

The deployed commit is passed into the Docker build as `COMMIT_HASH`, so `/version` and client
asset cache-busting reflect the selected revision.

## Custom domains

Raw Fly URLs work without extra setup:

```text
https://rts-0-zvorygin.fly.dev
https://rts-0-zvorygin-beta.fly.dev
```

For `beta.bewegungskrieg.net`, add the hostname to the beta app and then create the DNS record that
Fly prints:

```bash
flyctl certs add beta.bewegungskrieg.net -a rts-0-zvorygin-beta
flyctl certs show beta.bewegungskrieg.net -a rts-0-zvorygin-beta
```

Serving beta at `https://bewegungskrieg.net/beta` would require an HTTP reverse proxy or redirect
layer in front of the apps because Fly app routing is hostname-based, not path-based. Prefer
`beta.bewegungskrieg.net` unless a path URL is required.
