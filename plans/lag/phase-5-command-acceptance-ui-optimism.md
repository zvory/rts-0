# Phase 5 - Command Acceptance, Rejection, and UI Optimism

## Objective

Expand prediction from movement-only visuals into command acceptance feedback while keeping
authoritative rejection clear and recoverable.

## Prediction Scope

Consider local optimism for:

- production queue button feedback
- rally point and rally-plan markers
- build placement command feedback
- setup/teardown command affordances
- ability targeting feedback for owned carriers

Treat each command family separately. Do not enable a family until it has command-specific
accept/reject tests.

## Command Result Model

- Acknowledgement only means the server processed the command.
- The client needs a way to distinguish:
  - processed and accepted
  - processed and no-op
  - processed and rejected with notice
- Prefer explicit server command-result metadata when useful, but do not spam the wire with verbose
  per-command data if existing owner-only snapshot fields can prove acceptance.
- UI optimism must be reversible:
  - optimistic queue item disappears if not confirmed
  - optimistic rally marker is replaced by authoritative `rallyPlan`
  - optimistic build ghost disappears or turns into authoritative scaffold only after confirmation

## Client Work

- Add per-command-family prediction policies.
- Add timeout rules for optimistic UI that remains unconfirmed after a bounded number of
  authoritative snapshots.
- Keep local command feedback visually distinct in developer mode so tests and debugging can see
  predicted vs authoritative affordances.

## Server Work

- Add compact owner-only confirmation data only where snapshots cannot already prove command
  acceptance.
- Keep rejection notices fog-safe and player-local.
- Ensure command-result metadata is tied to `clientSeq` when emitted.

## Verification

- Unit tests for every predicted command family:
  - accepted path
  - rejected by ownership
  - rejected by affordability
  - rejected by tech requirement
  - rejected by invalid target or placement
  - accepted after a coalesced snapshot skip
- Browser smoke tests for:
  - train click optimistic queue feedback then authoritative confirmation
  - invalid build command optimistic feedback removed after rejection
  - rally marker corrected to authoritative `rallyPlan`
- Protocol mirror tests for any new command-result metadata.
- Replay test proving command-result metadata does not alter deterministic simulation.

## Player-Facing Outcome

Command UI feels responsive immediately, while invalid commands still settle to the server's
authoritative result.
