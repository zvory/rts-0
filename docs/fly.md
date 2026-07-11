# Fly.io deploy

This app runs as one Rust process that serves the static client and upgrades `/ws` to a WebSocket.
Fly proxies HTTPS and WSS traffic to the container on port 8080.

`fly.mainline.toml` and `fly.beta.toml` enable basic production performance tracing with
`RTS_PERF=spikes` and `RTS_PERF_SLOW_TICK_MS=40`. Fly logs will include a `performance tick
summary` row only when a server tick takes at least 40 ms. `fly.launcher.toml` is deliberately
separate and cannot serve game traffic.

## App roles and first deploy

```bash
flyctl auth login
flyctl apps create bewegungskrieg-mainline
flyctl apps create bewegungskrieg-beta
./deploy.sh mainline
./deploy.sh beta
```

The three release channels have deliberately separate app identities and configs:

```text
bewegungskrieg-mainline  fly.mainline.toml  stopped when idle
bewegungskrieg-beta      fly.beta.toml      stopped when idle
rts-0-zvorygin           fly.launcher.toml  always-on canonical launcher
```

The existing `rts-0-zvorygin` app retains the `bewegungskrieg.net` and
`www.bewegungskrieg.net` certificates and Squarespace DNS. Deploy the launcher there only after
both named game apps work directly. If a game app name is unavailable, choose another globally
unique name and use `--app`; also update the launcher allowlist before cutover.

The raw game origins are the direct access and recovery paths:

```text
https://bewegungskrieg-mainline.fly.dev
https://bewegungskrieg-beta.fly.dev
```

Run one machine only. Game rooms live in server memory, so multiple machines can split players
between different lobbies.

Both game channels use one `performance-1x` Machine with 2 GB of memory,
`auto_stop_machines = "stop"`, autostart enabled, and zero minimum running Machines. The launcher
uses one always-on `shared-cpu-1x`/256 MB Machine. `deploy.sh` always selects the channel's explicit
config, including when `--app` overrides the normal app name.

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

Set a repository Actions secret named `FLY_BETA_API_TOKEN` before relying on the workflow. Replace
the legacy beta token during cutover with an app-scoped deploy token for `bewegungskrieg-beta` so
the workflow can reach the new app without gaining access to unrelated apps.

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
  -a bewegungskrieg-beta

flyctl secrets set \
  DATABASE_URL='postgres://postgres:NEW_PASSWORD@db.umerhlzpdtbxndptnhui.supabase.co:5432/postgres?sslmode=require' \
  RTS_RECORD_MATCHES=1 \
  -a bewegungskrieg-mainline
```

Setting a secret restarts the machines. The first restart runs `sqlx::migrate!` to create the
`matches` table; subsequent restarts are no-ops because migrations are tracked.

Verify a deploy is recording:

```bash
curl https://bewegungskrieg-beta.fly.dev/api/matches | head -c 500
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

The wrapper maps `beta` to `bewegungskrieg-beta` and `mainline` to
`bewegungskrieg-mainline`, unless
`FLY_BETA_APP` or `FLY_MAINLINE_APP` override those names. It also accepts a raw app name:

```bash
scripts/fly-logs.sh bewegungskrieg-beta recent --region ewr
```

## Stop spending after game night

```bash
flyctl scale count 0 -a bewegungskrieg-mainline
flyctl scale count 0 -a bewegungskrieg-beta
```

To bring it back:

```bash
flyctl scale count 1 -a bewegungskrieg-mainline
flyctl scale count 1 -a bewegungskrieg-beta
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

## Canonical launcher and no-DNS cutover

The stable destinations are:

```text
https://bewegungskrieg.net                   launcher on rts-0-zvorygin
https://www.bewegungskrieg.net               launcher on rts-0-zvorygin
https://bewegungskrieg-mainline.fly.dev      mainline game server
https://bewegungskrieg-beta.fly.dev          beta game server
```

The launcher offers only those two fixed game origins. It polls the selected origin's `/version`
route to wake it, shows `Loading...`, and redirects rather than proxying HTTP or WebSocket
traffic. Requests cannot supply an upstream origin. Paths, queries, and fragments are preserved;
a non-root canonical URL defaults to mainline.

### Cost check

Rechecked against Fly's EWR resource pricing on 2026-07-11. The always-on
`shared-cpu-1x`/256 MB launcher is about $1.94 per 30 days. Each running
`performance-1x`/2 GB game Machine is $0.0431/hour; stopped Machines incur only rootfs storage at
$0.15/GB per 30 days. At the planning assumption of eight total game-server hours per month, base
compute is about $2.28/month (`$1.94 + 8 * $0.0431`), plus stopped-rootfs storage and small egress;
retaining the two legacy stopped Machines also adds their rootfs storage. This remains below the
roughly $7.78/month EWR compute price of the legacy always-on shared-cpu-4x/1 GB mainline before
counting beta. Recheck [Fly resource pricing](https://fly.io/docs/about/pricing/) and the account's
Cost Explorer immediately before cutover.

### Capture the legacy state

Immediately before any remote mutation, obtain explicit approval and capture both legacy game
apps. Keep the output local because it contains operational metadata; secret values cannot be read
back from Fly, so source them from the existing password manager or CI secret store and compare the
captured names before setting the new apps.

```bash
export ROLLOUT_DIR="/tmp/rts-hosting-rollout-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$ROLLOUT_DIR"
git rev-parse HEAD > "$ROLLOUT_DIR/phase-sha.txt"
for app in rts-0-zvorygin rts-0-zvorygin-beta; do
  flyctl status --json -a "$app" > "$ROLLOUT_DIR/$app-status.json"
  flyctl machine list --json -a "$app" > "$ROLLOUT_DIR/$app-machines.json"
  flyctl config show -a "$app" --toml > "$ROLLOUT_DIR/$app-config.toml"
  flyctl releases --json --image -a "$app" > "$ROLLOUT_DIR/$app-releases.json"
  flyctl secrets list --json -a "$app" > "$ROLLOUT_DIR/$app-secret-names.json"
done
curl --fail https://bewegungskrieg.net/version > "$ROLLOUT_DIR/legacy-mainline-version.json"
curl --fail https://rts-0-zvorygin-beta.fly.dev/version \
  > "$ROLLOUT_DIR/legacy-beta-version.json"
flyctl certs show bewegungskrieg.net -a rts-0-zvorygin \
  > "$ROLLOUT_DIR/canonical-cert.txt"
flyctl certs show www.bewegungskrieg.net -a rts-0-zvorygin \
  > "$ROLLOUT_DIR/www-cert.txt"
```

The known game secrets include `DATABASE_URL` and `RTS_RECORD_MATCHES`; beta may also have the
`RTS_SCENARIO_PR_*` secrets staged by CI. Treat the captured inventory as authoritative and do not
omit an unfamiliar channel-specific name.

### Cutover order

Create and verify the game apps before replacing the canonical app. These commands create paid
resources and change remote service roles; run them only after the phase commit is merged and the
user has approved the current price and cutover. Never change Squarespace DNS or move/remove the
canonical certificates.

```bash
export ROLLOUT_SHA=<merged-phase-2-sha>
flyctl apps create bewegungskrieg-mainline
flyctl apps create bewegungskrieg-beta

# Set every name captured for the corresponding legacy channel from the secret source of truth.
flyctl secrets set DATABASE_URL="$MAINLINE_DATABASE_URL" RTS_RECORD_MATCHES=1 \
  -a bewegungskrieg-mainline
flyctl secrets set DATABASE_URL="$BETA_DATABASE_URL" RTS_RECORD_MATCHES=1 \
  -a bewegungskrieg-beta

flyctl config validate --strict --app bewegungskrieg-mainline --config fly.mainline.toml
flyctl config validate --strict --app bewegungskrieg-beta --config fly.beta.toml
flyctl config validate --strict --app rts-0-zvorygin --config fly.launcher.toml
node scripts/check-deploy-assets.mjs
./deploy.sh mainline "$ROLLOUT_SHA"
./deploy.sh beta "$ROLLOUT_SHA"
curl --fail https://bewegungskrieg-mainline.fly.dev/version
curl --fail https://bewegungskrieg-beta.fly.dev/version
```

Confirm both raw origins report `ROLLOUT_SHA`, have no active room, and can each cold-start directly
once. Then confirm `rts-0-zvorygin` has no active room and replace it last:

```bash
./deploy.sh launcher "$ROLLOUT_SHA"
curl --fail https://bewegungskrieg.net/healthz
```

Retain the two legacy apps until acceptance is complete. Do not destroy either one. Verify
the launcher cold-starts beta twice and mainline once, preserves a canonical deep link, keeps each
server running during a browser session, and lets each stop after all browsers disconnect. After
those checks pass, stop the superseded apps without destroying them:

```bash
flyctl scale count 0 -a rts-0-zvorygin-beta
flyctl scale count 0 -a rts-0-zvorygin-launcher
```

### No-DNS rollback

Rollback restores the captured mainline game release to the canonical app and restarts legacy
beta. Use the pre-cutover game commit from the captured release record as
`LEGACY_MAINLINE_SHA`; the old mainline config is selected explicitly because the current launcher
default also targets `rts-0-zvorygin`.

```bash
export LEGACY_MAINLINE_SHA=<captured-pre-cutover-mainline-git-sha>
git worktree add --detach /tmp/rts-hosting-rollback "$LEGACY_MAINLINE_SHA"
/tmp/rts-hosting-rollback/deploy.sh mainline --app rts-0-zvorygin
git worktree remove /tmp/rts-hosting-rollback
flyctl scale count 1 -a rts-0-zvorygin-beta
flyctl scale count 0 -a bewegungskrieg-mainline
flyctl scale count 0 -a bewegungskrieg-beta
```

Verify `https://bewegungskrieg.net/version` reports the captured mainline build and the legacy beta
origin responds. DNS and certificates remain unchanged throughout either direction of the swap.
