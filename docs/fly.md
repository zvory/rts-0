# Fly.io deploy

The game runs as one Rust process that serves the static client and upgrades `/ws` to a WebSocket.
Fly proxies HTTPS and WSS traffic to the container on port 8080.

`fly.mainline.toml` and `fly.beta.toml` enable basic production performance tracing with
`RTS_PERF=spikes` and `RTS_PERF_SLOW_TICK_MS=40`. Fly logs include a `performance tick summary`
only when a server tick takes at least 40 ms.

## Release channels

Each release channel has one Fly app and one game Machine. Both stop when idle and start on the
first request; there is no launcher or proxy app in front of either channel.

```text
rts-0-zvorygin       fly.mainline.toml  mainline; bewegungskrieg.net and www.bewegungskrieg.net
rts-0-zvorygin-beta  fly.beta.toml      beta; rts-0-zvorygin-beta.fly.dev
```

The mainline app owns the existing canonical certificates and Squarespace DNS records. Keep those
records pointed at it. The beta app remains directly available at its Fly hostname.

Run one Machine per app. Game rooms live in server memory, so multiple running Machines can split
players between different lobbies.

Both channels use one `performance-1x` Machine with 2 GB of memory,
`auto_stop_machines = "stop"`, autostart enabled, and zero minimum running Machines. Fly's HTTP
activity keeps a Machine running while browsers are connected. A headless AI room without a browser
may not prevent autostop; durable unattended AI games are not a goal of this setup.

The ordinary lobby page does not hold a WebSocket. It loads `/api/lobbies` once, then refreshes at
most every five seconds only while the tab is visible and has seen player activity in the last 30
seconds; a manual Refresh remains available. Recent Matches performs one request when mounted.
Create, Join, Watch, Replay, and explicit launch URLs open the WebSocket on demand. The client closes
an open pre-join connection when its tab is hidden. This keeps passive lobby tabs and scrapers from
sustaining polling or heartbeat traffic through the Machine's idle tail, although any individual
HTTP request can still autostart the Machine. Once connected, the server closes a browser after five
minutes without a player action in either a lobby or match; automatic heartbeat and network-report
traffic does not count. An ordinary lobby disconnect silently returns the client to the main lobby
browser, and an empty room follows the normal disposal path so Fly can observe zero active traffic.

Deploy shutdown uses Fly's top-level `kill_signal = "SIGINT"` and `kill_timeout = 300`. The server
drains active matches inside a 295-second application budget after the deploy signal, then closes
connections and exits before Fly's final stop signal. That budget is split into 260 seconds for
matches to end naturally, 10 seconds for forced shutdown finalization of eligible live matches,
20 seconds to wait for queued match-history/replay writes, and 5 seconds of final WebSocket/Axum
slack. New matches are rejected while a drain is in progress. `deploy.sh` runs
`flyctl config validate --strict` before deploying so misplaced Fly config keys fail early instead
of being silently ignored by the platform.

## Automated beta deploys

GitHub Actions deploys beta automatically after the `Main test gate` workflow succeeds on `main`.
The workflow checks out the exact commit that passed the gate and runs:

```bash
./deploy.sh beta <tested-commit>
```

Set the repository `FLY_BETA_API_TOKEN` secret to an app-scoped deploy token for
`rts-0-zvorygin-beta`.

The beta deploy workflow uses the `beta-deploy` concurrency group with `cancel-in-progress: false`.
GitHub allows an in-flight deploy to finish its Fly drain, then deploys only the newest successful
pending commit.

## Match-history and stress-test secrets (Supabase)

Match history persistence requires `DATABASE_URL`. `RTS_RECORD_MATCHES=1` enables beta/mainline
writes; when the gate is off, the server can read history but does not upload match rows or replay
artifacts. Local `cargo run` should keep the gate off.

`RTS_RECORD_STRESS_TESTS=1` independently persists accepted `/stress-test` reports. With that gate
off they are still structured-log events and remain downloadable until the server process restarts.

Set these once per Fly app (replace the URL with the rotated password):

```bash
flyctl secrets set \
  DATABASE_URL='postgres://postgres:NEW_PASSWORD@db.umerhlzpdtbxndptnhui.supabase.co:5432/postgres?sslmode=require' \
  RTS_RECORD_MATCHES=1 \
  RTS_RECORD_STRESS_TESTS=1 \
  -a rts-0-zvorygin-beta

flyctl secrets set \
  DATABASE_URL='postgres://postgres:NEW_PASSWORD@db.umerhlzpdtbxndptnhui.supabase.co:5432/postgres?sslmode=require' \
  RTS_RECORD_MATCHES=1 \
  RTS_RECORD_STRESS_TESTS=1 \
  -a rts-0-zvorygin
```

Setting a secret restarts the Machine. The first restart runs `sqlx::migrate!` to create the
`matches` table; later restarts are no-ops because migrations are tracked.

Verify a beta deploy is recording:

```bash
curl https://rts-0-zvorygin-beta.fly.dev/api/matches | head -c 500
scripts/fly-logs.sh beta recent | rg 'database connected|match recorded|RTS_RECORD_MATCHES'
```

Expected boot line:

```text
INFO rts_server::db: database connected and migrations applied
```

(No `match history writes disabled` line; that only prints when the public gate is off.) Each
finished deployed match logs `match recorded` with map, outcome, replay status, and local-only
scope.

To validate an interrupted deploy-drain match, keep a live match in progress during the deploy and
check the recent logs for the full forced-abort chain:

```bash
scripts/fly-logs.sh beta recent | rg 'shutdown natural drain timeout reached|shutdown finalized active match as aborted|shutdown forced finalization|all match-history writes completed|shutdown match-history write wait timed out|match recorded'
```

Expected success is a natural-drain timeout, one `shutdown finalized active match as aborted` line
per eligible room, an aggregate `shutdown forced finalization complete`, a `match recorded` line
with `outcome=aborted` and `replay=true`, and either `all match-history writes completed during
shutdown` or no pending-write line when the local write completed before waiting began. Treat
`shutdown forced finalization incomplete`, `shutdown match-history write wait timed out`, or
`failed to record match` as deploy validation blockers until Recent Matches confirms the row and
replay.

If public reads return `[]` after a match, check that `RTS_RECORD_MATCHES` is set and not
`0`/`false`, and that the match involved at least one human player with legacy quickstart/debug
mode off. See [docs/design/match-history.md](design/match-history.md) for the full scope table.

## Agent log access

Agents can inspect Fly server logs with a read-only Fly token stored in local `.env`. `.env` is
gitignored; never commit the token. The log helper reads its worktree's `.env` first, then falls
back to the main worktree's `.env`.

Create or rotate the token from an authenticated Fly session:

```bash
flyctl tokens create readonly \
  --org personal \
  --name codex-rts-logs \
  --expiry 175200h0m0s
```

`175200h0m0s` is Fly's default maximum duration: 20 years. Put the returned token in `.env`:

```text
FLY_API_TOKEN=fm2_...
```

Use the wrapper so agents get JSON logs without remembering app names:

```bash
scripts/fly-logs.sh beta recent
scripts/fly-logs.sh mainline recent
```

For older logs inside Fly's retention window, use the paginated HTTP search:

```bash
scripts/fly-logs.sh beta search --from 2026-06-11T22:00:00Z --to 2026-06-11T23:30:00Z
scripts/fly-logs.sh beta search --from 2026-06-11T22:00:00Z --to 2026-06-11T23:30:00Z \
  --filter 'performance tick summary|client network report'
```

For live tailing, bound the command so it cannot run indefinitely:

```bash
timeout 30 scripts/fly-logs.sh beta tail
```

The wrapper maps `mainline` to `rts-0-zvorygin` and `beta` to
`rts-0-zvorygin-beta`, unless `FLY_MAINLINE_APP` or `FLY_BETA_APP` override those defaults. It also
accepts a raw app name:

```bash
scripts/fly-logs.sh rts-0-zvorygin-beta recent --region ewr
```

## Stop spending after game night

```bash
flyctl scale count 0 -a rts-0-zvorygin
flyctl scale count 0 -a rts-0-zvorygin-beta
```

To bring them back:

```bash
flyctl scale count 1 -a rts-0-zvorygin
flyctl scale count 1 -a rts-0-zvorygin-beta
```

Always include `-a` for manual scaling commands; the repository intentionally has multiple Fly
configs.

## Deploy and direct-route validation

Deploy mainline or beta from the repository root:

```bash
./deploy.sh mainline
./deploy.sh beta
```

To deploy an exact revision, pass it as the second argument. The script creates a temporary
detached worktree for that revision, deploys it, and removes the worktree afterward:

```bash
./deploy.sh mainline 5a29d29
./deploy.sh beta 5a29d29
```

The deployed commit is written into the runtime image as `COMMIT_HASH`, so `/version` and client
asset cache-busting reflect the selected revision without baking the SHA into Rust compile
artifacts.

The stable public destinations are direct game origins:

```text
https://bewegungskrieg.net                   mainline on rts-0-zvorygin
https://www.bewegungskrieg.net               mainline on rts-0-zvorygin
https://rts-0-zvorygin-beta.fly.dev          beta on rts-0-zvorygin-beta
```

After a routing or release change, verify both URLs report the intended build and cold-start
normally:

```bash
curl --fail https://bewegungskrieg.net/version
curl --fail https://www.bewegungskrieg.net/version
curl --fail https://rts-0-zvorygin-beta.fly.dev/version
```

The temporary `bewegungskrieg-mainline`, `bewegungskrieg-beta`, and
`rts-0-zvorygin-launcher` apps are not part of the public route. Retain them stopped as rollback
artifacts until the direct endpoints have been accepted; do not change Squarespace DNS or move the
canonical certificates.

To roll mainline back, deploy a known-good commit to the same canonical app, then verify the
canonical `/version` endpoint:

```bash
./deploy.sh --channel mainline --app rts-0-zvorygin <known-good-commit>
curl --fail https://bewegungskrieg.net/version
```
