# Phase 4 - Command Card And World Targeting UX

## Phase Status

Status: pending.

## Objective

Expose the new artillery fire modes to players in the world targeting UI. Point Fire and Blanket
Fire should be separate command-card buttons with separate command ids, target modes, hotkeys,
active targeting state, preview feedback, and command feedback.

## Scope

- Enable the Blanket Fire command-card entry for Artillery in the bottom-right slot. In the default
  grid hotkey layout, use `C` for Blanket Fire while preserving Point Fire as a separate targeted
  command.
- Add or update command-card descriptor logic so Point Fire and Blanket Fire show distinct labels,
  titles, icons, hotkeys, enabled states, cooldown/cost display, and active targeting state.
- Add JS command construction for `useAbility(blanketFire)` and route world clicks through the
  normal command issuer with `queued` preserved from Shift state.
- Add advisory client-side target locking that mirrors the server helper closely enough for
  previews and command feedback:
  - per selected artillery origin,
  - current setup or planned facing fallback,
  - 25-to-55 tile range band,
  - same-ray map clamp,
  - per-gun locked effective point.
- Update Point Fire and Blanket Fire hover previews so players can tell whether each selected gun
  will fire from its current cone or redeploy toward the locked point.
- Draw the current cone when the locked point is inside a deployed gun's current cone, and draw the
  future setup/redeploy cone when the locked point needs new facing.
- Draw Blanket Fire's 15-tile blanket radius around each gun's locked center.
- Add command feedback after issuing Point Fire or Blanket Fire that marks the effective locked
  point when the client can compute it. Blanket Fire feedback should include the blanket radius.
- Keep the client advisory only. Do not bypass server validation or assume the client lock is
  authoritative.
- Update `docs/design/client-ui.md` and, if needed, `docs/design/balance.md` for the new command-card
  and preview surfaces.

## Expected Touch Points

- `client/src/protocol.js`
- `client/src/protocol_constants.js` if phase 1 did not finish all constants
- `client/src/config.js`
- `client/src/config/rules_mirror.js`
- `client/src/config/factions.js`
- `client/src/hud_command_card.js`
- `client/src/hud_ability_affordance.js`
- `client/src/hud_unit_commands.js`
- `client/src/input/commands.js`
- `client/src/client_intent.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/feedback_view_model.js`
- `tests/hud_command_card.mjs`
- `tests/client_contracts/config_contracts.mjs`
- `tests/client_contracts/state_input_contracts.mjs`
- `tests/client_contracts/input_contracts.mjs`
- `tests/client_contracts/ability_hotkey_targeting_contracts.mjs`
- `tests/client_contracts/renderer_feedback_contracts.mjs`
- `docs/design/client-ui.md`

## Edge Cases To Cover

- Point Fire and Blanket Fire appear as separate Artillery buttons and cannot share active target
  state accidentally.
- Blanket Fire always requires a target click, including when every selected artillery piece is
  already deployed and idle.
- Hotkeys select the intended fire mode. Point Fire and Blanket Fire must not conflict in the same
  command-card context.
- Mixed selections expose only commands allowed by the selected artillery and do not issue
  `blanketFire` for non-artillery units.
- A close hover shows the locked 25-tile effective point, not an invalid dead-zone rejection.
- A far hover shows the locked 55-tile effective point and does not suggest artillery will walk.
- Deployed in-cone hovers show the current cone; out-of-cone hovers show redeploy-facing preview.
- Blanket Fire preview and command feedback include the 15-tile blanket radius centered on the
  locked point.
- Advisory previews degrade cleanly if selected units lack enough local data to compute a lock; the
  server command still sends the raw click for authoritative locking.
- Existing mortar, smoke, Ekat, support-weapon setup, and Point Fire feedback are not regressed.

## Verification

- Focused command-card, hotkey, input targeting, and renderer feedback contract tests.
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/protocol_parity.mjs` if protocol constants were touched in this phase.
- `git diff --check`

## Manual Test Focus

In a local match, select artillery and use both the command-card buttons and hotkeys for Point Fire
and Blanket Fire. Move the cursor inside minimum range, outside maximum range, inside the current
cone, and outside the current cone; confirm previews and command feedback show locked effective
points, redeploy intent, and Blanket Fire radius without implying automatic movement.

## Handoff Expectations

Document the client target-lock helper, where command-card/hotkey descriptors live, and any known
cases where advisory preview may still differ from server authority. Include player-facing patch
notes for the new Blanket Fire button and Point Fire targeting changes.
