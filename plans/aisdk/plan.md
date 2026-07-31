# Practical AI SDK Plan

## Purpose

Deliver the highest-value first 80% of an AI authoring SDK without designing a universal or
"godlike" AI framework. The work first freezes the real production behavior of Jeff's AI and makes
offline tools execute that same runtime, then extracts typed observations, conservative task
feedback, rule/query services, and transactional actions behind compatibility seams. The final two
phases prove the surfaces are real by first moving Jeff and the shared profile engine onto them,
then separately removing obsolete shims and installing architecture ratchets, while requiring the
original production transcript to remain exact.

## Overall Constraints

- Production Jeff behavior is immutable throughout this plan. Every phase after Phase 1 must pass
  the checked-in live-controller transcript without regenerating it or changing invocation ticks,
  empty batches, command order, unit IDs, coordinates, queue flags, retreat ordering, recipient
  events, or post-tick state fingerprints.
- Phase 2 intentionally changes old offline matchup and arena results where their six-tick cadence,
  tick-zero think, build search, missing retreat reflexes, or duplicated filtering differ from
  production. This is a tooling correction; live Jeff remains unchanged.
- Preserve the server-authoritative boundary: `Game` remains AI-free, `rts-sim` must not depend on
  `rts-ai`, every AI command still passes through ordinary `Game::enqueue` validation and replay
  logging, and no SDK service receives simulation authority.
- Preserve fog authority. Public AI data and queries may use only the player's fog-filtered view,
  owner-private facts already available to that player, controller state, and public static
  map/start data; they may never consult full snapshots, hidden occupancy,
  simulation path caches, private line-of-sight state, or hidden blockers.
- Prefer honest epistemic names. `NoKnownConflict`, `Issued`, `ObservedActive`,
  `Unknown`, and `TimedOut` must not be presented as authoritative legal/clear/accepted/rejected
  answers.
- Treat deterministic ordering as behavior. Preserve controller order, helper-call order,
  caller-ordered worker and producer candidates, stable ID tie-breaks, existing `f32` calculations,
  and command-vector order unless a later user explicitly approves a gameplay change.
- Compatibility must preserve known Jeff quirks, including synthetic unseen-resource amounts,
  steel-only pending-build commitments, current combat-kind lists, pending-build expiry and failed
  site behavior, placement ring scans, and stage/attack suppression. New strategies may use more
  truthful SDK representations without silently switching Jeff to them.
- Build the SDK inside `rts-ai` as a curated Rust authoring seam. Do not create an external plugin
  ABI, behavior-tree/GOAP framework, async planner, general tactical solver, or stable cross-language
  contract in this effort.
- Keep each phase independently reviewable. Do not combine opportunistic balance, targeting,
  production, placement, or tactical improvements with these refactors.
- Update `docs/design/ai.md` and `docs/design/testing.md` as their source-of-truth contracts evolve.
  Update `docs/design/server-sim.md` and balance/rules documentation only when a phase moves an
  authoritative cross-crate owner.
- A fresh sub-agent must implement each phase through the repository's `phase-runner` workflow.
  Each phase lands on its own `zvorygin/` branch, is pushed as an owned PR with auto-merge armed,
  and waits until the PR is definitely merged and its head is reachable from `origin/main` before
  the next phase starts.
- When a phase is complete, mark its phase document done in that phase's implementation commit.
  After each phase, the implementing agent must provide a handoff describing what changed, what the
  next agent should do, the exact parity evidence, and the core features that should be manually
  tested.

## Phase Summaries

### [Phase 1 - Freeze Production Jeff](phase-1.md)

Create a versioned transcript oracle that runs the actual production `AiController` loop and records
every pre-tick invocation, input fingerprint, ordered command batch, recipient event digest, and
post-tick state digest. Keep one bounded Jeff-versus-Jeff fixture, using a cheap prefix in the normal
gate and a longer continuation under `RTS_FULL_AI_TESTS=1`, with actionable first-divergence output.
This phase changes no AI behavior or runtime; it establishes the immutable compatibility specimen
used by every later phase, while Phase 2 closes its host-orchestration blind spot.

### [Phase 2 - Canonicalize Live and Offline Execution](phase-2.md)

Extract one shared AI tick driver from the production host, make `AiController` the only normal
profile runtime, and replace `ProfileBackedScript`'s duplicated cadence, placement, memory, and
filtering with thin host adapters. Route the live room, Phase 1 oracle, matchup, arena, balance,
self-play, and relevant tests through that driver, including nine-tick staggering and retreat
injection. Accept intentional offline baseline changes, but require Phase 1's Jeff transcript to
remain byte-for-byte unchanged through the cutover.

### [Phase 3 - Add the Typed Authoring Seam](phase-3.md)

Add a public, object-safe `rts_ai::sdk` lifecycle centered on a typed, fog-safe `AiFrame` and a small
command sink, keeping raw protocol DTOs and string parsing inside one adapter. Run current profiles
through a crate-private `LegacyProfileStrategy` whose compatibility projection recreates every
historical `AiObservation` quirk exactly. Prove frame secrecy, lifecycle determinism, external-crate
usability, and full Jeff transcript parity without changing the simulation or wire protocol.

### [Phase 4 - Add Observational Task Feedback](phase-4.md)

Add deterministic intent IDs and conservative observational feedback only for build, gather, move,
and setup actions whose own-state evidence is already available. Consolidate existing pending-build
and stage/attack bookkeeping under one runtime-owned compatibility owner while preserving its exact
update points, timeouts, filters, and ordering. Defer causal train, research, attack, and event-based
tracking; task status remains non-authoritative and does not feed Jeff's policy.

### [Phase 5 - Expose Rules and Known-World Queries](phase-5.md)

Expose a faction-bound `AiRulebook` assembled from authoritative `rts-rules`, and move pure upgrade
definitions into that rules authority with a simulation compatibility shim. Add deterministic
known-world indexes, coordinate helpers, static terrain connectivity, and Jeff-compatible build-site
search whose types state their uncertainty and never consult hidden simulation state. Explicitly
defer the current defensive firing-lane approximation so this phase does not import combat or
line-of-fire semantics into the first query surface.

### [Phase 6 - Expose the Transactional Action Planner](phase-6.md)

Promote the existing per-think budget, worker/node/producer reservations, command accumulation, and
tactical group canonicalization into a public `AiActionPlanner`. Cover the common paid build,
resume, train, research, gather, move, attack, hold, and Anti-Tank Gun setup operations with atomic
local blockers and a narrow SDK-owned uncommon-action enum. Keep Jeff on mechanical compatibility
wrappers and preserve its steel-only commitments, candidate ordering, trace labels, and command-call
order.

### [Phase 7 - Cut Jeff Over to the SDK](phase-7.md)

Move Jeff and the shared profile decision engine onto `AiFrame`, `AiRulebook`, `WorldQueries`, the
task compatibility layer, `AiActionPlanner`, and minimal `UnitGroup` helpers without changing their
policy. Migrate one slice at a time and preserve all legacy projections, placement arithmetic,
candidate ordering, runtime memory, traces, and command construction behind compatibility seams.
Do not perform broad cleanup or add architecture enforcement in this parity-only cutover PR.

### [Phase 8 - Clean Up and Ratchet the SDK](phase-8.md)

After the cutover has merged and parity has been independently rerun, delete only adapters and
duplicated helpers proven obsolete. Add an outside-crate conformance strategy exercising the full
first-80% authoring path and a narrow architecture checker that confines protocol parsing and raw
command construction to documented boundaries. Preserve the immutable Jeff transcript while
documenting all legitimate low-level exceptions and deferred gold-architecture work.

## Phase Index

1. [Phase 1 - Freeze Production Jeff](phase-1.md)
2. [Phase 2 - Canonicalize Live and Offline Execution](phase-2.md)
3. [Phase 3 - Add the Typed Authoring Seam](phase-3.md)
4. [Phase 4 - Add Observational Task Feedback](phase-4.md)
5. [Phase 5 - Expose Rules and Known-World Queries](phase-5.md)
6. [Phase 6 - Expose the Transactional Action Planner](phase-6.md)
7. [Phase 7 - Cut Jeff Over to the SDK](phase-7.md)
8. [Phase 8 - Clean Up and Ratchet the SDK](phase-8.md)

## Required Verification Themes

Every implementation phase must select the smallest relevant subset while always running the exact
Phase 1 Jeff oracle once it exists:

- Focused `rts-ai` nextest coverage and the full opt-in Jeff transcript continuation.
- Deterministic reruns and first-divergence diagnostics for transcript-sensitive changes.
- Existing replay verification, while explicitly treating it as simulation-after-command evidence
  rather than AI-generation parity.
- Focused `rts-rules` tests when rule ownership or catalog façades change.
- Crate-boundary and simulation-architecture checks.
- Fog A/B tests showing that worlds differing only in hidden state produce identical SDK frames and
  known-world query results.
- Public integration tests compiled outside `rts_ai` private module visibility.
- `node scripts/check-docs-health.mjs` and `git diff --check`.
- The authoritative GitHub `Main test gate` for every phase PR.

## Deferred Gold-Architecture Backlog

- Simulation-correlated authoritative command receipts and detailed rejection reasons.
- Recipient-event delivery and causal train, research, attack, death, or supersession tracking.
- Idempotent cross-tick `ensure_*` goals and a general goal scheduler.
- Behavior trees, GOAP, asynchronous planning, influence maps, or automated strategic decomposition.
- Omniscient or hidden-state-sensitive placement, pathing, target-legality, and line-of-fire queries.
- Dynamic path ETA, vehicle-body route prediction, traffic-aware routing, and formation solvers.
- Persistent squad brains, automatic role assignment, or cross-tick tactical blackboards.
- Strategy checkpoint/restore beyond what concrete evidence later requires.
- A stable plugin ABI, dynamic plugins, or non-Rust/cross-process AI SDK.
- Broad seed/map certification or AI strength claims; the transcript fixture protects behavior, not
  balance quality.

## Implementation Process

After this plan is approved, execute it serially with a fresh sub-agent for every phase:

```bash
scripts/phase-runner.sh --plan aisdk phase-1 --pr --wait
scripts/phase-runner.sh --plan aisdk phase-2 --pr --wait
scripts/phase-runner.sh --plan aisdk phase-3 --pr --wait
scripts/phase-runner.sh --plan aisdk phase-4 --pr --wait
scripts/phase-runner.sh --plan aisdk phase-5 --pr --wait
scripts/phase-runner.sh --plan aisdk phase-6 --pr --wait
scripts/phase-runner.sh --plan aisdk phase-7 --pr --wait
scripts/phase-runner.sh --plan aisdk phase-8 --pr --wait
```

Do not run the chain without `--wait`. Planning and final review remain manual, and any exact Jeff
transcript divergence stops the chain until the implementation is corrected or the user explicitly
authorizes a behavior change.
