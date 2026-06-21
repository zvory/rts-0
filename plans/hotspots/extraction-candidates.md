# Hotspot Extraction Candidates

Generated for Phase 3 from the Phase 1 baseline and Phase 2 responsibility map. This is a cleanup
backlog, not an implementation plan: no runtime source, tests, protocol files, client modules, CSS,
or design docs were changed in this phase.

## Ranking Method

Candidates are ranked by combining the hotspot evidence with the responsibility-map seams:

- cognitive-load reduction: how much less code a reviewer or agent would need to load at once;
- churn and recency: current-file score, recent churn, recent co-change degree, and blame freshness;
- size: current non-empty LOC and whether the file is acting as an aggregate;
- contract centrality: whether the file is a mirrored protocol, balance, fog, or `Game` API surface;
- behavior-change risk: whether the split can be mechanical or changes ordering/semantics;
- history-tracking risk: whether a split would make future churn analysis lose identity unless the
  follow-up tracks architectural groups;
- ownership conflicts: whether another active plan owns the area;
- verification confidence: whether focused tests already cover the seam.

The result favors small mechanical decompositions that preserve public entry points and existing
tests. High-score contract surfaces are ranked lower when a split needs design work or active-owner
coordination before code moves.

## Ranked Backlog

| Rank | Candidate | Classification | Follow-up plan? | Why here |
| ---: | --- | --- | --- | --- |
| 1 | Split `tests/client_contracts.mjs` by contract area | Safe mechanical decomposition | Yes, first | Top file hotspot, very high review-context payoff, no runtime behavior change |
| 2 | Split command-service tests and pure input guards | Mechanical first, then narrow service split | Yes | Third file hotspot, high recent churn, clear pure guard seams, but command ordering stays central |
| 3 | Split broad sim `Game` tests by behavior family | Safe test-suite decomposition | Yes | Fourth file hotspot, large domain clusters, strong payoff without runtime edits |
| 4 | Extract HUD resource/control-group/command-card helpers | Narrow client UI decomposition | Maybe | Medium size but high recent churn and good contract coverage |
| 5 | Extract `GameState` visual-effect or selection helpers | Narrow model decomposition | Maybe | Helps client context loading, but public `GameState` methods must stay stable |
| 6 | Extract small `Match` app-shell collaborators | Medium-risk client shell decomposition | Maybe | Good payoff, but frame/teardown ordering makes it less mechanical than HUD |
| 7 | Split AI self-play tests and harness helpers | Test-suite decomposition | Maybe | High score and churn, but lower current-line freshness and environment-gated coverage |
| 8 | Protocol mirror cleanup as a Rust plus JS group | Design-first contract plan | Yes, separate | High churn and coupling, but independent file splits are unsafe |
| 9 | Balance/config mirror cleanup or validation | Design-first contract plan | Yes, separate | Frequent edits, but numeric mirror mistakes are gameplay-facing |
| 10 | CSS section modularization | Defer unless visual workflow is ready | No, not first | Large file, but selector/load-model risk and weak automated coverage |
| 11 | Room task runtime extraction | Defer to active room owner | No, not from this plan yet | Second file hotspot, but active cleanup owns the surface |

## 1. Client Contract Suite Split

Proposed extraction:

- Keep `node tests/client_contracts.mjs` as the stable command.
- Move shared fake DOM, fake Pixi, fake audio, fake storage, assertion, and fixture helpers into
  `tests/client_contracts/` helper modules.
- Split existing section clusters into imported contract files such as HUD, protocol, lobby, lab,
  state/input, renderer, audio, observer-analysis, and config contracts.

Likely touched by a later cleanup plan:

- `tests/client_contracts.mjs`
- new `tests/client_contracts/*.mjs` helper and contract modules
- possibly `tests/select-suites.mjs` only if suite selection names need to reference split files

What stays stable:

- the top-level `node tests/client_contracts.mjs` command;
- every existing assertion and fixture behavior;
- Node-only, dependency-free execution;
- protocol/config mirror checks remaining close to parity guardrails.

Verification:

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs` if protocol/config assertions move into their own files

Manual review focus:

- confirm failures still identify the contract area clearly;
- confirm the split does not hide client architecture assertions or make the suite browser-only;
- check that helper sharing is simple enough that future tests know where to add fixtures.

Expected player-facing impact:

- None. This is test organization only.

Why rank 1:

- Phase 1 ranked `tests/client_contracts.mjs` first at score 95.43, with 9,028 non-empty LOC, 279
  rename-aware touches, 10,721 recent churn, and 220 recent co-change partners.
- Phase 2 found clear existing section boundaries and low runtime risk.
- The file is a high-degree hub for client, protocol, config, HUD, and architecture checks; splitting
  it would most reduce review context while preserving coverage.

History caveat:

- A split will make path-level history less obvious. The follow-up should preserve one top-level
  runner and update future hotspot grouping to treat `tests/client_contracts/**` as one contract
  test group.

## 2. Command-Service Guards And Tests

Proposed extraction:

- First split the in-file command-service tests by behavior family.
- Extract pure input shaping, id dedupe/capping, command-budget validation, non-finite target
  rejection, and authority checks into a small command guard module.
- Leave `apply_commands` as the orchestration entry point until a later design pass proves more
  ordering can move safely.

Likely touched by a later cleanup plan:

- `server/crates/sim/src/game/services/commands.rs`
- new command-service test modules under the same service ownership boundary
- a new internal guard/helper module near `server/crates/sim/src/game/services/commands.rs`
- possibly `server/crates/sim/src/game/services/mod.rs` if module declarations are needed

What stays stable:

- `pub(crate) fn apply_commands` and its call site;
- command acceptance semantics, receipt ordering, resource mutation order, replay determinism, and
  queued-order behavior;
- client-visible command budget constants and protocol command payloads.

Verification:

- focused Rust tests for command-service behavior in `rts-sim`;
- command replay and command-budget tests if they are split or moved;
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if new
  modules cross existing architecture boundaries.

Manual review focus:

- compare before/after tests to ensure assertions were moved, not rewritten;
- inspect `apply_commands` ordering for no semantic movement;
- review any helper names for command-domain clarity rather than generic validation buckets.

Expected player-facing impact:

- None if done as intended. Any gameplay change would be a bug and should stop the cleanup plan.

Why rank 2:

- Phase 1 ranked `server/crates/sim/src/game/services/commands.rs` third at score 73.56, with 5,208
  non-empty LOC, 7,831 recent churn, and 169 co-change partners.
- Phase 2 identified pure guard/adaptor seams and a narrow public entry point.
- This gives strong context reduction without touching protocol or balance mirrors, but it is ranked
  below contract tests because command ordering is gameplay-critical.

History caveat:

- Track command service as a group after the split: `commands.rs`, command guard helpers, planner
  adapters, and command-service tests should be analyzed together.

## 3. Broad Sim `Game` Tests

Proposed extraction:

- Rename or convert `server/crates/sim/src/game/tests.rs` into a test module root with domain files.
- Split by existing behavior clusters: ability/Ekat, artillery, fog/projection, mortar/smoke,
  movement replay/determinism, scoring/observer analysis, mining/resources, and tank traps.
- Move shared fixtures first, then move assertions without changing setup APIs.

Likely touched by a later cleanup plan:

- `server/crates/sim/src/game/tests.rs`
- new domain test modules under `server/crates/sim/src/game/tests/`
- only module declarations needed to preserve the existing `#[cfg(test)]` test module

What stays stable:

- `Game` public API tests remain tests of the public seam, not server room machinery;
- fog/projection assertions stay recipient-specific;
- replay determinism and command replay setup stay readable and auditable.

Verification:

- focused `rts-sim` tests for the moved test module;
- any named domain tests moved in the split;
- `git diff --check`.

Manual review focus:

- verify the split follows behavior families rather than arbitrary line chunks;
- verify shared fixtures are smaller than the original burden and do not become a new monolith;
- compare moved assertions around fog/projection and replay determinism carefully.

Expected player-facing impact:

- None. This is test organization only.

Why rank 3:

- Phase 1 ranked `server/crates/sim/src/game/tests.rs` fourth at score 63.98, with 5,124 non-empty
  LOC, 5,659 recent churn, and 177 recent co-change partners.
- Phase 2 found clear domain clusters and no active ownership conflict.
- It is slightly lower than command-service cleanup because the file is test-only, but it is a
  strong low-runtime-risk cleanup candidate.

History caveat:

- Future hotspot runs should group `server/crates/sim/src/game/tests/**` with the broad sim test
  group, or this cleanup will make churn appear to disappear.

## 4. Client HUD Helpers

Proposed extraction:

- Extract resource-row rendering, control-group tab rendering, and command-card DOM/button rendering
  behind small helpers.
- Consider a separate command intent dispatch helper only if it preserves command issuer, control
  policy, hotkey, and `ClientIntent` seams.
- Reuse the existing local pattern established by `hud_command_card.js` and
  `hud_selection_panel.js`.

Likely touched by a later cleanup plan:

- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/hud_selection_panel.js`
- new `client/src/hud_*.js` helpers
- relevant client contract tests, ideally after the client-contract split lands

What stays stable:

- `HUD` constructor, `update`, and `destroy` behavior;
- command IDs, descriptors, hotkey resolution, affordability checks, cooldown display, missing
  resource audio, command issuer calls, and `ClientIntent` usage;
- no balance/config number changes.

Verification:

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- targeted HUD contract file if Rank 1 has already split the suite

Manual review focus:

- browser-check command card actions, disabled affordability states, cooldowns, selected-unit panel,
  resource display, and control groups;
- inspect that helper dependencies are injected or local rather than cross-importing broad app state.

Expected player-facing impact:

- None if the split is mechanical. Visual or command-card regressions would be bugs.

Why rank 4:

- Phase 1 ranked `client/src/hud.js` fifteenth at score 47.40, with 921 non-empty LOC, 2,427 recent
  churn, and 108 co-change partners.
- It co-changed with `tests/client_contracts.mjs` 39 times in the recent window, and Phase 2 found
  concrete DOM/helper seams.
- It is less urgent than test and command-service splits, but it has good verification coverage and
  obvious local helper patterns.

History caveat:

- Group future HUD helpers under `client-hud` so `hud.js` shrinking is not misread as reduced HUD
  churn.

## 5. Client `GameState` Helper Split

Proposed extraction:

- Extract transient visual-effect buffers and query helpers first.
- Consider selection/control-group helpers only if command-budget admission remains shared and tests
  cover the same public methods.
- Keep prediction/optimistic overlay extraction for a later pass unless the first split proves the
  method safe.

Likely touched by a later cleanup plan:

- `client/src/state.js`
- new state helper modules near `client/src/state.js`
- client contract tests for GameState, selection budget, and intent-state boundaries

What stays stable:

- `GameState` public methods and data shape consumed by renderer, HUD, minimap, input, and match;
- snapshot application order and interpolation semantics;
- browser-local command, placement, and lab intent remaining outside `GameState`.

Verification:

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- targeted GameState contracts if Rank 1 has already split the suite

Manual review focus:

- exercise selection/control groups, death/resource deltas, projectile and smoke effects, and
  prediction overlays in a local match or replay;
- confirm state helpers do not import renderer, HUD, or input modules.

Expected player-facing impact:

- None if mechanical. Any visible interpolation, effect, or selection change is a regression.

Why rank 5:

- Phase 1 ranked `client/src/state.js` eighteenth at score 43.40, with 1,031 non-empty LOC, 1,423
  recent churn, and 147 co-change partners.
- Phase 2 found several helper-shaped clusters, but state ordering is more sensitive than HUD DOM
  rendering, so this should follow stronger client contract organization.

History caveat:

- Keep the `client model` group stable across `state.js`, `client_intent.js`, command budget,
  prediction, and new state helpers.

## 6. Client `Match` App-Shell Collaborators

Proposed extraction:

- Extract net-report/ping management, combat audio event handling, and settings action wiring into
  small collaborators.
- Keep `Match` as the composition shell with explicit injected dependencies.
- Do not move prediction, frame loop, room-time, lab/replay wiring, or teardown ordering in the
  first cleanup.

Likely touched by a later cleanup plan:

- `client/src/match.js`
- new match-local helper modules
- client contracts for frame recovery, match health, prediction toggles, observer analysis, and
  teardown

What stays stable:

- `Match` public class construction and destruction;
- frame ordering, prediction adapter lifecycle, room-time handling, lab/replay controls, and
  listener/GPU teardown;
- all transport calls continuing through injected `Net`/policy seams.

Verification:

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- browser smoke through the normal client smoke path when helper movement touches lifecycle code

Manual review focus:

- start and leave a live match, replay, and lab session;
- check pointer lock, settings controls, pause/give-up controls, combat audio, net report status,
  and rematch teardown.

Expected player-facing impact:

- None if mechanical. Any lifecycle, audio, or frame-loop behavior change is a regression.

Why rank 6:

- Phase 1 ranked `client/src/match.js` tenth at score 52.05, with 1,184 non-empty LOC, 2,426 recent
  churn, and 160 co-change partners.
- The recent window shows `client/src/match.js` co-changing with `tests/client_contracts.mjs` 54
  times, and Phase 2 found helper seams.
- It ranks below HUD/state because app-shell sequencing and teardown are easier to break.

History caveat:

- Track new helpers with the `client match shell` group.

## 7. AI Self-Play Test Split

Proposed extraction:

- Split harness/artifact helpers from profile matchup tests.
- Split resource regression, pending-build tracker, live-AI team behavior, and full real-AI tests
  into domain files.
- Keep default quick tests and `RTS_FULL_AI_TESTS=1` coverage gates explicit.

Likely touched by a later cleanup plan:

- `server/crates/ai/src/selfplay/tests.rs`
- new self-play test modules under the same crate/module ownership
- possibly shared test fixture modules if the existing harness is moved

What stays stable:

- replay artifact schema and writing behavior;
- environment gates and default test runtime;
- profile behavior, resource rules, and live-AI team semantics.

Verification:

- focused `rts-ai` self-play tests that are moved;
- optional full-AI gate only when the moved code touches long-test paths;
- replay artifact schema test after helper extraction.

Manual review focus:

- confirm default test runtime does not silently expand;
- confirm failure artifacts still point to useful replay data when a self-play assertion fails.

Expected player-facing impact:

- None. This is test organization only.

Why rank 7:

- Phase 1 ranked `server/crates/ai/src/selfplay/tests.rs` fifth at score 63.25, with 1,775 non-empty
  LOC and 12,195 total churn.
- Phase 1 also showed only 37.2% of lines were 14 days old or newer, making it less urgent than the
  highest-freshness files.
- The split is useful, but AI self-play gates and artifact behavior add review nuance.

History caveat:

- Keep all split files grouped under `ai` and preserve artifact-related history notes.

## 8. Protocol Mirror Cleanup

Proposed extraction:

- Treat protocol as a Rust plus JS plus parity group, not as independent file cleanup.
- Candidate substeps include extracting Rust compact codec/MessagePack writer implementation into a
  submodule, moving contract metadata tables into a focused module, and splitting client binary
  frame parsing from compact snapshot semantic decoding while preserving exported names.

Likely touched by a later cleanup plan:

- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/crates/sim/src/protocol.rs`
- `server/crates/contract/src/lib.rs`
- `client/src/protocol.js`
- `tests/protocol_parity.mjs`
- protocol sections of client contracts
- `docs/design/protocol.md` if any contract shape or exported surface changes

What stays stable:

- every tag, field, compact code, version, enum vocabulary, and optional slot behavior;
- `protocol_contract`, `encode_snapshot_frame`, snapshot serializers, `parseServerFrame`,
  `decodeServerMessage`, `msg`, and `cmd` exported behavior;
- Rust/JS mirrors changing together.

Verification:

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- Rust protocol tests in `rts-protocol`
- any arch or contract generation check the follow-up changes touch

Manual review focus:

- inspect wire compatibility and compact snapshot fixtures;
- verify no client or server imports bypass the stable exports.

Expected player-facing impact:

- None for pure module movement. Any wire or decode behavior change is a bug unless the follow-up
  explicitly becomes a protocol migration.

Why rank 8:

- The protocol group is a major hotspot: Phase 1 ranked `server/crates/protocol/src/lib.rs` sixth
  and `client/src/protocol.js` sixteenth, and the `protocol-and-contracts` group has only 5 files
  but 13,912 non-empty LOC, 624 touches, and 18,061 recent churn.
- Recent coupling is extreme: Rust protocol plus JS protocol co-changed 62 times, Rust protocol plus
  client contracts 49 times, and JS protocol plus client contracts 40 times.
- That centrality makes cleanup valuable, but it must be a design-first mirrored-contract plan.

No-go within this candidate:

- Do not split Rust and JS protocol mirrors independently.
- Do not alter compact snapshot version, field order, or semantic snapshot shape for size cleanup.

History caveat:

- Future hotspot analysis should use the `protocol mirror` group as the stable identity.

## 9. Balance And Config Mirror Cleanup

Proposed extraction:

- Prefer validation or generation of client-visible mirror data before large manual rearrangement.
- If splitting manually, group Rust constants by domain only behind stable exports, and split client
  catalogs by units, abilities, upgrades, resources, and factions behind stable re-exports.

Likely touched by a later cleanup plan:

- `server/crates/rules/src/balance.rs`
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/faction.rs`
- `server/src/config.rs`
- `server/crates/sim/src/config.rs`
- `client/src/config.js`
- wiki/faction parity scripts and config sections of client contracts
- `docs/design/balance.md` if mirror contracts or player-visible values change

What stays stable:

- Rust remains authoritative;
- client remains a UI/render/fog mirror only;
- exported constants, stat helpers, catalogs, and descriptors stay compatible;
- no scalar values change in a cleanup plan.

Verification:

- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-wiki.mjs`
- `node tests/client_contracts.mjs`
- focused Rust rules tests if Rust modules move

Manual review focus:

- compare before/after unit/building stats, costs, supply, ranges, abilities, upgrades, and faction
  catalogs;
- verify docs/wiki output is unchanged unless a later plan intentionally updates it.

Expected player-facing impact:

- None for pure organization. Numeric drift would be gameplay-facing and must stop the cleanup.

Why rank 9:

- Phase 1 ranked `client/src/config.js` twentieth and `server/crates/rules/src/balance.rs`
  twenty-fifth, with high touch counts and direct co-change between them 33 times in the recent
  window.
- Phase 2 found mirror seams, but balance cleanup has higher accidental-player-impact risk than HUD
  or test splits.

No-go within this candidate:

- Do not make the client mirror authoritative.
- Do not mix server-only balance internals with client presentation descriptors.
- Do not rename exported constants without checking Rust shims and JS imports.

History caveat:

- Track balance mirror churn as a group, not by the shrunken `config.js` or `balance.rs` path alone.

## 10. CSS Section Modularization

Proposed extraction:

- Defer broad CSS splitting until there is a clear static-serving load strategy and screenshot or
  browser-smoke verification path.
- If revisited, split only along existing screen/surface boundaries such as lobby/browser, game
  canvas chrome, HUD, lab, replay/analysis, settings, game over, and match history.

Likely touched by a later cleanup plan:

- `client/styles.css`
- `client/index.html` or other static entry points only if multiple CSS files must be loaded
- DOM producers that already use the relevant class/id contracts, but avoid selector renames

What stays stable:

- selectors, class names, responsive layout behavior, command-card/HUD sizing, replay overlays, lab
  window behavior, and no-build static serving.

Verification:

- browser smoke or screenshot checks across desktop and mobile-ish viewport sizes;
- `node tests/client_contracts.mjs` only as a weak DOM-producer backstop;
- `git diff --check`.

Manual review focus:

- inspect lobby, live match HUD, minimap, command card, lab panel, replay controls, settings, game
  over, and match history at multiple viewport sizes.

Expected player-facing impact:

- None if mechanical. Visual regressions are likely enough that this is not an early cleanup.

Why rank 10:

- Phase 1 ranked `client/styles.css` seventh at score 57.25, with 3,573 non-empty LOC and 3,694
  recent churn.
- Phase 2 found section boundaries, but CSS selectors are UI contracts and automated coverage is
  weak compared with test-only or JS-helper splits.

No-go within this candidate:

- Do not split CSS solely to lower line count.
- Do not do broad selector renames as cleanup.

History caveat:

- If split, future hotspot analysis must keep `client/styles*.css` or a styling group together.

## 11. Room Task Runtime

Proposed extraction:

- Defer implementation until active room-runtime cleanup ownership is clear or explicitly folded
  into a new plan.
- A later owner may consider pure helpers for lab operation conversion/state, branch staging message
  construction, drain warnings, replay-viewer room-time controls, or splitting in-file tests by room
  mode.

Likely touched by a later cleanup plan:

- `server/src/lobby/room_task.rs`
- room-runtime helper modules such as session policy, projection, participants, snapshot fanout, and
  tick control only under that plan owner's direction
- room-task tests and live Node lobby/server integration tests

What stays stable:

- `RoomTask::handle_event`, phase transitions, tick handlers, replay/live/lab mode ordering,
  `SessionPolicy`, `ProjectionPolicy`, `Participants`, and public `Game` API usage;
- fog-gated fanout and server-only match-history behavior.

Verification:

- targeted room/lobby Rust tests for any moved helper;
- live Node lobby/server integration suite if wire-visible room behavior changes;
- protocol parity/client contracts if message construction moves.

Manual review focus:

- verify live match start, spectator join, replay viewer flow, lab session flow, branch staging,
  pause/give-up, drain handling, and empty-room reset.

Expected player-facing impact:

- None for pure helper movement. Any room lifecycle, projection, or replay behavior change is a
  regression unless explicitly planned.

Why rank 11:

- Phase 1 ranked `server/src/lobby/room_task.rs` second at score 90.75, with 7,664 non-empty LOC,
  13,503 recent churn, and 181 co-change partners.
- Despite that score, Phase 2 marked the file read-only because active room-runtime cleanup already
  owns the surface.
- This is a high-value area, but it is not safe for this hotspot backlog to initiate without owner
  coordination.

No-go within this candidate:

- Do not mechanically move `handle_event`, tick handlers, or mode transitions for size reduction.
- Do not bypass session/projection/participant policies or the public `Game` API seam.

History caveat:

- Keep all future room-runtime helper files grouped together; otherwise a split will hide room churn.

## Explicit No-Go Or Design-First Decisions

- Do not split mirrored protocol or balance files one side at a time.
- Do not treat `server/crates/sim/src/game/setup.rs`, `server/crates/sim/src/game/mod.rs`, or
  `server/crates/sim/src/game/systems.rs` as raw line-count targets. Phase 1 shows they are current
  hubs, but Phase 2 maps them as `Game` API and orchestration surfaces that need their own design
  question before movement.
- Do not modularize CSS until the static load model and visual verification are part of the plan.
- Do not move room-task event dispatch or tick ordering while another room-runtime plan owns the
  area.
- Do not use test splitting as a reason to rewrite assertions, setup APIs, or runtime behavior.

## Recommended First Follow-Up Plan

Create a focused plan for `tests/client_contracts.mjs` decomposition first. It should:

- preserve `node tests/client_contracts.mjs` as the stable command;
- create shared contract-test helpers under `tests/client_contracts/`;
- move one or two low-risk sections first, then the remaining sections once the helper shape is
  proven;
- run `node tests/client_contracts.mjs`, `node scripts/check-client-architecture.mjs`, and protocol
  parity if protocol/config contracts move;
- update future hotspot group tracking so split contract files remain one logical group.

This is the best return with the least runtime risk. It also improves the review surface for later
HUD, state, match, protocol, and config work because their contract coverage becomes easier to load.

## What To Measure After Each Cleanup Lands

- current-file and architectural-group LOC, touches, churn, recent churn, and co-change degree;
- whether defect/fix-looking touches move into smaller files or simply follow the same logical
  group;
- whether focused verification commands remain quick and discoverable;
- whether a follow-up split increased or reduced the number of files an engineer must load for a
  typical change;
- whether path-level history became misleading and needs group-map support in the Phase 4 workflow.
