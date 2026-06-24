# Phase 2 - Lab Command Ownership

Status: done.

## Goal

Make lab command affordances and command dispatch resolve against the selected issue-as owner rather
than the lab viewer's `state.playerId`. This phase should keep `issueCommandAs` as the privileged
operator-only command path.

## Scope

- Define a single client-side way to resolve the command owner for the current lab selection.
- Use that owner for command-card context:
  - resources and affordability
  - faction id and build/train catalogs
  - upgrades and tech requirements
  - prerequisite checks
  - ability cooldown/uses/autocast affordances
- Fix train, research, cancel, and producer selection helpers so they use the lab control policy
  instead of `e.owner === state.playerId`.
- Fix right-click and targeted-command owner classification so P2 attacking P1 is an attack, not a
  move, and P2-owned targets are not treated as enemies just because the lab viewer id is P1.
- Fix self-ability hover/range origins so selected non-local carriers produce previews.
- Keep mixed-owner selections blocked for gameplay commands, with clear operator feedback.
- Clarify in docs or code comments why `issueCommandAs` exists: lab starts are spectator-shaped and
  normal gameplay commands are authenticated by the sender's active player seat, so lab needs an
  explicit privileged issuer override.

## Expected Touch Points

- `client/src/lab_control_policy.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/input/commands.js`
- `client/src/minimap.js`
- `client/src/command_budget.js`
- targeted command-card/input tests where practical

## Constraints

- Do not allow arbitrary non-lab clients to spoof issuer identity.
- Do not remove `LabClient` request/response handling or convert lab issue-as into ordinary
  gameplay commands unless the server authentication model is redesigned in a separate plan.
- Do not let empty lab selection mean "all owners" for command dispatch. Empty selection may remain
  inspectable, but issuing gameplay commands needs exactly one selected owner.
- Keep client-side affordances advisory. The server remains authoritative and may still reject a lab
  command.

## Verification

- Run the client architecture check:

```bash
node scripts/check-client-architecture.mjs
```

- Run focused command-card/input contract tests if available. Add a small test for P2 lab
  right-click classification if the current test harness can cover it without a live server.

## Manual Testing Focus

In lab, select P2 units and right-click P1 units, P2 units, resources, and empty ground. Confirm the
issued command intent is attack, no-op/selection-safe, gather, or move as appropriate. Select P2
producers and verify train/research/cancel buttons reflect P2 resources, faction, upgrades, and
queues, then submit commands successfully through `issueCommandAs`.

## Player-Facing Outcome

Lab command buttons and right-click behavior match the selected side, so controlling P2 no longer
looks like partially controlling P1 through a spectator shell.

## Handoff

After implementation, summarize the command-owner resolution rule, any command surfaces still known
to be local-player based, and the P1/P2 lab command flows manually tested.
