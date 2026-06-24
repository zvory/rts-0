# Phase 5 - Projection Contract Hardening

Status: done.

## Goal

Document and test the projection contract so future lab/replay/spectator changes do not regress
owner control, event delivery, memory projection, or global event semantics.

## Scope

- Update protocol/client/server design docs for:
  - lab `issueCommandAs` and selected-owner control policy
  - full-world snapshot/event projection
  - spectator/replay/lab team event projection
  - remembered-building memory under player, team, and all-player vision
  - player-resource visibility under full-world and team/union vision
  - globally visible artillery firing markers
- Refresh relevant context capsules if section lists or invariants shift.
- Add targeted regression tests for:
  - lab P2 feedback/command affordance behavior
  - P2 right-click attack classification
  - full-world event delivery
  - spectator private notice filtering
  - replay vision memory reset/switching
  - lab team resource scoping
  - artillery firing remains global
- Add lightweight client contract coverage for the control-owner helper if Phase 1/2 did not add it
  already.
- Consider a projection audit checklist for future event types so new transient events name their
  intended projection policy.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`
- `docs/context/protocol.md`
- `docs/context/client-ui.md`
- `docs/context/server-sim.md`
- focused Rust and JS tests
- optional small checklist under `docs/` or `plans/` if useful

## Constraints

- Do not bundle new behavior into this phase unless it is required to make a documented test pass.
- Keep documentation factual. If a behavior is an intentional gameplay rule, state it as such; if
  it is a projection implementation rule, name the owning code seam.
- Do not run broad local suites by default. Use focused tests and rely on the PR gate for full
  coverage.

## Verification

- Run focused docs-related and client/server checks selected by changed files.
- At minimum, run protocol parity if protocol vocabulary or compact fields changed:

```bash
node tests/protocol_parity.mjs
```

- Run focused client architecture checks if client module contracts changed:

```bash
node scripts/check-client-architecture.mjs
```

## Manual Testing Focus

Run one lab scenario controlling P1 and P2, one replay vision switch, one live spectator attach, and
one dev full-world scenario. Confirm the documented projection rules match observed events,
overlays, memories, resources, and artillery markers.

## Player-Facing Outcome

The projection fixes become durable: future changes have explicit docs and regression checks for
the lab, replay, dev, and spectator behaviors players now rely on.

## Handoff

After implementation, summarize updated docs, new regression coverage, any intentionally uncovered
manual-only cases, and whether the projection audit checklist should be used for new event work.
