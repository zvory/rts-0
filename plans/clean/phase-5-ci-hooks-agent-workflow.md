# Phase 5 - CI, Hooks, and Agent Workflow

## Objective

Make architecture checks part of normal development so agents see failures while they work, not
after a human architecture review.

## Work

- Add the Rust checker to the standard verification path:
  - local command documented in `AGENTS.md` or the relevant context capsule
  - commit hook if the hook already runs repo checks
  - CI job next to existing crate-boundary checks
- Document the workflow for failed checks:
  - prefer reducing coupling
  - if growth is intentional, update the baseline with a reason
  - avoid broad allowlist additions without a cleanup follow-up
- Add a short "when touching `rts-sim::game`" checklist:
  - can this be pure policy?
  - can this mutate through an existing entity/player helper?
  - did this add a new service-to-service edge?
  - did this increase a ratcheted file budget?
- Update `docs/context/server-sim.md` to point to the checker and this plan once Phase 1 exists.

## Verification

- CI fails on an intentionally introduced forbidden edge.
- CI passes on the unchanged baseline.
- The command output gives enough context for an agent to fix the issue without reading the
  checker source.

## Outcome

The architecture rules become part of the repo's ordinary feedback loop. Cleanup pressure is small,
continuous, and mechanical instead of dependent on periodic manual reviews.
