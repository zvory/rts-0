# Phase 1 - Freeze Production Jeff

## Phase Status

- [ ] Ready for implementation.

## Objective

Create a deterministic golden oracle for the actual production `AiController` call sequence before
changing any AI architecture. The oracle must prove tick by tick that Jeff receives the same inputs
and emits the same ordered commands, including empty and retreat-only calls. This phase is
protective only: it must not change controller behavior, cadence, placement, balance, protocol, or
room execution.

## Why Existing Evidence Is Insufficient

- Command-log replay proves deterministic simulation after commands already exist; it does not rerun
  Jeff and prove that Jeff generates the same commands.
- Normal AI tests assert broad milestones and liveness rather than exact Jeff output.
- Matchup and arena use `ProfileBackedScript`, whose cadence, placement search, and retreat behavior
  differ from production, so their artifacts cannot be the compatibility oracle.
- The profile fingerprint covers data and hand-maintained metadata, not observation projection,
  decision code, runtime state, cadence, or adapter behavior.

## Work

- Add a test-only production-loop transcript runner that mirrors the current live AI-only sequence:
  - load the authored `Chokes` map with fixed seed and normal player/team/start assignment;
  - run two `jeffs_ai` controllers in player order so both stagger offsets and map sides are covered;
  - compute `primary_base_alive_players()` before each tick and skip absent controllers;
  - obtain each player's `snapshot_for` and `worker_retreat_commands_for`;
  - call controllers and collect every result before enqueueing any command;
  - enqueue in controller/result order, then call `Game::tick()`;
  - capture recipient events and fog-filtered post-tick state.
- Define a versioned canonical transcript schema containing:
  - fixture schema, seed, horizon, map/content fingerprint, ordered player/team/faction/profile data,
    and profile fingerprints;
  - every pre-tick, ordered alive IDs, invoked/skipped controllers, player/profile identity,
    snapshot fingerprint, exact retreat input, exact ordered emitted commands, and a new decision
    trace only when its trace tick matches the current tick;
  - post-tick command-log delta, recipient event fingerprints, objective-alive IDs, and per-player
    snapshot fingerprints.
- Use documented stable canonical bytes and a repository-owned stable hash such as FNV-1a64. Do not
  use `DefaultHasher`, Git identity as behavior identity, or hashes without preserving full emitted
  commands for diagnostics.
- Commit one bounded Jeff-versus-Jeff fixture. The normal gate should compare an early prefix long
  enough to cover cadence, opening economy, construction, production, staging, and first attack
  orders; `RTS_FULL_AI_TESTS=1` should compare the longer continuation through armored combat,
  retreats, pending-build recovery, expansion, and stage suppression.
- Measure the candidate fixture before committing it. Prefer a compact JSONL/manifest representation
  and avoid a seed/map matrix; if the approximately 9,000-tick proposal produces an unreasonably
  large fixture or slow default prefix, shorten or chunk it while retaining the named behavior
  coverage and full opt-in continuation.
- Implement a first-divergence comparator that stops at the first mismatching scenario/tick/player
  and prints expected/actual invocation status, command index and payload, snapshot and retreat
  inputs, trace lines, event/post-state fingerprints, fixture metadata, and a useful classification
  such as `input_drift`, `command_drift`, `trace_drift`, or `post_tick_drift`.
- Unit-test missing, extra, reordered, and field-changed commands plus input/post-state drift.
- State the Phase 1 boundary explicitly: this runner freezes the current controller and mirrored
  host sequence, but cannot by itself catch a future divergence between that mirror and
  `lobby/live_tick.rs`. Phase 2 must remove that blind spot by extracting one shared AI tick driver
  used by the live host, offline hosts, and this transcript runner; the frozen fixture guards that
  production cutover.
- Provide an explicit candidate-generation path that writes only under `target/`. Ordinary tests
  must never bless or rewrite checked-in fixtures, and Phases 2-8 may not update the fixture to make
  a refactor pass.
- Update `docs/design/testing.md` and `docs/design/ai.md` to distinguish simulation replay,
  controller-generation parity, and the still-unresolved offline/live mismatch.

## Expected Touch Points

- A new oracle module under `server/crates/ai/src/selfplay/tests/` or an equivalent test-only area.
- Test registration under `server/crates/ai/src/selfplay/tests/mod.rs`.
- A reviewable fixture/manifest location under `server/crates/ai/`.
- An optional test-support generator/comparator module that does not widen production exports.
- `docs/design/testing.md`.
- `docs/design/ai.md`.

## Implementation Checklist

- [ ] Mirror the exact current production AI-only call order without extracting it.
- [ ] Document the temporary host-orchestration blind spot and Phase 2 closure requirement.
- [ ] Define the versioned transcript schema and stable canonical hashing.
- [ ] Capture exact commands and all required input/post-tick fingerprints.
- [ ] Commit one bounded Jeff-versus-Jeff fixture with normal/full tiers.
- [ ] Add actionable first-divergence reporting and comparator tests.
- [ ] Add an explicit target-only candidate generator and fixture refresh policy.
- [ ] Document what replay proves versus what the Jeff oracle proves.
- [ ] Confirm no production AI, simulation, protocol, or gameplay code changed.
- [ ] Mark this phase done in the implementation commit.

## Verification

```bash
cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default -p rts-ai \
  -E 'test(/jeff_live_oracle/)'

RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default -p rts-ai \
  -E 'test(/jeff_live_oracle/)'
```

- Generate the candidate twice and byte-compare the outputs.
- Run focused existing replay-determinism tests.
- Run `node scripts/check-docs-health.mjs` and `git diff --check`.

## Manual Test Focus

Generate a candidate under `target/`, confirm it matches the committed fixture, then modify one
command in a temporary copy. Verify the report names the exact tick, player, command index,
expected/actual payload, and surrounding input fingerprints. Open the replay only as a sanity check;
the transcript is the behavior authority.

## Non-Goals

- No SDK types or shared runtime extraction; Phase 2 owns the production-host cutover.
- No changes to `AiController::think`, `ProfileBackedScript`, cadence, placement, strategy, balance,
  fog, protocol, or simulation.
- No serialization of private controller memory.
- No broad seed sweep, map matrix, outcome golden, or AI-strength claim.
- No automatic fixture blessing.

## Handoff Expectations

Report the fixture paths, schema/version, seed/map/horizon, normal/full coverage, candidate-generation
command, exact parity commands, first-divergence artifact location, fixture size/runtime, and manual
comparator check. Tell the Phase 2 agent which bytes and fields are immutable and confirm that
offline/live parity is still intentionally unresolved.
