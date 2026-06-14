# Selection Supply Budget Plan

## Purpose

Replace the strict selected-unit count cap with a command supply budget so high-power units consume
more of the player's command bandwidth than low-power infantry. This should make Tank-heavy armies
less effortless to move as one blob while preserving the existing simple selection ordering: drag,
double-click, shift-select, and control-group recall fill from their normal candidate order until
the budget is full. The same budget must be enforced on the client for usability and on the server
for hardening, because oversized command unit lists should be impossible for honest clients and
rejected when sent by malformed or hostile clients.

The initial base command budget is 24 supply. Each selected or commanded Command Car increases the
budget by 12, and multiple Command Cars stack. Units use their mirrored supply value as command
weight; selectable entities without supply, including buildings, workers if the mirror is missing,
and other non-combat selectable entities, count as 1.

## Phase Summaries

Phase 0 inventories the current selection, command validation, balance mirror, and HUD assumptions
before behavior changes start. It records every path that admits selected units or sends multi-unit
commands, including the current client `.slice(0, 12)` caps and the server `MAX_UNITS_PER_COMMAND`
hardening seam. The outcome is a narrow implementation map and a decision on whether this rollout
starts with manual mirrored constants or introduces generated client config.

Phase 1 defines the shared command-budget contract and enforces it on the server. It replaces the
raw count-only command cap with a dedupe-then-budget validation step that rejects oversized unit
commands, while preserving a defensive absolute id-list bound for tick-loop safety. The outcome is
authoritative hardening that treats legal human commands, including stacked Command Cars, the same
way the client will.

Phase 2 adds the client-side selection budget model and applies it to direct selection, shift
selection, drag-box selection, and double-click same-kind selection. It keeps the existing candidate
ordering, but pre-admits Command Cars from the candidate set so their budget bonus is not dependent
on box order. The outcome is playable client behavior where the old 12-unit limit is gone and
overflow candidates are ignored instead of replacing or trimming already-selected units.

Phase 3 applies the same client budget rules to control groups and command composition. It makes
control-group save, add, and recall unable to preserve or restore an over-budget human selection,
and it ensures every outgoing human multi-unit command is checked against the same budget before it
is sent. The outcome is that control groups and command hotkeys cannot bypass the selection supply
limit, while AI command generation remains unaffected.

Phase 4 replaces the multi-selected HUD summary with a two-row command-budget grid. It renders
selected entities as acronym blocks spanning their command weight, shows `used / cap`, expands when
Command Cars are selected, and flashes red overflow text when an attempted selection exceeds the
budget. The outcome is a visual explanation of why four Tanks fill the base grid while infantry can
fill many more cells.

Phase 5 removes obsolete 12-unit language, updates docs, and adds focused regression coverage. It
checks server rejection behavior, client selection admission, Command Car stacking, control groups,
and HUD grid rendering without running broad bundles during development. The outcome is a cleaned-up
rollout whose contract is documented and whose highest-risk seams are covered by targeted tests.

## Phase Index

1. [Phase 0 - Inventory and Contract Decision](phase-0.md)
2. [Phase 1 - Server Command Budget Validation](phase-1.md)
3. [Phase 2 - Client Selection Budget](phase-2.md)
4. [Phase 3 - Control Groups and Command Sending](phase-3.md)
5. [Phase 4 - Selection Budget Grid UI](phase-4.md)
6. [Phase 5 - Cleanup, Docs, and Regression Coverage](phase-5.md)

## Overall Constraints

- Replace the old strict selected-unit cap. Do not keep 12 units as a second gameplay limit.
- Keep selection ordering dumb and predictable. Do not optimize mixed selections to maximize power,
  value, or filled supply; preserve the existing candidate order except for Command Car pre-admit
  logic needed to make their bonus reliable.
- Command Cars always count as selected/commanded entities and consume their own supply weight, but
  each admitted Command Car also adds 12 to the command budget. Multiple Command Cars stack.
- For candidate selection passes, Command Cars present in the candidate set should be admitted even
  if they would not fit at that point in ordinary order, so their bonus does not depend on drag-box
  or same-kind ordering quirks.
- Server overflow should reject the command, not silently filter it. Honest clients should not send
  overflow commands; rejection is a hardening signal.
- Preserve a defensive absolute unit-id bound on the server so huge malformed payloads cannot force
  unbounded per-id work before budget validation finishes.
- Use authoritative/mirrored supply as command weight. Selectable entities without supply count as
  1, including buildings and non-combat selectable objects.
- AI is out of scope for the command-budget limit. AI may continue to produce command lists through
  its existing server-side action layer unless a later AI-specific balance plan changes that.
- Spectator and replay selection are out of scope except for not breaking existing inspection
  behavior. Old replay compatibility does not matter for this pre-alpha project.
- Keep the wire protocol mirrored if command rejection reasons or command payload contracts change:
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md` must stay aligned.
- Keep the balance mirror aligned if command weight constants or supply values move:
  `server/crates/rules/src/balance.rs`, `server/src/config.rs`, `client/src/config.js`, and
  `docs/design/balance.md` are the important surfaces.
- Prefer a small mirrored configuration seam first unless Phase 0 proves a generation path is
  already practical. Generated JS/JSON config is allowed only if it reduces total risk for this
  rollout rather than becoming a parallel infrastructure project.
- Balance/gameplay patch notes should call out that command bandwidth is now supply-based, Tanks
  consume more selection budget than infantry, and Command Cars increase the command budget by 12
  each.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
