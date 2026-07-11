# Phase 1 - Beta Performance-Autostop Canary

## Phase Status

- [ ] Pending.

## Objective

Prove that beta can use a stopped `performance-1x` Machine economically and reliably before any
launcher or mainline work depends on that assumption. This phase owns channel-specific Fly config,
beta-only resource/autostop configuration, a repeatable cold-start measurement path, and preserved
canary evidence. Mainline must remain on its current configuration throughout this phase.

## Preconditions and Approval Gate

- Recheck current `ewr` pricing for `performance-1x`/2 GB and stopped rootfs.
- Confirm the beta app has no active human, spectator, replay, lab, or headless AI room before any
  deploy, stop, or resize.
- Obtain explicit user approval immediately before applying the cost-bearing beta Machine change.
- Record the beta and mainline `/version` values before the canary so channel isolation can be
  verified afterward.

## Work

- Introduce channel-specific Fly configuration selected explicitly by `deploy.sh`:
  - beta: `performance-1x`, 2 GB, `auto_stop_machines = "stop"`, autostart enabled, minimum `0`
  - mainline: preserve its current remote size and always-on lifecycle in this phase
- Keep shared environment settings in one source where practical, but make the channel distinction
  reviewable in Git. A beta deploy must not silently rewrite mainline lifecycle policy, and vice
  versa.
- Preserve `kill_signal = "SIGINT"` and the bounded shutdown timeout.
- Add a narrow readiness endpoint only if `/version` plus a WebSocket-open probe cannot distinguish
  process start from actual readiness. Do not expose secrets, database state, room contents, or
  privileged diagnostics.
- Add a bounded operator harness that:
  - verifies beta is stopped before each sample
  - requests the beta readiness URL and records request-to-ready time
  - opens a real WebSocket and records request-to-WS-open time
  - records build id, Machine id/state transition, region, and failure reason
  - repeats at least ten clean samples without an unbounded stream
  - writes ignored artifacts under `target/` rather than committing volatile results
- Measure server boot components when the result exceeds the target, especially image availability,
  process start, database/migration connection, readiness, and proxy recognition.
- Run an active-connection soak for at least 30 minutes and confirm the single Machine remains
  started throughout.
- Close all clients and confirm the Machine stops after Fly's idle evaluation. Verify the no-room
  shutdown path finalizes quickly instead of consuming most of the 295-second timeout.
- Verify a stopped beta restarts from direct lobby, match-launch, replay, spectator, and lab entry
  URLs without losing their query parameters.
- Preserve a compact canary summary under `docs/` only when it is stable operational evidence;
  raw samples stay ignored.
- Update deployment documentation with beta canary commands, rollback, pricing observation, and
  the headless-AI caveat.

## Expected Touch Points

- `fly.toml` and/or new channel-specific Fly config files
- `deploy.sh`
- `Dockerfile` only if readiness or startup size requires a justified change
- `server/src/main.rs` only if a readiness route is necessary
- a focused script under `scripts/` for bounded cold-start measurement
- `scripts/check-deploy-assets.mjs`
- `docs/context/deployment.md`
- `docs/design/hardening.md` only when the deployment/lifecycle source of truth changes
- focused deploy-script and route tests
- `plans/cheaper-faster/phase-1.md` status update in the implementation commit

## Explicit Exclusions

- No launcher app.
- No mainline resize, autostop, DNS, or URL change.
- No `suspend` canary.
- No shared reservation purchase.
- No external-provider deployment.
- No promise or UI copy that startup is under five seconds.

## Implementation Checklist

- [ ] Record current pricing, versions, Machine specs, and lifecycle policy.
- [ ] Add reviewable channel-specific deploy configuration.
- [ ] Add or reuse a truthful readiness signal.
- [ ] Add bounded cold-start/WS-open measurement tooling.
- [ ] Pass repository checks before remote mutation.
- [ ] Obtain explicit approval and deploy beta only.
- [ ] Capture at least ten stopped-to-ready samples.
- [ ] Complete active-WebSocket and idle-stop checks.
- [ ] Confirm direct deep-link behavior and channel isolation.
- [ ] Document rollback to beta `shared-cpu-4x`/1 GB always-on.
- [ ] Mark this phase done in the implementation commit.

## Verification

Run the relevant subset selected by the diff, including:

```bash
flyctl config validate --strict --app rts-0-zvorygin-beta --config <beta-config>
flyctl config validate --strict --app rts-0-zvorygin --config <mainline-config>
node scripts/check-deploy-assets.mjs
node scripts/check-docs-health.mjs
node tests/select-suites.mjs --verify
git diff --check
```

If a server route changes, add its focused Rust/Node coverage and verify missing assets still return
404 rather than the SPA shell.

## Manual Test Focus

With beta confirmed stopped, open the ordinary beta URL and one deep link from a desktop browser and
a phone. Confirm the Machine wakes, the intended page eventually opens, the WebSocket remains
connected, and a 30-minute lobby or match is not stopped. Close every client, verify beta stops, and
repeat enough times to recognize inconsistent or failed cold starts.

## Handoff Expectations

Report the exact beta config and deployed build, the ten-sample cold-start and WS-open distribution,
all failures, active-soak result, idle-stop timing, no-room drain timing, and observed cost rate.
State whether the beta canary passed every gate needed by Phase 2, identify any copy that the
measurements support, and give the next agent the exact rollback command and launcher assumptions.
