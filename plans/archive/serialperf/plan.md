# Serial Hellhole Performance Plan

## Purpose

Make three straightforward, semantics-preserving reductions to the canonical server-only
`supply-300-hellhole` path, one independently measured change at a time. The starting evidence is
roughly 2.4× real time and about 13.6 ms per API round trip on the designated reference MacBook,
while the project goal is at least 8.0× and at most 4.167 ms; every phase must capture a fresh
parent-versus-candidate baseline because `origin/main` continues to move. This plan deliberately
defers behavioral cadence changes, incremental/event-driven simulation, broad storage rewrites, and
parallelism.

## Constraints

- Run the default 900-tick isolated harness in release mode on the designated MacBook, on one
  serial execution lane and without a profiler attached. Do not use Rayon, worker fan-out, reduced
  tick/snapshot cadence, a smaller scenario, or a changed command stream to claim progress.
- Preserve exact simulation and projection output. Generate the 900-frame Hellhole snapshot stream
  from the phase parent and candidate, require `cmp` byte identity, record both SHA-256 hashes, and
  compare every non-timing harness summary field; aggregate event counts alone are not parity.
- Preserve deterministic ordering, panic-free stale-id handling, the public `Game` API, checkpoint
  semantics, fog authority, and all existing phase boundaries unless the phase explicitly names a
  storage-reuse change at that boundary.
- Each phase contains exactly one performance idea. Targeted tests, measurement artifacts, and the
  phase-status/evidence update are supporting work, not permission to fold in adjacent cleanup.
- Build parent and candidate release binaries with the same pinned toolchain in separate clean
  worktrees and target directories, then invoke the binaries directly so compilation is not timed.
- Commit a clean local candidate before measurement and record its SHA. Build and measure exactly
  that commit; any later runtime, test, dependency, or performance-tool edit invalidates the
  evidence and requires a new candidate commit and complete rerun.
- After two unrecorded warm-ups per binary, collect nine paired 900-tick runs, alternating
  `AB, BA, AB...` order. Keep the reference MacBook plugged in and otherwise idle, do not attach a
  profiler or tracing during timed runs, and retain every JSON result under ignored
  `target/server-perf/` storage.
- For pair `i`, compute primary gain as
  `100 × (parent.apiRoundTrip.totalUs - candidate.apiRoundTrip.totalUs) /
  parent.apiRoundTrip.totalUs`; positive is faster. Compute the confirming wall gain as
  `100 × (candidate.realtimeFactor / parent.realtimeFactor - 1)`.
- The median primary gain must clear the complexity threshold, the median wall gain must be
  positive, and at least eight of nine pairs must be positive on both measures. This predeclared
  sign requirement has one-sided binomial probability below 0.02 under an equal win/loss null and
  avoids an unimplemented bootstrap/MAD procedure.
- The minimum convincing median improvement depends on added complexity: 3% for a localized change
  with no persistent state/invalidation, 5% for several files or new internal collection semantics,
  and 8% for persistent cache/invalidation state, `DerivedState` ownership, or broad signatures.
  Passing a number never overrides a correctness or maintainability objection.
- Compare the median candidate versus parent p95 and p99 for both `tick` and `apiRoundTrip`.
  Reject p95 regression over 3% or p99 regression over 5% without compelling repeatable evidence.
- If nine pairs are genuinely inconclusive, allow one clean rerun using eleven entirely new pairs,
  unchanged binaries, and the same order protocol. Require ten of eleven positive pairs, do not
  combine the two run sets, and have a second fresh reviewer return only `ACCEPT` or `REJECT`.
- Reprofile an otherwise acceptable candidate before review. The before/after flame graphs must
  show that the intended hot cost fell rather than being moved outside the measured interval.
- A fresh read-only subagent must review the actual diff, raw measurements, parity hashes,
  non-timing fields, focused tests, flame graphs, and complexity tier after implementation. The
  reviewer must not have authored the candidate, and the implementing agent may ship an
  optimization only with an `ACCEPT` verdict.
- Each phase uses its own branch from then-current `origin/main` and commits only that phase. The
  automated `scripts/phase-runner.sh --pr` lifecycle is not safe for this plan because it may let
  `agent-pr.sh` rewrite the measured source and immediately arm auto-merge; execute each existing
  phase interactively with the `phase-runner` skill and the two-stage shipping gate below.
- After each accepted and merged phase, hand off the parent/candidate SHAs, parity hash, nine paired
  results, reviewer verdict, accepted outcome, cumulative 8× progress, and next task. Name a short
  manual test of ordinary movement/combat and replay determinism rather than an exhaustive matrix.

## Independent Review Gate

Spawn the reviewer only after the candidate and evidence are complete, and do not give it an
implementation task. Ask it to calculate the paired result itself, assign the complexity tier,
verify the benchmark and exact-output protocol, inspect ownership and bounded memory, and return
exactly `ACCEPT`, `REJECT`, or `RERUN` with reasons. `RERUN` is allowed only for genuinely
inconclusive noise and triggers the single eleven-pair retry; any byte difference, semantic-counter
difference, determinism failure, parallelism, cadence change, or work moved outside measurement is
an unconditional `REJECT`.

If the reviewer returns `REJECT`, stop the entire plan before `scripts/agent-pr.sh`: do not push an
implementation PR, mark the phase done, remove the candidate, or start the next phase. Preserve the
local candidate branch/worktree, raw ignored artifacts, exact-output hashes, tests, flame graphs,
and reviewer verdict so the user can inspect them. Report in plain language what changed, the exact
measured gain or regression versus the required threshold, what complexity was added, whether
parity/tests passed, why the reviewer rejected it, and the local branch/worktree/artifact paths;
then wait for the user to choose whether to revise, abandon and record, or override the stop.

Use this exact shape for the report:

> Phase N stopped: the independent reviewer rejected [change] because [reason]. It measured [x%]
> versus the required [y%]; parity [passed/failed] and focused tests [passed/failed]. Nothing was
> shipped, and Phase N+1 was not started. The candidate is preserved at branch [branch], worktree
> [path], parent [SHA], candidate [SHA], with evidence at [path]. Inspect it with
> `git -C [path] diff [parent]...[candidate]`. Tell me whether to revise it, abandon and record it,
> or override the stop.

## Exact Final-Source Shipping Gate

An initial `ACCEPT` permits preparation for shipping; it is not permission to auto-merge. Mark the
phase done and commit its evidence, then run:

```bash
scripts/agent-pr.sh \
  --verification "<focused checks and performance/parity gates passed>" \
  --no-auto-merge
```

This lets the mandatory adversarial quality pass update the branch without any possibility of
merge. Capture the post-quality HEAD and compare it to the measured/reviewed commit.

If runtime source, tests, dependencies, or performance tooling changed, rerun byte parity, all nine
timed pairs, profiles, focused checks, and a fresh independent review on that exact post-quality
HEAD. If only plan/evidence Markdown or automatic plan archival changed, a fresh reviewer must
verify that the runtime tree is byte-identical to the accepted candidate and may reuse the prior
measurements. A final `REJECT` leaves the non-auto-merging PR/branch intact and stops with the same
plain-language report, additionally naming the PR; nothing has shipped.

Only a final `ACCEPT` on the exact post-quality runtime tree permits removing `needs-human`, updating
the owned-PR metadata from auto-merge disabled to requested, arming `gh pr merge --auto --merge`, and
running `scripts/wait-pr.sh`. Verify the accepted runtime commit is reachable from `origin/main`
before starting the next phase. Do not rerun `agent-pr.sh` in default auto-merge mode after final
review, because another quality rewrite would invalidate the reviewed head.

## Reproducible Evidence Recipe

Use direct binaries from separate parent/candidate release target directories. Generate exact
streams and compare them with:

```bash
"$PARENT_TARGET/release/generate_hellhole_snapshot_stream" "$OUT/parent.rtsstream" 900
"$CANDIDATE_TARGET/release/generate_hellhole_snapshot_stream" "$OUT/candidate.rtsstream" 900
cmp "$OUT/parent.rtsstream" "$OUT/candidate.rtsstream"
shasum -a 256 "$OUT/parent.rtsstream" "$OUT/candidate.rtsstream"
```

For every paired harness JSON, compare the complete non-timing surface by normalizing only timing
values while retaining their sample counts:

```bash
jq 'del(.elapsedMs,.realtimeFactor)
    | .tick |= {samples}
    | .snapshotBuild |= {samples}
    | .snapshotCompact |= {samples}
    | .snapshotSerialize |= {samples}
    | .apiRoundTrip |= {samples}' parent.json > parent.semantic.json
jq 'del(.elapsedMs,.realtimeFactor)
    | .tick |= {samples}
    | .snapshotBuild |= {samples}
    | .snapshotCompact |= {samples}
    | .snapshotSerialize |= {samples}
    | .apiRoundTrip |= {samples}' candidate.json > candidate.semantic.json
cmp parent.semantic.json candidate.semantic.json
```

This retains mode, connection/transport flags, tick/simulated time, entity counts, snapshot count
and bytes, event counts, last combat tick, duration sample counts, and the complete snapshot-payload
summary. A semantic mismatch in any pair rejects the candidate.

Profile parent and candidate separately from timed runs, using identical release binaries, tick
count, and settings:

```bash
xcrun xctrace record --template 'Time Profiler' --output "$OUT/profile.trace" \
  --target-stdout "$OUT/profile-summary.json" --launch -- "$BIN" --ticks 900 --json
xcrun xctrace export --input "$OUT/profile.trace" \
  --xpath '/trace-toc/*/data/table[@schema="time-profile"]' \
  --output "$OUT/time-profile.xml"
inferno-collapse-xctrace "$OUT/time-profile.xml" | rustfilt > "$OUT/profile.folded"
inferno-flamegraph "$OUT/profile.folded" > "$OUT/flamegraph.svg"
awk '{ weight=$NF; sub(/ [0-9]+$/, "", $0); count=split($0, frame, ";");
       samples[frame[count]] += weight }
     END { for (leaf in samples) print samples[leaf], leaf }' "$OUT/profile.folded" \
  | sort -nr > "$OUT/ranked-leaves.txt"
```

Retain the trace/XML/folded/SVG/ranked-leaf artifacts and explicitly sum inclusive folded samples
containing the intended before/after function family. Never mix profiled timing with the nine-pair
acceptance results.

## Phase Summaries

### [Phase 1 - Use Narrow Integer Hashing](phase-1.md)

Replace randomized general-purpose hashing only for the server-allocated `u32` keys inside
`EntityStore`, leaving maps with uncontrolled keys unchanged. Preserve sorted iteration, serde,
checkpoint, lookup, and stale-id behavior while avoiding a global hasher conversion. Accept it only
if byte parity holds and the independent reviewer finds a low-complexity gain of at least 3%.

### [Phase 2 - Reuse Ordered Entity IDs](phase-2.md)

Stop rebuilding and sorting the complete live-id list for every immutable `EntityStore` iteration
while retaining stable ascending iteration and fallible lookup. Keep the change inside the store
and its direct tests rather than replacing the store with an ECS or broad dense-component model.
Accept it only if byte parity holds and the independent reviewer finds a gain of at least 5% worth
the extra index/bookkeeping semantics.

### [Phase 3 - Reuse Spatial Index Storage](phase-3.md)

Reuse allocated spatial-grid storage across the existing named rebuild boundaries instead of
allocating a fresh map-sized vector-of-vectors each time. Preserve every rebuild point, cell
membership, sorted-id order, collision behavior, and final snapshot-interest index. Accept it only
if the independent reviewer considers the measured gain convincing relative to scratch-state
ownership and retained memory.

## Measured Checkpoint

If all three phases receive `ACCEPT` and merge, Phase 3 must run the canonical 900-tick benchmark
against the pre-plan commit and final candidate, report cumulative accepted gain and remaining
distance to 8.0×, and inspect a fresh flame graph before it is marked done. A reviewer `REJECT`
stops the plan earlier under the Independent Review Gate; do not automatically promote deferred
work into phases.

## Deferred Backlog

- Persist static occupancy data only after accounting for its ownership cost: `Occupancy` currently
  borrows `Map`, so cross-tick reuse first requires separating cached `OccupancyData` from the
  map-bound view and updating the derived-state registry.
- Reuse additional combat, movement, or collision candidate vectors only after new allocation
  attribution identifies one buffer family; collision already reuses its candidate vector within a
  tick.
- Cache within-call unit-body geometry/trigonometry while preserving identical floating-point
  operation order. Persistent body caches require position/facing/kind invalidation and are not a
  straightforward first-cohort change.

## Execution

After review and approval, execute one existing phase at a time with the `phase-runner` skill in its
own clean worktree. Do not use the automated `scripts/phase-runner.sh --pr` path until it supports a
post-quality, pre-auto-merge performance-review hook. Complete the Exact Final-Source Shipping Gate
and definite merge for one phase before starting the next.
