# Short-Term Bug Fixes

A prioritized list of 20 minor bugs that are easy to fix — mostly things that were forgotten, left undone, or violate small invariants. Focus is on one-liners and simple deletions, not system changes.

---

## High Priority (violates hard invariants or leaks into production)

### 1. `server/src/game/services/commands.rs:183-184` — `order_attack` uses `.unwrap()` inside the tick loop
**Why it matters:** The `Game::tick()` path must be panic-free per `DESIGN.md §7`. Any `.unwrap()` on the tick path risks crashing the whole room task if a stale entity id slips through (e.g., a unit dies between command validation and movement system).  
**Fix:** Replace the two `.unwrap()` calls with `if let Some(t) = entities.get(target)` and `if let Some(e) = entities.get(id)`, returning early if either is `None`.

---

### 2. `server/src/game/services/commands.rs:229-230` — `order_gather` uses `.unwrap()` inside the tick loop
**Why it matters:** Same panic-free invariant as #1. A worker could die or a node could deplete between command receipt and application.  
**Fix:** Same pattern — `if let Some(n) = entities.get(node)` and `if let Some(e) = entities.get(id)`.

---

### 3. `server/src/game/services/commands.rs:341` — `order_build` uses `.unwrap()` on worker lookup
**Why it matters:** The worker entity could be killed by combat on the same tick the build command is applied.  
**Fix:** Replace `let e = entities.get(worker).unwrap()` with a fallible `if let Some(e) = entities.get(worker)`.

---

### 4. `server/src/game/services/movement.rs:40` — `movement_system` uses `.unwrap()` inside the tick loop
**Why it matters:** `entities.get(id)` can fail if an entity was removed by the death system in a previous phase, or if the id list has stale entries.  
**Fix:** `if let Some(e) = entities.get(id)` instead of `.unwrap()`.

---

### 5. `server/src/game/services/death.rs:46` — `death_system` cleanup loop uses `.unwrap()`
**Why it matters:** The `id` comes from `entities.ids()`, which is built at the top of `death_system`. Entities are removed inside the same function, so `entities.get(id)` could fail for an already-removed id in a later iteration.  
**Fix:** Use `if let Some(e) = entities.get(id)` in the cleanup loop.

---

### 6. `server/src/game/services/occupancy.rs:94` — `footprint_center` uses `.expect()` on the tick path
**Why it matters:** `footprint_center` is called from `order_build` (the tick path). `.expect()` panics if the building kind is invalid. While `order_build` validates the kind upstream, the extra panic here violates the no-panic-on-tick invariant.  
**Fix:** Return `Option<(f32, f32)>` instead of panicking, or use `if let Some(s) = config::building_stats(building)`.

---

### 7. `server/src/game/entity.rs:302` — `Entity::radius()` uses `.expect()` on the tick path
**Why it matters:** `radius()` is called from `combat_system` (the tick path). If a new entity kind is added and its stats are missing, this will panic mid-tick.  
**Fix:** Return a sensible default (e.g., `config::TILE_SIZE as f32 * 0.5`) instead of panicking, or return `Option<f32>`.

---

### 8. `client/src/protocol.js:40` — `PASSABLE` marks `FOREST (3)` as `false`
**Why it matters:** The server defines `FOREST (3)` as **passable for infantry** (`terrain::FOREST` is passable). The client table contradicts the wire protocol, which means the client-side build-placement validity check and fog pathfinding will treat forest as impassable. This desyncs the UI from the authoritative simulation.  
**Fix:** Change `3: false` to `3: true` in the `PASSABLE` table. If the table was only meant for building placement, rename it to `BUILDABLE`.

---

### 9. `client/src/lobby.js:302` — Leftover `console.error` in production callback handler
**Why it matters:** Debug logs leak into the browser console in production.  
**Fix:** Delete the `console.error("Lobby onGameStart callback failed:", err);` line or guard it behind a `__DEV__` flag.

---

### 10. `client/src/net.js:215` — Leftover `console.error` in production event dispatcher
**Why it matters:** Same as #9 — debug noise in production.  
**Fix:** Delete the `console.error(`Net handler for "${type}" threw:`, err);` line or guard it.

---

## Medium Priority (dead / unused code — clippy warnings or unreachable logic)

### 11. `server/src/config.rs:12` — `SNAPSHOT_EVERY_N_TICKS` is declared but never used
**Why it matters:** Dead code accumulates confusion. Clippy flags it on every build.  
**Fix:** Delete the constant. If it’s needed in the future, add it back with a real consumer.

---

### 12. `server/src/game/mod.rs:90,310,315` — `new_for_replay`, `tick_count`, and `command_log` are dead code
**Why it matters:** Only referenced by tests. Clippy warns. The replay module has its own `new_for_replay` path; these public methods are orphaned.  
**Fix:** Either `#[cfg(test)]` gate them or delete them if replay.rs covers the need.

---

### 13. `server/src/game/entity.rs:511-517` — `EntityStore::len()` and `EntityStore::is_empty()` are dead code
**Why it matters:** No callers outside tests. Clippy `dead_code` warning.  
**Fix:** Delete both methods.

---

### 14. `server/src/game/pathfinding.rs:21,93` — `MAX_EXPANDED` and `find_path` are unused
**Why it matters:** Only `find_path_with_budget` is ever called (from `pathing.rs`). The standalone `find_path` function and its `MAX_EXPANDED` constant are orphaned.  
**Fix:** Delete both. The budgeted version is the one true entry point.

---

### 15. `server/src/game/replay.rs:47` — `replay_commands` is never called
**Why it matters:** Clippy `dead_code` warning. The function is the public API of the replay module but no system currently replays logs.  
**Fix:** Keep it if a replay UI is planned, otherwise delete or `#[allow(dead_code)]` with a comment.

---

### 16. `server/src/game/replay.rs:108,113` — `ReplayError::OutOfOrder` and `CommandAfterEnd` are never constructed
**Why it matters:** Clippy warns that the enum variants are never built. The `replay_commands` function returns these errors, but nothing calls it.  
**Fix:** If `replay_commands` is kept, keep the variants. Otherwise delete the whole error enum.

---

### 17. `server/src/game/services/spatial.rs:38` — `SpatialIndex::cell()` is never used
**Why it matters:** Clippy `dead_code` warning. The method has no callers.  
**Fix:** Delete the method.

---

### 18. `server/src/game/services/spatial.rs:115` — `RectIter.min_ty` field is never read
**Why it matters:** Clippy warns. The field is stored but never referenced in `Iterator::next()`.  
**Fix:** Remove `min_ty` from the struct and its constructor.

---

### 19. `server/src/protocol.rs:40-55` — `UNITS`, `BUILDINGS`, `is_unit()`, `is_building()` are dead code
**Why it matters:** Clippy `dead_code` warning. These were likely intended for validation but are unused.  
**Fix:** Delete the constants and functions.

---

### 20. `client/src/hud.js:198` — Unreachable dead code in `_commandSubject`
**Why it matters:** The line `if (e.kind === KIND.WORKER && !worker) worker = e;` is unreachable because `isUnit(e.kind)` on the line above already returns `true` for workers, so the function returns early. The `worker` fallback is dead.  
**Fix:** Delete the `if (e.kind === KIND.WORKER && !worker) worker = e;` line.

---

## Summary Table

| # | File | Priority | Type | Effort |
|---|------|----------|------|--------|
| 1 | `server/src/game/services/commands.rs` | High | Panic risk | 1 line |
| 2 | `server/src/game/services/commands.rs` | High | Panic risk | 1 line |
| 3 | `server/src/game/services/commands.rs` | High | Panic risk | 1 line |
| 4 | `server/src/game/services/movement.rs` | High | Panic risk | 1 line |
| 5 | `server/src/game/services/death.rs` | High | Panic risk | 1 line |
| 6 | `server/src/game/services/occupancy.rs` | High | Panic risk | 1 line |
| 7 | `server/src/game/entity.rs` | High | Panic risk | 1 line |
| 8 | `client/src/protocol.js` | High | Protocol desync | 1 char |
| 9 | `client/src/lobby.js` | High | Debug leak | 1 line |
| 10 | `client/src/net.js` | High | Debug leak | 1 line |
| 11 | `server/src/config.rs` | Medium | Dead code | 1 line |
| 12 | `server/src/game/mod.rs` | Medium | Dead code | 3 lines |
| 13 | `server/src/game/entity.rs` | Medium | Dead code | 6 lines |
| 14 | `server/src/game/pathfinding.rs` | Medium | Dead code | 2 items |
| 15 | `server/src/game/replay.rs` | Medium | Dead code | 1 function |
| 16 | `server/src/game/replay.rs` | Medium | Dead code | 2 variants |
| 17 | `server/src/game/services/spatial.rs` | Medium | Dead code | 5 lines |
| 18 | `server/src/game/services/spatial.rs` | Medium | Dead code | 1 field |
| 19 | `server/src/protocol.rs` | Medium | Dead code | 15 lines |
| 20 | `client/src/hud.js` | Medium | Unreachable code | 1 line |

All 20 fixes are mechanical and can be done in a single pass without touching architecture.
