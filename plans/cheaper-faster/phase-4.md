# Phase 4 - Cost, Reliability, and Provider Decision

## Phase Status

- [ ] Pending.

## Objective

Validate the finished hosting model against real usage rather than one canary session. Preserve a
compact evidence package covering cost, startup, performance, WebSocket reliability, autostop, and
operator burden, then decide whether Fly remains the host. This phase changes providers only through
a separately approved follow-up plan; its normal output is evidence, tuning, and closure.

## Preconditions

- Phases 1 through 3 are merged and their heads are reachable from `origin/main`.
- Both channels and the launcher are deployed from known commits with rollback paths recorded.
- At least one normal billing/usage observation window is available. Prefer 30 days, but a shorter
  window may close the plan only when it includes multiple real beta sessions, one mainline session,
  multiple idle stops, and enough cold starts to evaluate the success gates.

## Work

- Collect per-channel running hours, stopped hours, rootfs charges, launcher compute, game compute,
  bandwidth, and total hosting spend. Separate one-time or unrelated Fly charges from this plan's
  recurring cost.
- Compare observed cost with:
  - old baseline: `$15.56/month` compute for two always-on shared Machines
  - planned formula: launcher plus `0.0431 * combined_game_running_hours`, using current rates
  - the approved monthly and annual budget
- Collect cold-start p50/p95/max, failure/retry counts, time to HTTP readiness, time to WebSocket
  open, and launcher abandonment/timeout evidence without storing player or room identifiers.
- Collect server tick p50/p95/max, slow-tick percentage, scheduler lag, Fly CPU throttle/steal,
  snapshot gaps, and command acknowledgement during representative real or replay workloads.
- Confirm there is no shared-CPU quota signature on the performance Machines.
- Verify that active WebSockets prevented autostop and that idle stop occurred consistently after
  sessions. Record cases where forgotten tabs, background AI, lab, replay, or spectators kept a
  Machine running unexpectedly.
- Review launcher availability, latency, logs, security events, and resource use. Confirm 256 MB is
  sufficient without swap, OOM, or restart churn.
- Review deploy and rollback burden for both channels and launcher. Record any match interrupted by
  a deploy, resize, autostop decision, launcher failure, or incorrect health state.
- Tune timeouts, copy, and documentation only from observed evidence. Do not weaken correctness to
  make the startup number look better.
- Evaluate a provider pilot only when a named gate fails:
  - **Northflank:** use when Fly performance or lifecycle reliability fails but managed dedicated
    CPU remains desirable.
  - **OVHcloud:** use when recurring cost fails the budget and the user accepts VPS operations.
- For an eligible pilot, write a new plan with a no-DNS benchmark phase. Do not provision or migrate
  from this phase without explicit approval.
- Mark this plan complete only when retained evidence supports staying on Fly or a separate provider
  plan has been approved.

## Expected Touch Points

- a compact hosting evidence/runbook document under `docs/`
- bounded analysis tooling under `scripts/` only if current Fly/log tooling cannot answer the gates
- launcher or client timeout/copy tests for evidence-backed tuning
- deployment docs and pricing observations
- `plans/cheaper-faster/phase-4.md` status update in the implementation commit

## Decision Matrix

Stay on Fly when:

- startup and retry gates pass
- performance Machines eliminate quota-shaped stalls
- active matches are never autostopped
- launcher plus game cost stays under the approved budget
- operator burden remains lower than a VPS alternative

Open a Northflank pilot plan when:

- performance-autostop lifecycle is unreliable or cold starts repeatedly fail
- a full dedicated vCPU is still required
- managed deployment is worth the higher listed cost

Open an OVHcloud pilot plan when:

- cost remains unacceptable after autostop
- the user accepts host security/operations ownership
- a no-DNS incident-workload benchmark is authorized

Rollback or keep one channel always on when:

- active WebSockets do not reliably prevent stop
- headless rooms are a required product behavior
- cold-start or launcher failure materially blocks players

## Implementation Checklist

- [ ] Define the observation window and approved budget.
- [ ] Export normalized cost and running-hour evidence.
- [ ] Summarize cold-start, retry, and launcher reliability.
- [ ] Summarize tick, throttle, snapshot, and command performance.
- [ ] Audit active-connection and idle-stop correctness.
- [ ] Audit launcher resource use and security posture.
- [ ] Audit deploy, rollback, and operator burden.
- [ ] Apply only evidence-backed timeout/copy/documentation tuning.
- [ ] Record the Fly/provider decision and rationale.
- [ ] Create a separate provider plan only when a named gate fails and the user approves it.
- [ ] Mark this phase and the plan done in the implementation commit.

## Verification

Run the smallest suites selected by any tuning diff, plus:

```bash
node scripts/check-docs-health.mjs
node tests/select-suites.mjs --verify
git diff --check
```

Re-run the bounded cold-start harness and the preserved incident-shaped server/client performance
workloads when the phase changes readiness, retry, launcher, or hosting behavior. Verify cost claims
against the provider's current official billing output rather than only the planning estimates.

## Manual Test Focus

Use the launcher for multiple real beta sessions and at least one mainline session from desktop and
phone. Include a cold start, warm start, direct-link recovery, spectator connection, disconnect,
forgotten-tab case, and full idle stop. Confirm the experience is understandable without developer
tools and that no active match is interrupted.

## Handoff Expectations

Provide the final monthly/annual cost, running hours, cold-start distribution, performance summary,
autostop correctness, launcher reliability, and operator burden. State the explicit decision to
stay on Fly, roll back, keep one channel always on, or open a separately approved provider pilot,
and list the core manual checks that future hosting changes must preserve.
