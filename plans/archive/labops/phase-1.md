# Phase 1 - Atomic Bulk Mutation And Placement Diagnostics

Status: done.

## Objective

Make the existing Lab mutation primitives genuinely bulk and authoritative so large scenes take one
operation per mutation family rather than one operation per entity. Rejected placements must return
enough structured evidence for a caller to correct the request without guessing coordinates.

## Contract

- CLI `spawn` keeps `{sessionId, spawns:[...]}` with 1-400 specifications and sends one
  `spawnEntities` operation. CLI `remove` keeps `{sessionId, refs:[...]}` with 1-400 references and
  sends one `deleteEntities` operation.
- CLI `update` adds `{sessionId, updates:[...]}` with 1-400 closed-union specifications. Continue
  accepting legacy `{sessionId, update:{...}}`, normalize it to one item, and send `applyUpdates`
  even for that singular compatibility form.
- Add the plural wire/replay operations `spawnEntities`, `applyUpdates`, and `deleteEntities` while
  retaining readers for existing singular replay operations. Successful plural outcomes use one
  ordered batch result containing `{index, outcome}` items, where each nested outcome is the
  existing singular outcome shape; failed results carry `failedIndex` and structured details.
- Apply non-move updates to scratch state in input order, then validate all move updates
  simultaneously against that resulting scratch state. Reject multiple updates targeting the same
  entity, and reject duplicate player/field updates, so move-plus-owner and similar order-dependent
  ambiguity cannot enter one request.
- Requests are atomic, results preserve input order, duplicate aliases/entity ids are rejected, and
  failure reports the input index. Spawn ids and aliases are committed only after the full request
  succeeds.
- For bulk movement, remove every moved entity from occupancy before validating destinations, then
  validate destination-to-destination collisions and all ordinary terrain/building/boundary rules.
- Apply the batch against scratch state and replace live state only after success. Repair and commit
  once, then let the CLI await one post-batch authoritative observation; do not assert that a live
  30 Hz room publishes exactly one snapshot.
- Extend Lab placement failures with the attempted position, blocker records, and at most eight
  deterministic nearby legal suggestions found within a bounded search radius/work budget.
- Compute suggestions against the transactional scratch prefix for the failed input. Earlier valid
  batch items and already reserved simultaneous destinations must therefore participate in the
  suggestion predicate, so retrying the corrected whole batch does not receive a self-conflicting
  alternative.
- Blockers distinguish entity id/kind, terrain tile/type, building/resource feature, and world
  boundary. Building alternatives use authoritative snapped centers.
- Preserve these error details through the Lab result, browser client, interaction bridge, driver,
  command service, daemon, and CLI JSON error envelope.
- Add plural Lab replay/timeline vocabulary and keep existing compatible artifacts readable.
- Keep IPC v1 compatible through the structured-error extension so a pre-change authenticated
  daemon can still answer `status` and `shutdown` during Phase 2 freshness recovery.

## Expected Touch Points

- `server/crates/sim/src/game/lab.rs` and standability/geometry helpers
- public `Game` Lab seam only if a new method is required
- protocol Lab operation/result/replay types and public-surface tests
- room-task Lab routing, timeline/replay application, scenario error conversion, and tests
- `client/src/protocol.js`, `lab_client.js`, and `interact_bridge.js`
- `scripts/interact/command_service.mjs`, driver, fake driver, and CLI/driver tests
- protocol, server-simulation, and Interact documentation

## Verification

- Rust tests for 400-item success, 401 rejection, all-or-nothing failure, duplicate references,
  simultaneous swap/translation, destination conflicts, one repair/snapshot, ordered results, and
  deterministic placement suggestions.
- Protocol parity/public-surface coverage for plural tags, fields, result shapes, and legacy replay
  compatibility.
- Room-task and Lab timeline tests proving one accepted operation, one authoritative commit, no
  per-item observation wait, and one coherent post-batch observed state.
- Client/bridge tests proving one request and structured error-detail preservation.
- CLI/driver contracts plus live smoke covering a large bulk spawn, bulk move, bulk remove, and an
  intentionally rejected placement corrected using a returned suggestion.
- Run the smallest selected Rust, client, and Interact suites, then the owned-PR workflow.

## Manual Testing Focus

Create opposing mixed-kind groups in one spawn request, translate them in one update request, and
confirm inspection observes the whole batch in one resulting authoritative state. Attempt one
blocked placement and confirm the returned blocker and first suggested point are understandable and
the suggestion succeeds without manual coordinate searching.

## Handoff

Report the final plural wire/replay shapes, atomicity and paused-time semantics, diagnostic schema,
compatibility behavior, exact tests, and any remaining performance risk. Tell Phase 2 which limits
are authoritative and which CLI/client summary caps still need to move to 400.
