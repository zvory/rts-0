# Fly.io deploy

This app runs as one Rust process that serves the static client and upgrades `/ws` to a WebSocket.
Fly proxies HTTPS and WSS traffic to the container on port 8080.

`fly.mainline.toml` and `fly.beta.toml` enable basic production performance tracing with
`RTS_PERF=spikes` and `RTS_PERF_SLOW_TICK_MS=40`. Fly logs will include a `performance tick
summary` row only when a server tick takes at least 40 ms. `fly.launcher.toml` is deliberately
separate and cannot serve game traffic.

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

Phase 1 leaves mainline always-on with its existing lifecycle. Beta uses one `performance-1x`
Machine with 2 GB of memory, `auto_stop_machines = "stop"`, autostart enabled, and zero minimum
running Machines. `deploy.sh` always selects the channel's explicit config, including when `--app`
overrides the normal app name.

Fly's HTTP activity keeps a game Machine running while browsers are connected. A headless AI room
without a connected browser may not prevent autostop; durable unattended AI games are not a goal
of this setup.
Deploy shutdown is configured with Fly's top-level `kill_signal = "SIGINT"` and
`kill_timeout = 300`, the configured graceful-stop window for game Machines. The server drains
active matches inside a 295 second application budget after the deploy signal, then closes
connections and exits before Fly's final stop signal. That budget is split into 260 seconds for
matches to end naturally, 10 seconds for forced shutdown finalization of any still-active tracked
rooms, 20 seconds to wait for queued match-history/replay writes, and 5 seconds of final
WebSocket/Axum slack. Forced finalization records eligible normal live matches as replay-backed
`outcome = aborted` rows with no winner; non-eligible active rooms ack without public
match-history writes. New matches are rejected while a drain is in progress. `deploy.sh` runs
`flyctl config validate --strict` before deploying so misplaced Fly config keys fail early instead
of being silently ignored by the platform.

## Automated beta deploys

GitHub Actions deploys beta automatically after the `Main test gate` workflow succeeds on `main`.
The workflow checks out the exact commit that passed the gate and runs:

```bash
./deploy.sh beta <tested-commit>
```

Set a repository Actions secret named `FLY_BETA_API_TOKEN` before relying on the workflow. Prefer
an app-scoped Fly deploy token for `rts-0-zvorygin-beta` so the CI secret cannot deploy unrelated
apps if it leaks.

The beta deploy workflow uses the `beta-deploy` concurrency group with `cancel-in-progress: false`.
GitHub keeps one deploy running and, by default, only one pending replacement in the same group.
That coalesces frequent pushes to `main`: an in-flight deploy is allowed to finish its Fly drain,
and the newest successful pending commit replaces older pending deploys instead of deploying every
intermediate commit.

## Match-history secrets (Supabase)

Match history persistence requires `DATABASE_URL`. `RTS_RECORD_MATCHES=1` enables beta/mainline
writes; when the gate is off, the server can read history but does not upload match rows or replay
artifacts. Local `cargo run` should keep the gate off.

Set these once per Fly app (replace the URL with the rotated password):

```bash
flyctl secrets set \
  DATABASE_URL='postgres://postgres:NEW_PASSWORD@db.umerhlzpdtbxndptnhui.supabase.co:5432/postgres?sslmode=require' \
  RTS_RECORD_MATCHES=1 \
  -a rts-0-zvorygin-beta

flyctl secrets set \
  DATABASE_URL='postgres://postgres:NEW_PASSWORD@db.umerhlzpdtbxndptnhui.supabase.co:5432/postgres?sslmode=require' \
  RTS_RECORD_MATCHES=1 \
  -a rts-0-zvorygin
```

Setting a secret restarts the machines. The first restart runs `sqlx::migrate!` to create the
`matches` table; subsequent restarts are no-ops because migrations are tracked.

Verify a deploy is recording:

```bash
curl https://rts-0-zvorygin-beta.fly.dev/api/matches | head -c 500
scripts/fly-logs.sh beta recent | rg 'database connected|match recorded|RTS_RECORD_MATCHES'
```

Expected boot lines on a recording env:

```
INFO rts_server::db: database connected and migrations applied
```

(no "match history writes disabled" line; that one only prints when the public gate is off). Each
finished deployed match logs `match recorded` with map, outcome, replay status, and local-only
scope.

To validate an interrupted deploy-drain match, keep a live match in progress during the deploy and
check the recent logs for the whole forced-abort chain:

```bash
scripts/fly-logs.sh beta recent | rg 'shutdown natural drain timeout reached|shutdown finalized active match as aborted|shutdown forced finalization|all match-history writes completed|shutdown match-history write wait timed out|match recorded'
```

Expected successful drain-abort evidence is a natural-drain timeout, one per-room
`shutdown finalized active match as aborted` line for each eligible live match, an aggregate
`shutdown forced finalization complete`, a `match recorded` line with `outcome=aborted` and
`replay=true`, and either `all match-history writes completed during shutdown` or no pending-write
line when the local write completed before the wait began. Treat `shutdown forced finalization
incomplete`, `shutdown match-history write wait timed out`, or `failed to record match` as deploy
validation blockers for that interrupted match until Recent Matches confirms the row and replay.

If public reads return `[]` after a match: check `RTS_RECORD_MATCHES` is set and not `0`/`false`,
and that the match involved at least one human player with legacy quickstart/debug mode off. See
[docs/design/match-history.md](design/match-history.md) for the full scope table.

## Agent log access

Agents can inspect Fly server logs with a read-only Fly token stored in local `.env`.
`.env` is gitignored; never commit the actual token. The log helper reads this worktree's `.env`
first, then falls back to the `main` worktree's `.env`, so agents in `/tmp/rts-worktrees/...` can
use the real checkout's token.

Create or rotate the token from an authenticated Fly session:

```bash
flyctl tokens create readonly \
  --org personal \
  --name codex-rts-logs \
  --expiry 175200h0m0s
```

`175200h0m0s` is Fly's default maximum duration: 20 years. Put the returned token in `.env`:

```bash
FLY_API_TOKEN=fm2_...
```

Use the repo wrapper so agents get JSON logs and do not need to remember app names:

```bash
scripts/fly-logs.sh beta recent
scripts/fly-logs.sh mainline recent
```

For older logs inside Fly's retention window, use search mode. It calls Fly's HTTP logs API and
pages forward from `--from`, instead of only returning the small `flyctl logs --no-tail` buffer:

```bash
scripts/fly-logs.sh beta search --from 2026-06-11T22:00:00Z --to 2026-06-11T23:30:00Z
scripts/fly-logs.sh beta search --from 2026-06-11T22:00:00Z --to 2026-06-11T23:30:00Z \
  --filter 'performance tick summary|client network report'
```

For live tailing, bound the command when an agent runs it so it does not stream forever:

```bash
timeout 30 scripts/fly-logs.sh beta tail
```

The wrapper maps `beta` to `rts-0-zvorygin-beta` and `mainline` to `rts-0-zvorygin`, unless
`FLY_BETA_APP` or `FLY_MAINLINE_APP` override those names. It also accepts a raw app name:

```bash
scripts/fly-logs.sh rts-0-zvorygin-beta recent --region ewr
```

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

Always include `-a` for manual scaling commands; the repository intentionally has multiple Fly
configs.

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

The deployed commit is written into the runtime image as `COMMIT_HASH`, so `/version` and client
asset cache-busting reflect the selected revision without baking the SHA into Rust compile
artifacts.

## Custom domains

The stable destinations are:

```text
https://bewegungskrieg.net                 launcher
https://www.bewegungskrieg.net             launcher
https://mainline.bewegungskrieg.net        mainline game server
https://beta.bewegungskrieg.net            beta game server
```

The raw Fly hostnames remain recovery paths even if custom DNS or the launcher is unavailable:

```text
https://rts-0-zvorygin.fly.dev
https://rts-0-zvorygin-beta.fly.dev
https://rts-0-zvorygin-launcher.fly.dev
```

The launcher offers only fixed mainline and beta choices. It polls a fixed `/version` URL to wake
the chosen game app, shows `Starting server...`, and redirects rather than proxying HTTP or
WebSocket traffic. Requests cannot supply an upstream origin. Paths, queries, and fragments are
preserved; a non-root canonical URL defaults to mainline.

Remote setup changes paid Machine sizing and hostname routing, so capture `PRE_PHASE_SHA` and the
current DNS values and obtain explicit approval immediately before running these commands:

```bash
flyctl apps create rts-0-zvorygin-launcher
./deploy.sh launcher
./deploy.sh beta

flyctl certs add mainline.bewegungskrieg.net -a rts-0-zvorygin
flyctl certs add beta.bewegungskrieg.net -a rts-0-zvorygin-beta
flyctl certs add bewegungskrieg.net -a rts-0-zvorygin-launcher
flyctl certs add www.bewegungskrieg.net -a rts-0-zvorygin-launcher
flyctl certs show mainline.bewegungskrieg.net -a rts-0-zvorygin
flyctl certs show beta.bewegungskrieg.net -a rts-0-zvorygin-beta
flyctl certs show bewegungskrieg.net -a rts-0-zvorygin-launcher
flyctl certs show www.bewegungskrieg.net -a rts-0-zvorygin-launcher
```

Apply the A/AAAA/CNAME records printed by those `certs show` commands only after both channel
hostnames work. Save the old canonical A/AAAA/CNAME values before replacing them.

### Phase 1 rollback

To restore beta's pre-phase image, shared CPU size, and always-on lifecycle, run the old deployment
wrapper from the captured commit:

```bash
git worktree add --detach /tmp/rts-hosting-rollback "$PRE_PHASE_SHA"
/tmp/rts-hosting-rollback/deploy.sh beta
git worktree remove /tmp/rts-hosting-rollback
```

To return the canonical certificates to mainline, first restore the saved canonical DNS records,
then run:

```bash
flyctl certs add bewegungskrieg.net -a rts-0-zvorygin
flyctl certs add www.bewegungskrieg.net -a rts-0-zvorygin
flyctl certs remove bewegungskrieg.net -a rts-0-zvorygin-launcher
flyctl certs remove www.bewegungskrieg.net -a rts-0-zvorygin-launcher
flyctl certs show bewegungskrieg.net -a rts-0-zvorygin
flyctl certs show www.bewegungskrieg.net -a rts-0-zvorygin
```

The DNS provider is intentionally not automated by this repository, so the exact saved record
values are part of the rollout record. After rollback, the launcher app can be left idle for
inspection or removed with `flyctl apps destroy rts-0-zvorygin-launcher` after explicit approval.
