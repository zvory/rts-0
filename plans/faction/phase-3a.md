# Phase 3A - Canonical Faction Validation and Lifecycle Matrix

Status: Designed, not implemented.

## Objective

Fix the canonical current-faction identity before durable lifecycle, replay, command, hotkey, or
prediction contracts depend on the temporary Phase 1/2 id. The existing faction is **Kriegsia**
with canonical id `kriegsia`; the first real future faction is **Ekaterina** with reserved id
`ekaterina`.

## Scope

- Rename the temporary current-faction id `steel_vanguard` to `kriegsia` across protocol mirrors,
  Rust catalog data, client catalog parity data, docs, and tests.
- Reserve `ekaterina` in planning docs only. Do not add Ekaterina gameplay, catalog entries,
  loadouts, command cards, or UI in this phase.
- Keep `phase2_empty_fixture` as an architecture fixture. It is allowed only in Rust tests and
  explicitly documented dev/test harnesses; it is not a product faction and must not be selectable
  through normal lobby UI.
- Add a server-side faction validation helper near lobby/start assembly, with pure catalog
  existence still owned by the Rust rules catalog.
- Update `plans/faction/lifecycle-matrix.md` so every lifecycle row says where faction truth comes
  from, which factions are allowed, and whether unsupported ids reject or stay deferred.
- Do not expose normal faction selection in the lobby.

## Validation Contract

The helper should separate catalog existence from server policy. A concrete implementation may
adjust names to fit local style, but the contract should be equivalent to:

```rust
enum FactionRequestContext {
    NormalLobby,
    Quickstart,
    AiSeat,
    ReplayPlayback,
    ReplayBranch,
    DevScenario,
    SelfPlay,
    TestFixture,
}

enum FactionValidation {
    Defaulted { faction_id: String },
    AcceptedPlayable { faction_id: String },
    AcceptedFixture { faction_id: String },
    Rejected { requested: Option<String>, reason: FactionRejectReason },
}
```

Policy belongs in the server/lobby layer: lobby-selectable, AI-supported, prediction-supported,
dev/test-only, and replay-allowed. Catalog data belongs in `rts-rules`.

## Expected Touch Points

- `plans/faction/`
- `server/crates/contract/src/lib.rs`
- `server/crates/rules/src/faction.rs`
- Protocol mirrors and parity tests
- `server/src/lobby/` for validation policy
- Focused tests that currently assert the temporary id

## Verification

- Focused Rust/Node tests proving `kriegsia` is the default faction id everywhere Phase 1/2
  previously exposed `steel_vanguard`.
- Catalog parity still passes after the id rename.
- Lifecycle matrix updated with explicit Phase 3A ownership.
- No normal lobby selection for `ekaterina` or `phase2_empty_fixture`.

## Manual Testing Focus

Start a normal match and confirm gameplay remains unchanged. This phase changes identity strings,
not faction behavior.

## Handoff Expectations

The handoff must name the validation helper, confirm all temporary `steel_vanguard` runtime ids
were removed or intentionally limited to migration notes, and state that `ekaterina` is reserved
for later phases only.
