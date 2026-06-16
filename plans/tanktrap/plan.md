# Tank Trap Plan

## Purpose

Add Tank Traps as engineer-built, vehicle-blocking field obstacles without turning them into
ordinary walls that block infantry. A Tank Trap is a 1x1 Czech hedgehog obstacle that costs 15
steel, has 200 HP, is armored, takes 10 seconds to build, requires a completed Training Centre, and
does not provide sight, fog reveal, supply, production, or elimination survival. The important
systems goal is a clean movement-blocker model: infantry can path and stand on Tank Trap tiles,
while vehicle-body units cannot pass unless the physical gap is wide enough for their existing
body/clearance rules.

## Product Requirements

- Engineers build Tank Traps after the player owns a completed Training Centre.
- Tank Traps cost 15 steel and 0 oil, have 200 HP, are armored, use a 1x1 footprint, and take
  `TICK_HZ * 10` build ticks.
- Tank Traps do not count as buildings for elimination. A player with only Tank Traps remaining is
  eliminated.
- Tank Traps provide no sight and no fog reveal. They still appear to enemies through the normal
  scouted/remembered building behavior once discovered.
- Tank Traps can be attacked and destroyed normally. They do not need special cancel, repair, or
  salvage behavior beyond whatever generic mechanics already exist.
- Tank Traps block every vehicle-body movement kind, including Tank, Scout Car, Command Car,
  Anti-Tank Gun, Mortar Team, and Artillery. Prefer a shared movement/body classification over a
  Tank Trap-specific list.
- Infantry can path through and stand on the same tile as a Tank Trap.
- Tank Traps block vehicles while under construction and after completion.
- Placement uses Bresenham-style tile lines between drag start and drag end, but emitted Tank Trap
  sites must never leave a diagonal vehicle gap. The drag start tile is always included; the end
  tile is included only when it lands on the line cadence.
- Line placement allows one empty tile between Tank Traps on the same row or column, and allows
  diagonally touching Tank Traps. Consecutive emitted sites from the drag algorithm must not be a
  knight's move apart (`abs(dx), abs(dy)` of `2,1` or `1,2`); when a shallow or steep Bresenham line
  would produce that spacing, the line helper should emit a diagonal-touching bridge site instead.
  Invalid trap positions are skipped while later valid positions remain eligible.
- Without Shift, a line command sends at most one build site per selected worker, assigning the
  first valid trap positions from drag start toward drag end.
- With Shift, additional sites beyond the selected worker count use the existing queued build worker
  distribution semantics instead of a new client-side scheduler.
- Resource charging keeps the current construction model: affordability is checked at command time
  for feedback and charged on worker arrival; later sites may fail if resources are unavailable.
- Tank Trap uses the next open worker build-card slot and a placeholder readable hedgehog visual
  unless a later art pass replaces it.

## Phase Summaries

Phase 0 locks the architecture contract before code changes. It inventories current construction,
pathing, occupancy, elimination, fog memory, client placement, and command-card seams specifically
for vehicle-only static blockers. The outcome is a short implementation note in the phase file that
names the shared movement-blocker abstractions, open edge cases, and any plan adjustments needed
before touching implementation files.

Phase 1 adds dormant Tank Trap identity and mirrored metadata without exposing it as buildable. It
adds the new entity kind, compact protocol code, Rust and client stat definitions, faction catalog
visibility needed for snapshots, and balance/protocol docs, while keeping worker build menus unable
to place it. The outcome is a round-tripping kind whose data can be rendered or tested, but no
player can yet construct it through normal UI.

Phase 2 introduces the shared static blocker classification used by movement and placement. It
splits static occupancy into terrain, all-ground building blockers, and vehicle-only blockers, and
routes unit standability/pathing through a movement/body class instead of a one-size-fits-all
building grid. The outcome is that existing buildings still block everyone, while Tank Trap
footprints can block vehicle-body units only, including while under construction.

Phase 3 wires Tank Trap into authoritative construction, economy, combat targeting, fog memory, and
elimination rules while it remains hidden from the worker build menu. It allows valid build commands
for Tank Trap after Training Centre, charges 15 steel on arrival, excludes Tank Traps from
elimination-survival counts, and confirms zero sight does not create fog reveal. The outcome is a
server-complete obstacle that can be spawned or commanded in focused tests without client UX
exposure.

Phase 4 adds client mirror support, rendering, and advisory placement validation for Tank Traps
without enabling the final worker button yet. It draws a compact Czech hedgehog placeholder,
handles construction progress and remembered/scouted display consistently with other buildings, and
uses the same blocker policy for placement previews so infantry overlap is not treated like vehicle
or structure overlap. The outcome is a client that can display and preview Tank Traps correctly once
the final placement command surface turns on.

Phase 5 implements the Tank Trap line-placement interaction and exposes the build-card button. It
adds a small line-placement collaborator that derives Bresenham trap sites with one-tile orthogonal
gaps or diagonal-touching bridges, skips invalid sites, previews the line, sends one immediate build
command per selected worker for the first sites, and sends queued standard build commands for
additional Shift sites using the existing server worker distribution. The outcome is the requested
player workflow without changing the wire command shape unless Phase 0 proves the standard `build`
command cannot support it safely.

Phase 6 consolidates tests, docs, manual scenarios, and cleanup. It adds focused regression coverage
for vehicle-only blocking, infantry pass-through, two-tile vehicle gaps, under-construction
blocking, line command distribution, elimination exclusion, and zero-sight behavior, then updates
design docs and context capsules. The outcome is a documented, shippable Tank Trap rollout with
known follow-ups limited to art, sound, AI usage, and future repair/cancel mechanics.

## Phase Index

1. [Phase 0 - Contract and Architecture Inventory](phase-0.md)
2. [Phase 1 - Dormant Kind and Mirrored Metadata](phase-1.md)
3. [Phase 2 - Movement-Class Static Blockers](phase-2.md)
4. [Phase 3 - Authoritative Construction and Gameplay Rules](phase-3.md)
5. [Phase 4 - Client Mirror, Rendering, and Preview Policy](phase-4.md)
6. [Phase 5 - Line Placement UX and Build Exposure](phase-5.md)
7. [Phase 6 - Regression Coverage, Docs, and Cleanup](phase-6.md)

## Overall Constraints

- Keep Tank Trap work requirements-gated. Do not implement Rust, JS, protocol, balance, art, tests,
  or generated data until the plan is approved and the relevant phase begins.
- Keep phases small. If a phase discovers a broader pathing, protocol, or placement redesign is
  needed, stop and update the plan rather than folding the redesign into that phase.
- Prefer shared systems over special casing. The vehicle-only blocking behavior should come from a
  static blocker/movement class contract, not from scattered `if kind == TankTrap` checks.
- Use the existing vehicle-body classification where it is already canonical. If it is not precise
  enough, introduce one narrow rules-level movement/body classification and map Tank, Scout Car,
  Command Car, Anti-Tank Gun, Mortar Team, and Artillery through it.
- Preserve panic-free tick behavior. Bad coordinates, stale worker ids, destroyed trap sites,
  unreachable staging tiles, malformed commands, and missing stats must become no-ops or notices.
- Preserve current construction economics. Resources are not globally reserved for an entire line;
  each site repeats final placement and affordability checks when a worker arrives.
- Preserve wire protocol mirrors. Adding the Tank Trap kind or any new command field must update
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs` if applicable,
  `client/src/protocol.js`, and `docs/design/protocol.md`.
- Preserve balance mirrors. Tank Trap cost, HP, sight, footprint, build time, armor class, and UI
  metadata must stay aligned across `server/crates/rules/src/`, compatibility config shims,
  `client/src/config.js`, and `docs/design/balance.md`.
- Do not make Tank Traps production anchors, rally targets, supply providers, or elimination
  buildings. They are attackable static obstacles only.
- Fog remains authoritative. Zero sight must not expand owner vision, and enemy visibility must use
  normal scouted/remembered building projection without leaking unseen positions.
- Client placement previews are advisory only. Server arrival-time validation remains authoritative
  for terrain, blockers, tech, ownership, and resources.
- Avoid broad test bundles during development. Each phase should run targeted Rust, Node, parity, or
  architecture checks matching its changed files; rely on the commit hook only when making
  merge-ready implementation commits.
- AI strategic use is out of scope. AI pathing must respect Tank Trap blockers because it uses the
  same movement systems, but AI does not need to build, prefer, or counter Tank Traps during this
  plan.
- Patch notes should say that engineers can build 15-steel Tank Traps after Training Centre, Tank
  Traps block vehicles but not infantry, and line-drag placement distributes construction across
  selected workers while avoiding diagonal vehicle gaps in dragged lines.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
