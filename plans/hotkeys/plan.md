# Hotkeys and Unified Settings Plan

## Purpose

Replace hard-coded command-card hotkeys with a first-class, profile-driven hotkey system and move
current settings into a unified settings surface. The first version focuses on command-card
hotkeys, preset/custom profiles, import/export validation, and a modal settings UI available from
lobby, live match, and replay screens. See [requirements.md](requirements.md) for the product
requirements this plan implements.

## Phase Summaries

Phase 1 creates the unified settings shell and moves existing settings behavior into portable panel
sections. It keeps player-facing behavior mostly unchanged while making settings available from the
lobby, match, and replay surfaces. The result is a stable container for later hotkey editing work,
including context-only controls like give up and debug options.

Phase 2 introduces the hotkey domain model, schema, preset definitions, local storage, and
validation without yet replacing every command-card key path. It defines stable command identities,
normalizes keys, loads Grid and Classic RTS presets, and supports importing/exporting JSON profiles.
The result is a testable settings service that can answer what key a command should display or use.

Phase 3 wires command-card descriptors, HUD labels, and input activation to the hotkey service. It
removes hard-coded command-card hotkey decisions from the HUD/input path and makes the Grid preset
follow command-card slot locations automatically. The result is that selected profiles genuinely
drive live command-card behavior.

Phase 4 builds the hotkey editor UI on top of the unified settings shell. It lets players clone
presets, select or create custom profiles, inspect command-card contexts, rebind command identities,
validate conflicts, and import/export profiles. The result is the complete first-version product
surface for custom hotkeys.

## Overall Constraints

- Work one phase at a time. Each phase should be implemented, committed, merged to `main`, and
  pushed before the next phase begins.
- When a phase is complete, mark that phase document as done in the same implementation commit.
- After implementing each phase, the agent must provide a handoff message for the next agent.
- Each handoff must summarize what changed, list verification commands and results, identify the
  next phase or follow-up work, and name the core manual testing focus.
- Manual testing notes should cover core features, not an exhaustive matrix.
- Follow the client architecture invariant: non-shell cross-area imports should prefer dependency
  injection through `App`/`Match`.
- Any module that installs DOM/window listeners must implement `destroy()` and be torn down by its
  owner.
- Do not change the wire protocol for this effort.
- Do not add server/database persistence for hotkeys in the first version.
- Do not change command-card locations as part of hotkey editing.
- Keep command-card descriptor coverage current when changing command-card behavior.
- Run `node scripts/check-client-architecture.mjs` for client architecture changes.

## Phase Index

1. [Phase 1 - Unified Settings Shell](phase-1.md)
2. [Phase 2 - Hotkey Model, Schema, and Presets](phase-2.md)
3. [Phase 3 - Command Card Integration](phase-3.md)
4. [Phase 4 - Hotkey Editor, Import, and Export](phase-4.md)

## Open Product Notes

- Classic RTS key choices still need exact product approval before Phase 2 lands.
- The first version should not try to solve replay hotkeys, global hotkeys, modifier customization,
  or physical keyboard layout support.
- Conflict detection should be based on rendered command-card contexts generated from the same
  descriptor system that drives the real HUD.

