# Cheaper, Faster Hosting Plan

## Purpose

Move beta and mainline from always-on shared CPU to independently deployable Fly
`performance-1x` Machines that stop when idle, while preserving a fast, understandable startup
experience through one tiny always-on launcher. The target is to remove shared-CPU quota stalls,
keep beta and mainline release isolation, and lower annual compute cost for the actual intermittent
play schedule. This plan keeps an evidence-gated escape hatch to Northflank or OVHcloud, but does not
migrate providers unless the Fly canary fails its performance, startup, reliability, or cost gates.

## Current Baseline (2026-07-11)

- Beta and mainline are separate Fly apps in `ewr`, each running one always-on
  `shared-cpu-4x`/1 GB Machine.
- Current listed compute is `$7.78` per app per 30 days, or `$15.56/month` and `$186.72/year`
  for both before bandwidth and other billable resources.
- The incident's repeated roughly 60 ms stalls match the shared CPU quota shape. The simulation and
  renderer optimizations already merged reduce load, but shared CPU still does not guarantee one
  uninterrupted core.
- Fly `performance-1x`/2 GB is `$0.0431/hour` in `ewr`. With both game apps stopped when idle,
  monthly game compute is approximately `0.0431 * combined_running_hours`.
- A continuously running `shared-cpu-1x`/256 MB launcher is `$1.94/month` or `$23.28/year`.
- At 110 combined beta/mainline running hours per month, the expected total is about `$6.68/month`
  or `$80.17/year` for launcher plus game compute, excluding stopped rootfs, bandwidth, taxes, and
  optional services.
- Official references: [Fly pricing](https://fly.io/docs/about/pricing/),
  [CPU quotas](https://fly.io/docs/machines/cpu-performance/), and
  [autostop/autostart](https://fly.io/docs/reference/fly-proxy-autostop-autostart/).

All prices are planning inputs, not permanent constants. Recheck official pricing before every
cost-bearing rollout phase and record the observed rate in that phase's handoff.

## Desired End State

- Beta and mainline remain separate apps, images, hostnames, secrets, room registries, and deploy
  lifecycles.
- Each game app uses one `performance-1x`/2 GB Machine with `auto_stop_machines = "stop"`,
  `auto_start_machines = true`, and `min_machines_running = 0`.
- One always-on launcher uses `shared-cpu-1x`/256 MB and presents channel choices, an immediate
  accessible startup state, a bounded readiness wait, and a redirect to the chosen game app.
- The launcher wakes only fixed allowlisted beta/mainline origins. It must never accept a caller
  supplied upstream URL or become a general HTTP proxy.
- The browser retries initial WebSocket connection during the bounded wake window and preserves
  replay, lab, match-launch, spectator, and room query parameters.
- Active human and spectator WebSockets keep the selected game Machine running. Headless AI rooms
  without a connected client remain an explicit autostop caveat and must not be silently treated as
  durable background work.
- Cost, cold-start, tick, WebSocket, and availability evidence determine whether the configuration
  remains on Fly or triggers a provider pilot.

## Overall Constraints

- Preserve the server-authoritative simulation, fog filtering, protocol, and current `Game` API.
- Do not combine beta and mainline into one Fly app or one process. They currently run different
  releases, and rooms/lobbies are process memory.
- Use `stop`, not `suspend`, for the initial rollout. A clean boot is safer than restoring Tokio
  timers, SQL connections, and process memory after an arbitrary pause.
- Never stop or resize a Machine during a live match. Every remote change must use the existing
  drain behavior or occur only after verifying the app has no active room.
- Keep the existing 295-second bounded shutdown/finalization path. Measure its no-room autostop
  behavior so an idle Machine does not stay billable for the full drain window unnecessarily.
- Treat a visible startup shell and server readiness as separate concerns. Fly-hosted static files
  also require a running Machine, so the launcher must be a separate always-on service if users are
  to see progress during a true cold start.
- Do not promise `under 5 seconds` in product copy until a beta sample of at least ten stopped-to-
  ready starts supports it. Before that gate, use truthful copy such as `Starting server... usually
  ready in a few seconds` and retain a longer bounded failure timeout.
- The readiness signal must prove the intended release is accepting connections, not merely that
  DNS or TLS responded. Prefer a narrow health/readiness response plus a real WebSocket-open sample.
- Keep launcher and client retries bounded, cancelable, accessible, and free of reload loops.
- Do not add a dedicated IPv4 address, volume, database, or paid CDN unless measured requirements
  justify it.
- Match history and replay persistence continue using external Supabase; no database migration is
  part of this plan.
- Every phase must recheck selected suites with `node tests/select-suites.mjs --from=<base>` and run
  the smallest focused validation that covers its actual changes.
- Each phase is implemented on its own clean `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, and followed through `scripts/wait-pr.sh` until the phase head is reachable from
  `origin/main`.
- Do not start a later phase from an open or merely green PR. A definite merge is required.
- When a phase is complete, mark that phase document done in its implementation commit.
- After every phase, provide a handoff describing what changed, the evidence collected, what the
  next agent should do, and the core features that should be manually tested.
- Repository changes may be prepared without another scope decision, but creating a new paid app,
  resizing a Machine, purchasing a reservation, changing DNS, or deploying a cost-bearing remote
  configuration requires explicit user approval immediately before that action.

## Success Gates

The rollout is successful when all of the following are true:

- A stopped beta `performance-1x` Machine completes at least 9 of 10 cold starts without manual
  refresh or failed launch state.
- The startup UI is visible before the game Machine is ready and never claims readiness before the
  health and WebSocket gates pass.
- Measured cold-start p50 is at most 5 seconds and p95 is at most 10 seconds. Copy may say `usually
  under 5 seconds` only when at least 9 of the 10 most recent clean beta samples are at or below
  5 seconds.
- A connected lobby, player, or spectator prevents autostop throughout a 30-minute soak.
- Closing all connections allows the Machine to stop, and the no-room shutdown path does not wait
  near the full 295-second drain timeout.
- The preserved incident replay or an equivalent high-entity authoritative workload shows no Fly
  shared-quota throttling signature on `performance-1x`.
- Normal beta and mainline deploys keep their releases, configs, secrets, history labels, and URLs
  isolated.
- Observed monthly launcher plus game compute remains below the current `$15.56/month` two-shared-
  Machine baseline at the actual play schedule.

## Phase Summaries

### [Phase 1 - Beta Performance-Autostop Canary](phase-1.md)

Split channel-specific Fly configuration and prepare beta to run on `performance-1x` with clean
autostop, while leaving mainline unchanged. Add a reproducible cold-start and connection measurement
path, then gather at least ten beta starts plus an active-WebSocket soak and an idle-stop check. This
phase decides whether Fly performance-autostop is technically viable before launcher or mainline
rollout work depends on it.

### [Phase 2 - Always-On Launcher and Startup UX](phase-2.md)

Add one tiny allowlisted launcher service that stays available while the game Machines are stopped
and visibly wakes either beta or mainline. Add bounded browser connection retry and truthful status
states without breaking direct links or launch query parameters. This phase proves the full user
journey from an idle environment to a connected lobby and records whether `usually under 5 seconds`
is supportable copy.

### [Phase 3 - Mainline Performance-Autostop Rollout](phase-3.md)

Apply the proven beta configuration to mainline while retaining independent releases and rollback
paths. Update primary navigation and operator documentation so the launcher is the default entry
point but direct channel URLs remain valid recovery paths. This phase completes the production
hosting change only after beta evidence and a final cost check pass.

### [Phase 4 - Cost, Reliability, and Provider Decision](phase-4.md)

Observe real usage long enough to compare running hours, rootfs charges, cold starts, slow ticks,
throttling, WebSocket longevity, and player-visible failures against the baseline. Keep Fly when the
measured outcome meets the success gates, and produce a bounded provider-pilot recommendation only
when a named gate fails. This phase closes the plan with retained evidence and avoids migrating to a
cheaper host merely because its advertised monthly price is lower.

## Phase Index

1. [Phase 1 - Beta Performance-Autostop Canary](phase-1.md)
2. [Phase 2 - Always-On Launcher and Startup UX](phase-2.md)
3. [Phase 3 - Mainline Performance-Autostop Rollout](phase-3.md)
4. [Phase 4 - Cost, Reliability, and Provider Decision](phase-4.md)

## Provider Escape Hatch

Provider migration is not a normal phase outcome. It becomes eligible only when Fly fails a named
success gate after the code-side performance fixes and performance-autostop canary are deployed.

- **Managed dedicated-CPU pilot:** Northflank `nf-compute-100-1`, currently listed as one dedicated
  vCPU/1 GB at `$18/month`, is the preferred performance comparison. Confirm several-hour WebSocket
  behavior before treating it as a production candidate.
- **Lowest-cost VPS pilot:** OVHcloud VPS-1 in Virginia, currently listed around `$4.54/month` for
  two vCores/4 GB, is the preferred cost comparison. Its CPU exclusivity is not explicit, so the
  incident workload and tail latency—not advertised vCore count—decide suitability.
- A VPS pilot must include Caddy/nginx TLS, OS patching, service supervision, logs, rollback,
  secrets, Docker deploys, drain behavior, and a documented operator burden comparison.
- Do not move DNS or mainline traffic during an initial provider benchmark.

References: [Northflank pricing](https://northflank.com/pricing) and
[OVHcloud VPS pricing](https://us.ovhcloud.com/vps/).

## Non-Goals

- No protocol, simulation, balance, fog, prediction, or gameplay change.
- No multi-Machine room routing, sticky-session system, or horizontal game-server scaling.
- No beta/mainline consolidation into one Fly process.
- No database or replay-storage migration.
- No promise of zero cold starts or uninterrupted headless background rooms.
- No permanent dedicated-performance reservation unless measured monthly performance usage makes
  the current Fly reservation blocks economical.
- No external-provider migration without a separate evidence-backed approval.

## Executor Commands

After this plan is reviewed and approved, phases may be executed one at a time with:

```bash
scripts/phase-runner.sh --plan cheaper-faster phase-1 --pr --wait
scripts/phase-runner.sh --plan cheaper-faster phase-2 --pr --wait
scripts/phase-runner.sh --plan cheaper-faster phase-3 --pr --wait
scripts/phase-runner.sh --plan cheaper-faster phase-4 --pr --wait
```

Do not run the next command until the previous phase is merged and its handoff has been reviewed.
