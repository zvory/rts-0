# Phase 9 - Protocol Mirror Cleanup Gate

Status: done.

## Goal

Decide whether protocol cleanup is safe enough to become its own phase-runner plan. This phase is a
design gate: it should not move protocol code unless the user explicitly redirects it into an
implementation phase.

## Scope

- Read `docs/context/protocol.md` and the relevant sections of `docs/design/protocol.md`.
- Re-read the protocol sections of `plans/hotspots/responsibility-map.md` and
  `plans/hotspots/extraction-candidates.md`.
- Inspect the current boundaries among `server/crates/protocol/src/lib.rs`,
  `server/src/protocol.rs`, `server/crates/sim/src/protocol.rs`, `server/crates/contract/src/lib.rs`,
  `client/src/protocol.js`, `tests/protocol_parity.mjs`, and protocol client contracts.
- Identify only mirrored cleanup moves that can preserve every tag, field, compact code, version,
  enum vocabulary, optional slot behavior, and exported API.
- If a safe path exists, create a separate `plans/<one-word-name>/` protocol cleanup plan with phase
  files. Prefer a short name such as `protocolcleanup` if unused.
- If no safe path exists, mark this phase done with a clear no-go decision and evidence in this
  phase file or a small companion note.

## Touch Points

- `plans/hotspotcleanup/phase-9.md`
- possible new `plans/protocolcleanup/plan.md` and phase files
- no protocol source files unless the user explicitly changes this phase from gate to implementation

## Constraints

- Do not split Rust and JS protocol mirrors independently.
- Do not alter compact snapshot version, field order, semantic snapshot shape, command payloads,
  start payloads, replay payloads, event visibility, or fog gating.
- Do not change protocol exported names just to make modules prettier.
- Any future protocol cleanup plan must require parity tests and design-doc review.

## Verification

- `node tests/protocol_parity.mjs` if any generated evidence or contract fixture is touched
- `node tests/client_contracts.mjs` if protocol contract files are touched
- `node scripts/check-docs-health.mjs` if a new plan or docs links are added
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected because this is a design gate. Manually review that the proposed
plan, if created, treats Rust, server adapters, JS, parity tests, and docs as one mirrored surface.

## Handoff

After implementation, mark this phase done and summarize either the new protocol cleanup plan path or
the no-go reason, the exact protocol invariants that blocked movement, commands run, and what human
decision is needed before protocol source files move.

## Gate Decision

Go, with guardrails. The safe path is a separate phase-runner-ready plan at
`plans/protocolcleanup/plan.md`; this phase does not move protocol source files.

Evidence:

- `docs/design/protocol.md` already names the protocol crate as the owner of wire DTOs, compact
  code tables, slot schemas, codec metadata, and protocol versions, with `client/src/protocol.js` as
  the browser mirror.
- `docs/design/protocol.md` also has a boundary inventory for semantic DTOs, compact code tables,
  slot behavior, adapter kind conversion, default faction id, and lobby palette mirroring.
- `tests/protocol_parity.mjs` compares the Rust protocol contract dump against JS tags,
  vocabularies, compact codes, codec/version metadata, docs code tables, selected builders, and
  compact decode fixtures.
- Current source inspection found one large Rust protocol crate entry point, one large JS mirror,
  small Rust adapter files, a shared contract DTO crate, and already split protocol client
  contracts. That shape supports mechanical module extraction while keeping public imports stable.
- The follow-up plan requires Rust and JS movement in paired phases and blocks on any compact
  version bump, field rename, exported-name change, stale-client compatibility shim, or protocol
  migration.
