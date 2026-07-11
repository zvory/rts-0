# Phase 3 - Mainline Performance-Autostop Rollout

## Phase Status

- [ ] Pending.

## Objective

Apply the proven performance-autostop lifecycle to mainline without weakening its stability role or
coupling its release to beta. Make the launcher the normal human entry point while preserving direct
channel URLs and a fast rollback to the previous mainline Machine configuration. Confirm that the
two environments remain operationally and observably distinct after rollout.

## Preconditions and Approval Gate

- Phases 1 and 2 are merged and reachable from `origin/main`.
- Beta has completed the required cold-start, active-WebSocket, idle-stop, and real-play canaries.
- Launcher cold-start and deep-link behavior has passed desktop and phone review.
- No unresolved beta issue indicates a shared lifecycle or client retry defect.
- Recheck current mainline usage, Fly pricing, launcher cost, and projected annual total.
- Obtain explicit user approval before resizing/stopping mainline or changing default public links.

## Work

- Update the mainline channel config to `performance-1x`/2 GB, clean autostop, autostart enabled,
  and minimum `0`.
- Keep beta and mainline configs separate and explicit in `deploy.sh`; a channel deploy must not
  overwrite the other channel's lifecycle or size.
- Verify the mainline app has no active room before the first resize/deploy and allow the existing
  drain path to complete.
- Record current mainline build, Machine config, secrets inventory names, DNS/hostname state, and a
  rollback command before mutation.
- Deploy mainline from a known merged commit and verify `/version`, readiness, WebSocket, lobby,
  history, replay, spectator, and one short authoritative match.
- Stop mainline and exercise launcher-to-mainline cold start, direct mainline cold start, deep links,
  and startup retry on desktop and phone.
- Confirm mainline remains started throughout an active player/spectator soak and stops only after
  all traffic closes.
- Confirm beta and mainline report their own intended build ids and do not share rooms or release
  state.
- Make the launcher the documented/default entry point in repository-owned navigation while
  retaining direct channel URLs for diagnosis and recovery.
- Update operator documentation for deploy order, no-live-room checks, forced rollback, launcher
  outage behavior, cost inspection, and headless AI caveats.
- Do not buy a performance reservation during rollout. Its smallest block is uneconomical until
  measured running hours consistently exceed the reservation break-even.

## Expected Touch Points

- mainline channel Fly config
- `deploy.sh`
- launcher channel metadata/links
- current mainline-to-beta redirect or navigation policy in `server/src/main.rs`
- focused redirect/navigation tests
- `docs/context/deployment.md`
- `docs/pr-first-workflow.md` only if operator recovery steps materially change
- a hosting-cost/runbook document if Phase 2 did not already add one
- `plans/cheaper-faster/phase-3.md` status update in the implementation commit

## Explicit Exclusions

- No beta/mainline app consolidation.
- No simultaneous mainline and beta deployment.
- No provider migration.
- No permanent always-on performance Machine.
- No DNS removal for direct channel origins.
- No gameplay or wire-protocol change.

## Implementation Checklist

- [ ] Recheck pricing and projected cost from observed beta running hours.
- [ ] Record mainline pre-change build/config and rollback commands.
- [ ] Update mainline channel config without changing beta.
- [ ] Pass repository and Fly config validation.
- [ ] Obtain explicit approval and verify no active mainline room.
- [ ] Deploy/resize mainline from the merged phase head.
- [ ] Validate warm, stopped, launcher, and direct flows.
- [ ] Complete active-connection and idle-stop checks.
- [ ] Confirm release, room, secret, history, and deploy isolation.
- [ ] Update navigation and operator docs.
- [ ] Mark this phase done in the implementation commit.

## Verification

Run the relevant selector-chosen suites plus:

```bash
flyctl config validate --strict --app rts-0-zvorygin --config <mainline-config>
flyctl config validate --strict --app rts-0-zvorygin-beta --config <beta-config>
node scripts/check-deploy-assets.mjs
node scripts/check-docs-health.mjs
git diff --check
```

Run focused HTTP/WebSocket integration for any redirect or launcher metadata change. After the
merged PR and explicit approval, verify both remote apps' `/version`, Machine size/state, history,
and independent room behavior.

## Manual Test Focus

With mainline stopped, enter through the launcher from desktop and phone and play one short match.
Repeat through a direct deep link and as a spectator, then close all clients and verify the Machine
stops. Deploy or inspect beta separately and prove mainline's build, lobby, and rooms are unchanged.

## Handoff Expectations

Report mainline's before/after Machine config, build ids for both channels, cold/warm timings,
active-soak and idle-stop results, launcher/direct behavior, projected monthly cost, and rollback
commands. Give Phase 4 the exact billing window, metrics, log queries, and known caveats it must
observe before closing the plan.
