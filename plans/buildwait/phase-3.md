# Phase 3 - Integration Hardening And Documentation

Status: planned.

## Goal

Make the new build-wait behavior robust across the broader simulation loop and document it as the
current build-order contract. This phase should focus on edge cases, design docs, and manual
gameplay confirmation rather than adding a new behavior layer.

## Scope

- Add integration-style sim tests around full tick ordering where needed, especially construction
  before collision cleanup and queued-order promotion after active-order cancellation.
- Confirm active-order cancellation from building blockers and unit-block timeout preserves or
  promotes queued handoff orders according to the established policy.
- Confirm notice behavior is not noisy across a multi-second resource wait or unit-block wait.
- Review AI/self-play behavior for unintended regressions from allowing unaffordable build orders
  to remain active.
- Update `docs/design/server-sim.md` section 3.5 so command planning, queued build promotion,
  resource payment, arrived-worker resource waiting, building-block cancellation, and unit-block
  timeout are described together.
- Refresh `docs/context/server-sim.md` only if section names or code-map entries change.
- Update any test comments or fixture names that still describe immediate skip/cancel behavior as
  the intended contract.
- Run the sim architecture check if new helper/module edges were introduced.

## Expected Touch Points

- `server/crates/sim/src/game/systems.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/services/commands/tests/build.rs`
- `server/crates/ai/src/selfplay/tests/` only if focused evidence shows a regression
- `docs/design/server-sim.md`
- `docs/context/server-sim.md` only if needed

Avoid touching:

- Protocol files unless Phase 2 proved a new client-visible state is necessary
- Client UI files unless there is a confirmed display regression
- Balance values unrelated to the three-second grace

## Implementation Notes

- This phase should not reopen the core state-machine design unless Phase 2 exposed a correctness
  problem.
- If the build-wait state needs client affordance later, document that as a follow-up rather than
  adding protocol churn here.
- Use current repo/runtime evidence, not memory, when checking AI/self-play effects.
- Keep docs factual. Do not describe strategic impact beyond the concrete gameplay change: workers
  no longer abandon valid build orders merely because resources are temporarily unavailable or a
  unit briefly blocks the footprint.

## Verification

Suggested focused commands:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim construction
cargo test --manifest-path server/Cargo.toml -p rts-sim queued_build
cargo test --manifest-path server/Cargo.toml -p rts-sim build_order
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check
```

If AI/self-play code changes, add the narrow AI test that corresponds to the touched area. Do not
run broad bundles by default; rely on the owned PR's `./tests/run-all.sh` gate for full coverage.

## Manual Testing Focus

Manually test the four Phase 2 flows again in a live local match or lab scenario:

- broke worker waits, then starts after resources arrive;
- worker arrives after resources were spent elsewhere and waits;
- another building/scaffold on the footprint cancels the order;
- temporary unit blocker clears before timeout and resumes, while persistent unit blocker times out
  to idle.

Also check one queued-order case: give a worker a queued build followed by a move, force the build
to timeout or become permanently blocked, and confirm the next queued order behavior matches the
documented policy.

## Handoff

After implementation, mark this phase done and summarize the final documented contract, any AI or
queued-order findings, focused verification output, and manual test observations. Include the
player-facing patch-note bullet: workers now wait at valid construction sites for resources and
brief unit blockers instead of immediately abandoning the build order.
