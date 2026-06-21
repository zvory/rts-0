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
Deploy shutdown is configured with Fly's top-level `kill_signal = "SIGINT"` and
`kill_timeout = 300`, the maximum graceful-stop window for shared-CPU Machines. The server drains
active matches for up to 295 seconds after the deploy signal, then closes connections and exits
before Fly's final stop signal. New matches are rejected while a drain is in progress. `deploy.sh`
runs `flyctl config validate --strict` before deploying so misplaced Fly config keys fail early
instead of being silently ignored by the platform.

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

The deployed commit is written into the runtime image as `COMMIT_HASH`, so `/version` and client
asset cache-busting reflect the selected revision without baking the SHA into Rust compile
artifacts.

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
