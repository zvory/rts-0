# Phase 5 - Lab UI and Product Hardening

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Add a minimal capability-gated LabPanel action for "Save replay so far." The UI should show pending,
success, error, and replay link states without adding a range picker or clip editor. Harden source
policy, branch controls, and product boundaries around local/dev save versus future sharing. File
write hardening, safe generated names, fixed output directories, and size/action caps should already
exist from Phase 4; this phase exposes those server capabilities and errors cleanly in the client.

## Expected Touch Points

- `client/src/lab_client.js`
- `client/src/lab_panel.js`
- `client/src/app.js`
- `server/src/lobby/launch.rs`
- `docs/design/protocol.md`

## Verification

- Run focused JS protocol/client tests selected by the changed files.
- Run one manual local lab save/open flow.

## Manual Testing Focus

Open a lab, make setup edits, issue commands, save replay, open the link, seek, and inspect at least
one fog perspective. Also verify the button is hidden or disabled when the server does not advertise
the capability.

## Handoff

The handoff must name any remaining product decision around production sharing or match-history
integration.
