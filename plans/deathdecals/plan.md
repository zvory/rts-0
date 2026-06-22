# Death Decals Plan

## Purpose

Add permanent, client-local ground decals for visible unit deaths. Infantry deaths should stamp a
player-tinted paint/blood mark, and vehicle or support-weapon deaths should stamp a blackened,
player-tinted hull/scorch mark that roughly matches the dead unit's body footprint. Decals are
visual-only, best-effort, and must have negligible steady-state FPS cost even after hundreds or
thousands of deaths.

## Core Direction

Use SVG as the source authoring format, then rasterize and stamp decals into one permanent ground
decal texture at runtime. Do not keep one SVG, Pixi `Graphics`, sprite, or display object per
death. The renderer should draw the accumulated marks as a single world-space layer every frame.

The intended pipeline is:

```text
death event -> recover owner/facing from the previous entity snapshot
-> choose SVG mask variant with a deterministic seed
-> apply owner player color plus seeded rotation/scale/offset
-> stamp once into an append-only decal texture
-> render one Pixi sprite/layer for all accumulated decals
```

## Phase Summaries

### [Phase 1 - Client Decal Runtime](phase-1.md)

Build the client-only data and rendering pipeline with simple procedural placeholder decals. This
phase should consume existing `death` events, recover owner/facing from the previous entity cache
when available, dedupe deaths by entity id, and append only unpainted decals into one ground texture.
It is a success when a long match can accumulate many placeholder decals while the Pixi scene graph
still has one permanent decal display layer rather than one object per death.

### [Phase 2 - SVG Authored Decal Assets](phase-2.md)

Replace the placeholder marks with SVG-authored decal masks and a small runtime atlas/loader. The
SVG sources should be easy for LLMs to edit, with simple shapes, stable names, explicit view boxes,
and no external dependencies, while runtime still rasterizes once and stamps bitmap masks into the
permanent texture. This phase should implement player-tinted infantry marks and vehicle/support
scorch marks using deterministic variant, rotation, scale, flip, and opacity choices.

### [Phase 3 - Stress, Polish, And Docs](phase-3.md)

Harden the feature under high death counts and polish the visible behavior in real matches. This
phase should add targeted stress coverage proving old decals are not redrawn every frame, check
texture size/update behavior on the current 126x126 maps, and manually verify fog dimming,
readability, player tint, and teardown across rematches. It should also update the client UI design
doc/capsule if the new renderer module or asset pipeline changes the documented client surface.

## Overall Constraints

- Keep the feature client-only and visual-only. Do not add protocol fields, server simulation state,
  authoritative decal persistence, match-history persistence, or replay artifact fields unless a
  later explicit product decision changes the requirement.
- Treat existing `death` events as best-effort visual triggers. If a snapshot is skipped or a client
  reconnects mid-match, the client may miss older decals; that is acceptable for this plan.
- Preserve fog safety. The server already gates death events; the client should stamp only received
  events and must not infer hidden deaths from missing entities or resource deltas.
- Keep steady-state frame cost effectively constant. The renderer must not traverse or draw every
  historical decal each frame.
- Keep texture memory bounded. Use a downsampled decal texture, not a full-resolution 4032x4032
  world texture on the current 126x126 maps unless performance evidence justifies it.
- Keep the existing no-build-step browser client. SVG loading/rasterization must work with plain
  ES modules and the server's static client asset serving.
- Keep PixiJS as the display layer. SVG is for authoring and masks; Pixi draws the final accumulated
  ground texture.
- Keep player color readable. Infantry marks may be strongly player-colored; vehicle/support marks
  should still read as blackened scorch with player-tinted paint/scrape fragments.
- Keep death decals under fog, units, buildings, resources, selection, and feedback. The intended
  layer order is `terrain -> decals -> resources -> ... -> fog -> ...`.
- Recover owner and facing from the previous entity snapshot where possible. If an entity was never
  present in the local previous snapshot, fall back to neutral owner color and deterministic facing.
- Do not add permanent marks for buildings, resources, tank traps, or projectiles in this plan.
- Avoid broad client architecture churn. New client modules should respect current dependency
  boundaries and pass `node scripts/check-client-architecture.mjs`.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and what should be manually tested. Manual testing
  notes should cover core behavior, not an exhaustive test matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Unit Classification

Initial classification for this plan:

- Infantry/player-tinted paint marks: `worker`, `rifleman`, `machine_gunner`, `mortar_team`, `ekat`.
- Vehicle/support scorch marks: `scout_car`, `tank`, `command_car`, `anti_tank_gun`, `artillery`.
- No decal: buildings, `tank_trap`, steel/oil resources, smoke, ability objects, projectiles, and
  any unknown kind.

If playtesting shows support weapons should leave a different mark, adjust the classifier and SVG
assets in a follow-up with updated manual-test notes.

## Performance Contract

- Historical decals are pixels in one texture, not live objects.
- Per-frame work is one additional sprite/layer draw plus existing world transform.
- New-death work is batched: stamp all unpainted decals for a frame/snapshot, then update the decal
  texture once.
- Keep a small dedupe structure such as `paintedDeathIds`; do not keep full decal records after
  they have been stamped unless tests need a bounded diagnostic history.
- Use renderer diagnostics or test-only inspection to prove the decal layer object count does not
  grow with death count.
- Destroy the decal texture, atlas textures, canvases, and any async loader state in
  `Renderer.destroy()` / match teardown paths.

## Suggested Execution

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait
gate and confirm the phase head is reachable from `origin/main`.

```bash
scripts/phase-runner.sh --plan deathdecals 1 --pr --wait
scripts/phase-runner.sh --plan deathdecals 2 --pr --wait
scripts/phase-runner.sh --plan deathdecals 3 --pr --wait
```
