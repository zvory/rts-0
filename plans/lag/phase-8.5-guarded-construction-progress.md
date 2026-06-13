# Phase 8.5 - Guarded Construction Progress Extrapolation

Status: Considered; not implemented.

## Objective

Evaluate whether already-started building construction progress can be locally extrapolated safely.
This phase is separate from Phase 8 because construction progress depends on worker state and can
pause or fail for more reasons than production/research timers.

## Consideration Result

Do not implement construction-progress extrapolation with the current snapshot contract.

The client has enough mirrored data to draw authoritative construction progress:

- `buildProgress` is already projected for visible scaffolds.
- building `buildTicks` are mirrored in the client rules table.
- completed-building behavior already keys off the absence of `buildProgress`.

That is not enough to prove that construction is actively progressing between snapshots. Server
construction advances only while at least one worker remains in `BuildPhase::Constructing` for a
living, under-construction site. The current owner-visible snapshot can show a scaffold and its
last `buildProgress`, but it does not expose an explicit owner-only "active builder is currently
advancing this site" signal. Inferring activity from recent progress deltas would necessarily keep
extrapolating for at least one snapshot gap after worker death, pull-away, blocking, cancellation,
or site destruction until the next authoritative correction arrives.

That correction risk is small visually but broad semantically: construction completion gates supply,
tech prerequisites, command availability, production, pathing, and selection behavior. Even with a
display-only clamp below completion, a moving scaffold bar during a real pause is misleading in
the exact cases where construction feedback matters most. Leave construction progress
authoritative-only unless a later phase adds an owner-only active-construction signal and scenario
coverage proves that interrupted construction stops immediately.

This consideration made no implementation changes.

## Prediction Scope

Consider local construction progress only when all of these are true:

- the scaffold already exists in an authoritative snapshot
- the scaffold is owned and visible to the player
- `buildProgress` is finite and in `[0, 1)`
- the building kind is known and has a mirrored construction duration/rate
- recent authoritative snapshots show progress advancing, not stalled
- the local extrapolated value remains below completion

The client may render a locally advanced construction bar for an existing scaffold. It must not
create a scaffold, reserve tiles, spend resources, complete the building, unlock tech, increase
supply, enable commands, or change pathing/selection behavior before the server confirms.

## Stop Conditions

Stop extrapolating immediately when any authoritative snapshot shows:

- no scaffold for the entity id
- owner change or building death
- `buildProgress` missing, complete, lower than expected, or unchanged across the configured
  "recent progress" window
- worker interruption, cancellation, blocked construction, or any available owner-only signal that
  construction is paused
- fog/visibility loss that makes the scaffold no longer owner-visible

If the client cannot distinguish "still building" from "paused" with existing owner-visible
snapshot data, keep this phase disabled-by-default and document the missing signal instead of
guessing.

## Safety Model

Construction is less safe than production/research because server progress can pause when workers
die, move, are blocked, switch orders, or cannot reach the scaffold. The local display must
therefore be conservative and easily reversible:

- extrapolate only after server progress has already started
- cap below completion, for example at `0.98`
- prefer stopping too early over showing progress during a real pause
- treat all completion-side effects as authoritative-only

Steel and oil income remain out of scope.

## Client Work

- Reuse the Phase 8 display-progress seam if it exists.
- Add a construction-specific predictor that tracks authoritative `buildProgress`, receive time,
  building kind, entity id, and recent progress trend.
- Ensure HUD, renderer, command-card availability, fog, pathing, and selection logic continue to
  use authoritative building completion state.
- Add developer diagnostics that identify extrapolated construction bars separately from
  production/research bars.

## Server Work

Avoid server changes unless existing snapshots cannot expose a safe owner-only "construction is
actively progressing" signal. If a signal is needed, keep it owner-only, compact, and tied to
already-visible scaffolds; update protocol mirrors and docs in the same phase.

Current assessment: such a signal is needed before this feature should ship. A future implementation
should add the smallest owner-only metadata that answers "did the authoritative simulation advance
this scaffold this tick or snapshot window?" without exposing hidden workers, enemy denial, or
off-screen positions. Any added field must update `server/crates/protocol/src/lib.rs`,
`server/src/protocol.rs`, `client/src/protocol.js`, and `docs/design/protocol.md` together.

## Verification

- Unit tests for construction display extrapolation:
  - advances an already-started scaffold after authoritative progress
  - clamps below completion
  - stops when progress stalls
  - stops when the scaffold disappears or dies
  - stops on cancellation
  - does not mark the building completed locally
  - does not unlock commands, supply, tech, or production locally
- Tri-state scenarios:
  - normal already-started construction under delayed snapshots
  - worker killed or pulled away mid-construction
  - construction cancelled
  - construction completes on server while local display is clamped below completion
  - enemy denial/damage during construction
- Browser smoke under artificial latency for one safe construction path and one interrupted path.

## Manual Testing Focus

Under artificial latency, watch an already-started scaffold while workers continue building, then
interrupt the workers or cancel the building. Confirm the progress bar may move during short gaps
but stops or corrects promptly, never finishes early, and never enables completed-building
functionality before the server snapshot.

## Handoff Expectations

At handoff, state whether construction progress extrapolation shipped, remained behind a flag, or
was rejected as too state-dependent. For this consideration pass, it is rejected for now because
current snapshots do not expose a direct active-construction signal. List the exact stop
conditions, correction behavior, and any owner-only metadata added if a future implementation
reopens the feature.

## Player-Facing Outcome

Construction remains authoritative-only. Already-started buildings may still look frozen during
brief jitter, but the client will not show a moving construction bar when the server may have
paused the build because workers were interrupted, killed, blocked, pulled away, or the site was
cancelled/destroyed.
