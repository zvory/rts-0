# Phase 0 - Build the 300-Supply Lab Hellhole

## Phase Status

- [x] Done. Dispensed: the existing server-heavy Lab scenario and client-only Hellhole snapshot stream
  provide separate server and client saturation lanes; this combined end-to-end phase is
  intentionally not being pursued.

The remaining text is retained as historical context, not as an executable phase specification.

## Objective

Create one checked-in, server-authoritative Lab scenario on the authored `1v1` map that keeps two
exact 300-supply armies in a dense, sustained fight. Both players use Lab god mode, every ordinary
supply-bearing unit kind is represented on each side, and the two armies are interleaved as tightly
as authoritative placement permits so weapons, recoil, projectiles, smoke, muzzle effects, HP,
selection, fog-independent full-world projection, snapshot compression, decode/apply, and rendering
all receive a deliberately hostile workload.

Register the scenario as one canonical client-performance workload and prove that it is repeatable.
The checked-in scenario and workload assertions are the durable benchmark definition; timing output
is disposable machine-local evidence and should be regenerated when comparing branches.

## Benchmark Role

- This is the first measured checkpoint and must merge before Phase 1 begins.
- It is an end-to-end **Lab stress benchmark**: authoritative simulation, recipient projection,
  ordinary snapshot serialization/compression, WebSocket delivery, browser decode/apply, and Pixi
  rendering all remain on their production paths.
- Lab startup is spectator-shaped, uses full-world Lab projection, and omits normal active-player
  prediction. It is therefore a comparative worst-case stress fixture, not by itself proof that a
  production 300-supply cap is safe. Phase 1 retains the active-player workload needed for that
  separate conclusion.
- Do not make the benchmark depend on a permanently retained timing artifact. Later optimization
  phases should rerun the same checked-in workload on the relevant base and candidate revisions.

## Scenario Contract

- Add one bundled checkpoint-backed Lab scenario with a stable id such as
  `supply-300-hellhole`; do not replace or mutate `lategame` or `render-preview`.
- Use the authored `1v1` map, exactly two opposing players, current playable factions, full relevant
  research, ample resources/oil, and Lab god mode enabled for both players.
- Give each player exactly 300 used supply according to the authoritative rules. Each side must own
  at least one of every ordinary supply-bearing unit kind available through the Lab spawn catalog;
  exclude buildings, resource nodes, visual-only entities, and test-fixture-only kinds. Fill the
  remaining supply with a deterministic round-robin mix rather than a hand-tuned favorable army.
- Record and assert the exact per-owner/per-kind counts produced by that deterministic composition.
  A future unit addition must make the contract fail visibly until the scenario is deliberately
  regenerated or versioned.
- Place the armies in one compact central passable region, alternating owners through a
  deterministic dense lattice at the minimum legal spacing accepted by authoritative setup
  validation. Do not overlap impassable terrain, buildings, or invalid unit footprints merely to
  inflate density.
- Orient and seed ordinary authoritative combat state so both sides immediately acquire targets.
  Deploy support weapons where required, enable existing autocast behavior, and use deterministic
  orders/targets to keep tanks, small arms, support weapons, mortars, artillery, Panzerfausts, smoke,
  recoil, and muzzle/projectile effects active wherever their normal rules permit. Do not add a
  benchmark-only damage, cooldown, weapon, or particle system.
- Start a deterministic subset of units at positive partial HP before god mode is applied so the
  normal HP-bar path remains populated without allowing the fight to destroy the armies.
- God mode must prevent unit/building deaths without suppressing target acquisition, firing,
  projectiles, impact feedback, or ordinary cooldown progression. The scenario must remain busy
  after at least 60 seconds of authoritative simulation.
- Keep the exported checkpoint-backed JSON below the existing Lab scenario import/message caps.
  Generate it through the current Lab/checkpoint tooling or a deterministic repository script;
  never hand-edit the embedded checkpoint payload.
- Pin an initial camera that frames the dense combat region. Whole-map zoom remains a separate
  harness setting used by the later performance matrix.

## Workload and Assertions

Add a `supply-300-lab-hellhole` workload to the existing client performance harness. It must launch
the bundled scenario through the ordinary Lab room path and fail before sampling unless all of the
following are true:

- the selected scenario id and map are exact;
- there are two opposing players and both appear in `godModePlayers`;
- both players report exactly 300 used supply;
- the exact per-owner/per-kind counts match the checked-in descriptor;
- every required unit kind is present and the projected entity count meets the descriptor;
- the browser is in Lab mode with full-world projection, the normal MessagePack snapshot codec is
  active, and at least two successful rendered frames arrived after setup;
- combat remains active and the entity count remains stable through the sample window.

Run one short canonical cell at the default viewport, DPR 1, and CPU 1 for at least 10 seconds. Save
the ordinary raw harness summary under ignored `target/` output only long enough to inspect the
fixture and hand off its path. Phase 0 timing is provisional because Phase 1 changes frame ownership
and presentation attribution; Phase 1 must recapture the honest baseline without changing the
scenario or workload descriptor.

## Constraints and Non-Goals

- Do not change production supply caps, balance, weapons, cooldowns, damage, simulation tick rate,
  snapshot rate, compression format, fog rules, or protocol shapes.
- Do not add benchmark-only client entities, duplicate particle effects, blanket animation loops,
  or client-mutated setup state. Stress must come from ordinary authoritative units and combat.
- Do not add a CPU/DPR/device matrix, hard CI timing gate, retained benchmark database, or replay
  export in this phase. One deterministic scenario, one workload, and exact structural assertions
  are enough.
- Do not optimize the renderer, rigs, frame-entity path, fog, minimap, HP, selection, or trenches in
  Phase 0. This phase establishes the workload against which those changes can be judged.

## Expected Touch Points

- `server/assets/lab-scenarios/manifest.json`
- one generated checkpoint-backed scenario under `server/assets/lab-scenarios/`
- current Lab/checkpoint generation tooling, or one focused deterministic generator if the existing
  tooling cannot reproduce the scenario safely
- `scripts/client-perf/workloads.mjs`
- `scripts/client-perf-harness.mjs` only for the exact workload assertions that are not already
  supported
- focused Lab catalog/checkpoint, launch, workload, and harness contracts
- `docs/design/client-ui.md`, `docs/design/server-sim.md`, and `docs/perf-tracing.md` only where the
  new bundled scenario/workload needs catalog or benchmark documentation

## Focused Verification

Add deterministic tests for scenario catalog validity, checkpoint restoration, exact supply and
per-kind composition, god mode, legal dense placement, preserved combat orders, stable entity
count, and sustained firing after 60 simulated seconds. Prove the harness rejects the wrong
scenario, map, supply, unit counts, god-mode state, codec, or a quiet/non-rendering room.

Run the smallest matching focused commands, including:

```bash
cargo test --manifest-path server/Cargo.toml lab_scenario
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint_lab
node tests/client_contracts.mjs
node scripts/client-perf-harness.mjs --workload supply-300-lab-hellhole --seconds 10
node scripts/check-docs-health.mjs
node tests/select-suites.mjs --verify
git diff --check
```

Use narrower Rust filters if the implementation adds a specifically named scenario test; do not
substitute a timing threshold for the structural assertions. GitHub's `Main test gate` remains the
authoritative full suite.

## Interact Lab Manual Test

Use the project-local `interact` skill to launch the bundled scenario through Lab. Inspect the
authoritative setup, advance at normal speed for at least 30 seconds, and verify that both armies
remain densely interleaved, invulnerable, target-rich, and visibly firing without missing textures,
blank output, or rapid entity loss.

Capture one clean Pixi PNG of the central hellhole, inspect it once, close the session, and include
only the returned Tailnet Preview URL in the handoff. The image is visual confirmation of the
fixture, not performance evidence.

## Player-Facing Outcome

Local developers gain a deliberately pathological bundled Lab setup for repeatable client stress
comparisons. Production matches, balance, supply limits, and ordinary Lab mechanics are unchanged.

## PR and Handoff Requirements

- Implement on a fresh `zvorygin/` branch from current `origin/main` and mark this phase Done only
  after the scenario, workload, assertions, and focused verification are complete.
- Run `scripts/agent-pr.sh --verification "<scenario, workload assertions, focused checks, and Interact review passed>"`,
  then `scripts/wait-pr.sh <pr>` and verify reachability before Phase 1.
- The handoff must include the scenario/workload ids, exact composition table, god-mode and sustained
  combat proof, scenario byte size, codec and projected-entity assertions, provisional raw summary
  path, Interact Preview URL, and the exact Phase 1 command that recaptures the honest baseline.
