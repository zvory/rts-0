# Rust Crate Split - Multi-Phase Plan

This plan splits the current single Rust server package into smaller crates that enforce architecture
boundaries with Cargo instead of convention. The priority order is:

1. Enforce invariants such as "protocol code cannot import sim code" and "sim does not import AI".
2. Improve build and build-cache behavior, especially for AI/tool binaries.
3. Make test selection more defensible by tying suites to changed packages.

Today `server/` is one package with multiple binaries. `main.rs`, `ai_matchup.rs`,
`ai_balance_matrix.rs`, and `ai_perf_harness.rs` each declare overlapping module trees
(`config`, `game`, `perf`, `protocol`, `rules`). That preserves a simple source layout but means
architecture boundaries are mostly comments and extra binaries compile large parts of the sim as
their own binary-local modules.

The design docs already describe the main seam as `game::Game` versus the server shell. This plan
keeps that seam, then makes it real through package dependency direction.

## Target Dependency Direction

The final shape should point inward toward stable data and rules, and outward toward orchestration:

```text
rts-server
  -> rts-protocol
  -> rts-sim
  -> rts-ai
  -> rts-tools/dev support as needed

rts-tools
  -> rts-sim
  -> rts-ai
  -> rts-protocol

rts-ai
  -> rts-sim-api or rts-sim public query/command surface
  -> rts-rules
  -> rts-contract

rts-sim
  -> rts-rules
  -> rts-contract
  -> rts-protocol only temporarily, then not at all if contract/protocol split lands

rts-rules
  -> rts-contract or shared domain vocabulary

rts-protocol
  -> rts-contract

rts-contract
  -> serde only
```

The important negative rules are:

- Protocol/contract crates must not import sim, AI, lobby, tokio, axum, or server perf code.
- Rules/balance must not import sim services or lobby code.
- Sim must not import AI, lobby, axum, tokio room machinery, or server perf implementation.
- AI must emit ordinary commands and use public observation/query surfaces; it must not mutate sim
  internals directly.
- Server/lobby may compose everything, but should only touch simulation through the documented
  `Game` seam.

## Proposed Crates

Names can change during implementation, but keep the responsibilities stable:

- `rts-contract`: semantic DTOs shared across sim, protocol, replay, self-play, and server
  boundaries. Examples: `Snapshot`, `Event`, `StartPayload`, `PlayerScore`, semantic command DTOs.
- `rts-protocol`: WebSocket message envelopes, wire constants, compact snapshot serialization, JSON
  transport details. Depends on `rts-contract`, not on sim.
- `rts-rules`: `EntityKind`, unit/building/node definitions, terrain vocabulary, combat/economy
  formulas, balance constants and stats helpers.
- `rts-sim`: `Game`, entity store, map, fog, systems, services, deterministic replay core. Depends
  on rules and contract.
- `rts-ai`: live AI controller, AI core decisions/profiles, AI observations, profile-backed
  self-play support once it no longer needs sim internals.
- `rts-server`: axum/tokio shell, WebSocket handling, lobby, room task, static file serving, dev
  endpoints, server-side perf logging.
- `rts-tools` or individual tool packages: `ai-matchup`, `ai-balance-matrix`, `ai-perf-harness`.

## Known Couplings To Untangle

- `config.rs` imports sim `EntityKind` and `rules::defs`. Rules also import config stats, so
  balance/domain ownership is circular by concept.
- `EntityKind` conversion imports `protocol::kinds`, making sim/domain vocabulary depend on wire
  constants.
- `rules::projection` imports sim `Entity`, `EntityStore`, `Fog`, and protocol view DTOs. It is
  really a snapshot projection/fog policy layer, not a pure rules module.
- `Game` owns and ticks `AiController`, so sim currently imports AI.
- `Game::tick_with_perf` and `systems::run_tick` take `crate::perf` types, so sim imports server
  instrumentation.
- `CommandLogEntry` stores protocol `Command` for replay JSON compatibility, so deterministic
  replay and wire DTOs are coupled.
- Tests directly import many private modules through one crate. Splitting will require deliberate
  public test helpers or package-local tests.

## Phases

- [Phase 0 - Workspace and boundary audit](PHASE_0.md)
- [Phase 1 - Contract and protocol extraction](PHASE_1.md)
- [Phase 2 - Domain, rules, and balance extraction](PHASE_2.md)
- [Phase 3 - Simulation crate without server dependencies](PHASE_3.md)
- [Phase 4 - AI and self-play out of the sim core](PHASE_4.md)
- [Phase 5 - Server shell, tools, and perf composition](PHASE_5.md)
- [Phase 6 - Test selection, CI policy, and documentation lock-in](PHASE_6.md)

## Non-Negotiable Invariants

1. The game remains server-authoritative. Clients send intent only.
2. The wire protocol mirror remains exact. Any protocol change still updates Rust, JS, and
   `docs/design/protocol.md` together.
3. `Game::tick()` stays panic-free and deterministic.
4. Fog stays authoritative. Projection refactors must not leak hidden entities, target ids, hidden
   positions, rally data, order plans, debug paths, or events.
5. AI has no special authority. It emits ordinary commands that pass through the same validation,
   economy, supply, placement, and fog constraints as human commands.
6. The lobby/server shell touches simulation only through the documented `Game` API or a deliberate
   replacement API documented in the same change.
7. Each phase must leave `cargo test` for the affected packages green.
8. Avoid a big-bang move. Use compatibility shims and temporary re-exports when they reduce review
   risk, but remove them in the same or following phase once call sites are migrated.

## Build Strategy

Keep the workspace under `server/` initially. The repo also has a desktop Tauri crate, so do not
create a top-level Cargo workspace unless that is separately planned. Existing `.cargo/config.toml`
already points server builds at `/tmp/rts-cargo-target/rts-0-server`, which should continue to
benefit all server worktrees.

The first measurable build-cache win should come from replacing repeated binary-local module trees
with shared library crates. The three AI/tool binaries should link against already-built sim/AI
libraries instead of compiling another copy of the same modules for each binary target.

## Test-Selection Goal

The crate split should eventually allow policy like:

- Contract/protocol-only changes: protocol Rust tests, JS protocol mirror tests, compact transport
  tests, and targeted client decode tests.
- Rules/balance changes: rules tests, sim tests that consume stats/formulas, balance docs/mirror
  checks, and relevant gameplay regression tests.
- Sim changes: `rts-sim` tests, deterministic replay tests, relevant Node integration suites.
- AI-only changes: AI/profile tests and full AI/self-play gates.
- Server-shell changes: lobby/room task tests and live server integration tests.

This is a confidence aid, not permission to skip end-to-end coverage when a change crosses a
contract boundary.

