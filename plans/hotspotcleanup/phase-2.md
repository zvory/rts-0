# Phase 2 - Client Contract Domain Split

Status: planned.

## Goal

Finish splitting `tests/client_contracts.mjs` by contract area after Phase 1 proves the helper
pattern. The final top-level file should remain the stable runner but should no longer require a
reader to load every client contract at once.

## Scope

- Start from the Phase 1 layout on current `origin/main`.
- Move remaining major sections into domain files such as HUD, protocol, lobby, lab, state/input,
  renderer, audio, observer-analysis, config, and launch URL contracts.
- Keep shared helpers small and named by purpose.
- Preserve all assertion text and fixture behavior unless a tiny rename is needed to make module
  exports clear.
- Keep protocol/config/faction mirror checks adjacent to parity verification notes in the new module
  names or comments.
- Confirm `tests/client_contracts/` remains grouped under the `protocol-and-contracts` hotspot group.

## Touch Points

- `tests/client_contracts.mjs`
- `tests/client_contracts/*.mjs`
- `tests/select-suites.mjs` only if the suite selection command needs to know about new direct entry
  points
- `plans/hotspotcleanup/phase-2.md`

## Constraints

- Do not change assertion meaning, expected values, protocol field names, config values, or command
  descriptors.
- Do not split by arbitrary line ranges. Split by contract area so future owners can find tests.
- Do not make helper modules import production modules just to avoid explicit dependencies in domain
  tests.
- Preserve the exact top-level command: `node tests/client_contracts.mjs`.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs`
- `node scripts/check-faction-catalog-parity.mjs` if config/faction assertions move
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected. Manually review the domain file names, helper names, and top-level
runner output so future contract failures remain easy to triage.

## Handoff

After implementation, mark this phase done and summarize the final module map, any remaining large
sections, focused verification, and whether later HUD/state/match phases should run targeted domain
contract files or the full stable runner.
