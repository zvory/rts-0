# Rifleman directional art prototype

Opt-in Lab profile: `rifleman-directional-prototype`. Default matches retain their existing art.
Use normal server move orders: the sprite translates without a walk cycle and selects one of
sixteen headings. The camera stays oblique; the sprite is never spun as a flat overhead card.

`source-atlas.png` was generated with the built-in `image_gen` tool on 2026-11-27 from the
approved two-view white Rifleman concept. The exact request is in `generation-prompt.txt`.
The generator provided a transparent 4×4 atlas; angles and anatomy remain approximate concept art.

Run `node scripts/build-rifleman-directional-prototype.mjs` from the repository root to rebuild
`directional-strip.png` and `registration.json`. It keeps each cell's largest connected alpha
component, removes stray alpha specks, and registers the lower boot silhouettes to one shared
ground point. This excludes the projecting rifle from centering. It preserves alpha, uses a
uniform one-third-size area sample, and does not synthesize new art. Runtime anchors that common
point at the authoritative unit position; no visual rotation or locomotion bob is applied.

Run `node scripts/capture-rifleman-directional-prototype.mjs` in a clean worktree for a real
in-game WebM of one circuit of normal queued move orders. The script owns a temporary Interact
Lab session and closes it after capture, refusing to reuse an active session. It never composites
sprites over a screenshot.
