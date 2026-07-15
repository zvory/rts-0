# Phase 2 - Reuse Ordered Entity IDs

## Phase Status

- [ ] Not started.

## Objective

Eliminate repeated collection and sorting of all live entity ids during immutable `EntityStore`
iteration while preserving exact stable-id semantics and any accepted Phase 1 hasher.

## Preconditions

- Phase 1 received initial and final `ACCEPT` verdicts and merged. A Phase 1 rejection stops this
  plan; do not start Phase 2 without explicit user direction.
- Start from current `origin/main`, record its SHA, and read `docs/context/server-sim.md`, the
  entity-store checkpoint/serialization paths, and `plans/serialperf/plan.md`.
- Confirm no other phase is modifying `EntityStore` or its checkpoint representation.

## Work

- Keep the existing entity map and add one explicit, bounded ascending live-id index used by both
  immutable `iter()` and `ids()` so neither recollects and sorts all map keys on every call.
- Keep `get`, `get_mut`, removal, stale-id no-op behavior, monotonically increasing ids, checkpoint
  export/import, serde behavior, and deterministic iteration unchanged.
- Treat the ordered index as duplicated rebuildable bookkeeping with a single owner and explicit
  insertion/removal/clone/deserialize/restore invariants. It must not alter authoritative serde or
  checkpoint shape; rebuild it from final unique map keys after direct deserialization and
  `from_checkpoint_entities` rather than relying on `#[serde(skip)]` alone.
- Preserve missing removal, real removal, replacement/collision behavior if the wrapping id
  allocator reaches an existing id, and current `Default` behavior. Do not introduce `BTreeMap`, an
  ECS, component split, unchecked id indexing, broad `Vec<Option<Entity>>` conversion, or unrelated
  map/hasher changes.
- Add focused invariant tests after insertion, missing and real removal, clone, serde roundtrip,
  checkpoint restore, and duplicate/replacement inputs. Update `docs/design/server-sim.md` if the
  state-ownership registry requires a new rebuildable field row.

Expected touch points are `server/crates/sim/src/game/entity/store.rs`, its focused tests, and
possibly the state-ownership registry section of `docs/design/server-sim.md`.

## Exact Output Gate

Build `generate_hellhole_snapshot_stream` in separate release target directories from the parent
SHA and candidate head. Generate 900 frames to distinct ignored or `/tmp` paths, require `cmp` to
succeed, and record identical `shasum -a 256` values plus matching non-timing harness fields in this
phase file.

Any mismatch is an unconditional rejection. Also run focused entity/checkpoint tests and the sim
architecture check.

## Performance Gate

- Build separate parent and candidate `hellhole-perf-harness` release binaries and warm each twice.
- Run nine paired 900-tick comparisons with alternating order and retain all JSON outputs under
  `target/server-perf/serialperf-phase-2/`.
- Report every pair plus median API total, elapsed/realtime factor, and tick/API tails. New internal
  collection semantics are at least medium complexity and must clear 5%, eight of nine pairs, and
  all common semantic/tail gates from `plan.md`.
- Capture before/after flame graphs and confirm ID collection/sorting cost fell rather than moving
  work outside the measured interval.

## Independent Complexity Review

After the candidate and evidence exist, spawn a fresh read-only subagent. Require it to inspect the
parent-to-head diff, duplicated-state invariants, tests, parity hashes, non-timing fields, flame
graphs, and all raw paired measurements, calculate the result itself, assign a complexity tier, and
return `ACCEPT`, `REJECT`, or a justified one-time `RERUN`.

If accepted, keep the implementation and record the verdict here. If rejected, do not mark this
phase done, push a PR, delete the candidate, or start Phase 3; preserve the local branch/worktree and
artifacts, return a blocked handoff, then stop and give the user the plain-language inspection
report required by `plan.md`. Failure to spawn a fresh reviewer or obtain a well-formed verdict is
also blocked, never permission to self-accept.

## Completion and Handoff

Only after the initial `ACCEPT`, mark this file `Done` in the implementation/evidence commit and run
focused checks. Follow the common Exact Final-Source Shipping Gate: open/update the PR with
auto-merge disabled, obtain final review on the exact post-quality source, and only then arm
auto-merge and wait. Verify the accepted runtime commit is an ancestor of `origin/main`, then hand
off SHAs, exact-output hash, pair results, both reviewer verdicts, cumulative realtime factor, and
Phase 3 instructions. A rejection uses the stop-and-report path and has no Phase 3 handoff.

Manual testing focus: issue ordinary mixed-unit movement and combat commands and inspect a short
deterministic replay for unchanged ordering and behavior.
