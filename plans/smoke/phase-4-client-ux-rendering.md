# Phase 4: Client UX and Rendering

## Objective

Make Smoke playable and readable in the existing no-framework Pixi client: command-card affordance,
hotkey `D`, targeted cursor flow, range previews, command feedback, smoke rendering, and local fog
overlay parity.

## Command Card

- Add Smoke to selected scout cars on hotkey `D`.
- Show the button only when the selected group contains at least one apparent Smoke carrier.
- Disable the button when:
  - completed Steelworks is missing;
  - steel is below 25;
  - oil is below 25;
  - every selected carrier has Smoke cooldown remaining.
- Show cooldown clocks using the generic ability cooldown projection.
- Keep the command-card layout stable with existing `QWE/ASD/ZXC` hotkey conventions.

## Targeting Flow

- Clicking Smoke arms targeted ability mode.
- Left-click sends `cmd.useAbility("smoke", selectedScoutCarIds, x, y, queued)`.
- Shift-left-click sets `queued: true`.
- Right-click or Esc cancels.
- Minimap click behavior should either support Smoke targeting or explicitly cancel/ignore in a
  consistent way. Prefer supporting it if it can reuse existing target command routing safely.

## Range Preview

- While Smoke targeting is armed, draw dotted 9-tile circles around each selected eligible carrier.
- The preview is advisory only. The server still chooses the authoritative caster.
- If the hovered target is in range of at least one carrier, use the normal valid color.
- If not in range, use a secondary color to communicate that a scout car will move into range.

## Smoke Rendering

- Decode active smoke clouds from snapshots.
- Render smoke clouds as visible world effects, separate from entities.
- Clouds should be visible only when the server projects them.
- Early art can be simple: soft grey/white billows, dithered circles, or layered translucent
  particles. Projectile/canister visuals are deferred.
- Smoke should render below selection/feedback but above terrain/buildings enough to read as LOS
  blocking.

## Local Fog Overlay

- Update the client cosmetic fog model enough that smoke does not visually contradict server
  snapshots.
- Own/friendly units inside smoke should remain drawable if projected by the server.
- Enemy units omitted by the server must not remain clickable via stale client state.
- If precise smoke-aware client fog raycasts are too large for this phase, bias toward a conservative
  overlay that darkens smoke areas rather than pretending they are clear.

## Done

- A player can select scout cars, press `D`, see range circles, click a target, and get a smoke
  cloud.
- HUD affordability, tech, and cooldown states are understandable.
- Smoke clouds are rendered from server state.
- Client selection/targeting does not allow hidden smoke-covered enemies to be attacked.

## Verification

- `cd server && cargo test`
- Run a local server and manually test Smoke with scout cars and AT guns.
- Run client smoke tests if touched UI paths are covered by the headless client suite.
