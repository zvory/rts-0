# Phase 1 - Use Narrow Integer Hashing

## Phase Status

- [x] Done.

## Objective

Replace randomized general-purpose hashing only for server-allocated `u32` entity ids, preserving
all `EntityStore` behavior and outputs.

## Preconditions

- Start from current `origin/main` and record its SHA as the phase parent.
- Read `docs/context/server-sim.md`, entity-store checkpoint/serialization paths, and the shared
  acceptance protocol in `plans/serialperf/plan.md`.
- Confirm no other phase is modifying `EntityStore`.

## Work

- Give only `EntityStore`'s internal `HashMap<u32, Entity>` a deterministic integer-key hasher that
  avoids randomized general-purpose hash work. Measure candidate hashers rather than assuming an
  identity hasher is fast with hashbrown's control-byte fingerprints; keep the chosen implementation
  or dependency narrow, reviewable, and inaccessible to maps with client-controlled inserted keys.
- Keep `get`, `get_mut`, insertion, removal, stale-id no-op behavior, monotonically increasing ids,
  ascending iteration, checkpoint export/import, `Clone`, serde JSON shape/roundtrip, and current
  `Default` versus `new()` behavior unchanged.
- Do not convert other simulation/server maps, change entity storage shape, add ordered-id state,
  change map capacity/reservation/load-factor policy, introduce unchecked indexing, or combine this
  with Phase 2's sorting work. Every `Hasher::write*` path reachable from the panic-free tick must
  handle input without `panic!`, `unreachable!`, `unwrap`, or `expect`.
- Add focused tests covering lookup/insertion/removal, stable iteration, serialization/restore, and
  stale ids. Document the narrow safety boundary if it is not obvious from the type/API.

Expected touch points are `server/crates/sim/src/game/entity/store.rs`, its focused tests, and
possibly a narrowly scoped dependency declaration or local hasher helper.

## Exact Output Gate

Build `generate_hellhole_snapshot_stream` in separate release target directories from the parent
SHA and candidate head. Generate 900 frames to distinct ignored or `/tmp` paths, require `cmp` to
succeed, and record identical `shasum -a 256` values plus matching non-timing Hellhole summary
fields in this phase file.

Any mismatch is an unconditional rejection. Also run focused entity/checkpoint tests and the sim
architecture check.

## Performance Gate

- Build separate parent and candidate `hellhole-perf-harness` release binaries and warm each twice.
- Run nine paired 900-tick comparisons with alternating order and retain all JSON outputs under
  `target/server-perf/serialperf-phase-1/`.
- Report every pair plus median API total, elapsed/realtime factor, and tick/API tails. This should
  clear 3% only if the reviewer confirms localized low complexity with no new dependency or
  invariant; otherwise apply the 5% or 8% tier from `plan.md`. Eight of nine pairs and all common
  semantic/tail gates must pass.
- Capture before/after flame graphs and confirm entity hashing cost fell rather than moving work
  outside the measured interval.

## Independent Complexity Review

After the candidate and evidence exist, spawn a fresh read-only subagent. Require it to inspect the
parent-to-head diff, hasher scope/safety, tests, parity hashes, non-timing fields, flame graphs, and
all raw paired measurements, calculate the result itself, assign a complexity tier, and return
`ACCEPT`, `REJECT`, or a justified one-time `RERUN`.

If accepted, keep the implementation and record the verdict here. If rejected, do not mark this
phase done, push a PR, delete the candidate, or start Phase 2; preserve the local branch/worktree and
artifacts, return a blocked handoff, then stop and give the user the plain-language inspection
report required by `plan.md`. Failure to spawn a fresh reviewer or obtain a well-formed verdict is
also blocked, never permission to self-accept.

## Accepted Evidence

- Parent: `a568ea6da8b5ba80382991188e18ef7e061a83f8`.
- Measured runtime candidate: `327fb37ab3a614f4bd84a2b745667370bffd335c`.
- Scope: one private `EntityStore` map alias and local total/panic-free hasher, plus three focused
  tests. No dependency, persistent state, capacity-policy change, or extra memory; the independent
  reviewer assigned the localized 3% tier.
- Hasher selection: three preliminary alternating 300-tick pairs measured a 13.394% median API gain
  for identity hashing and 11.936% for an odd-multiplier mix. Identity hashing was retained.
- Exact output: both 900-frame streams were byte-identical at 23,997,915 bytes with SHA-256
  `0a6e4b40c8af8ab01aae08755fb6e8811584a38aa746682943ff7131901142d1`.
- Every normalized non-timing field matched in all nine official pairs.

| Pair | API gain | Wall-factor gain |
| ---: | ---: | ---: |
| 1 | 13.946% | 16.191% |
| 2 | 11.623% | 13.151% |
| 3 | 8.733% | 9.541% |
| 4 | 3.369% | 3.464% |
| 5 | 14.783% | 17.372% |
| 6 | 16.041% | 19.113% |
| 7 | 7.903% | 8.569% |
| 8 | 28.157% | 39.127% |
| 9 | 42.322% | 73.374% |

- Nine of nine pairs were positive on both measures. Median API gain was 13.946% and median wall
  gain was 16.191%. Pairs 8 and 9 experienced a whole-machine slowdown; excluding those outliers,
  the first-seven medians remained 11.623% API and 13.151% wall.
- Median tails improved: tick p95 13.068%, tick p99 7.534%, API p95 13.469%, and API p99 7.688%.
- Profiles retained identical semantic summaries. Inclusive `RandomState` samples fell from
  483/3,583 (13.480%) to 113/2,958 (3.820%), and SipHash samples fell from 155/3,583 (4.326%) to
  46/2,958 (1.555%). The remaining samples belong to unchanged maps.
- Focused checks passed: four `entity_store` tests, twelve `checkpoint_payload_` tests, the sim
  architecture check, `rts-sim --lib` clippy with warnings denied, and `git diff --check`.
  `rts-sim --all-targets` clippy remains blocked by unrelated pre-existing test warnings.
- Independent initial review verdict: `ACCEPT`. It confirmed the 3% tier, scope and panic safety,
  exact parity, all semantic and sign gates, tail improvements, and profiler attribution.
- Raw ignored evidence is retained under `target/server-perf/serialperf-phase-1/` in the task
  worktree. No PR was opened before the requested user evidence review.

## Completion and Handoff

Only after the initial `ACCEPT`, mark this file `Done` in the implementation/evidence commit and run
focused checks. Follow the common Exact Final-Source Shipping Gate: open/update the PR with
auto-merge disabled, obtain final review on the exact post-quality source, and only then arm
auto-merge and wait. Verify the accepted runtime commit is an ancestor of `origin/main`, then hand
off SHAs, exact-output hash, pair results, both reviewer verdicts, cumulative realtime factor, and
Phase 2 instructions. A rejection uses the stop-and-report path and has no Phase 2 handoff.

Manual testing focus: issue ordinary mixed-unit movement and combat commands and inspect a short
deterministic replay for unchanged ordering and behavior.
