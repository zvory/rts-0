# Phase 2 - Normalized Rig Schema and Guardrails

## Phase Status

Status: Done.

- [x] Done.

## Objective

Add the data contract for unit rigs without changing live rendering.

## Work

- Add pure schema and validation modules for normalized unit rigs.
- Define stable data structures for:
  rig id, unit kind, parts, draw order, local transforms, pivots, anchors, tint slots, semantic
  bounds, animation bindings, and required runtime inputs.
- Keep schema code independent from Pixi. It should accept plain objects and return normalized
  plain objects or structured validation errors.
- Add tests for valid rigs, missing required anchors, duplicate part ids, unsupported transforms,
  non-finite geometry, invalid tint slots, invalid animation references, and unit-kind mismatch.
- Update client architecture rules if needed so rig schema modules live in a clear renderer or
  art-pipeline sub-area without becoming a cross-area dependency.
- Document the target API in `docs/design/client-ui.md`.

## Expected Touch Points

- `client/src/renderer/rigs/` or another Phase-0-approved rig module directory.
- `tests/` focused rig schema tests.
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `plans/svg/phase-2.md`

## Implementation Checklist

- [x] Add pure rig schema validator.
- [x] Add normalized rig type comments or equivalent JS doc.
- [x] Add structured error reporting.
- [x] Add focused schema tests.
- [x] Update architecture checks or docs for the new boundary.
- [x] Run verification and record exact results.

## Verification

- Focused rig schema test.
- `node scripts/check-client-architecture.mjs`
- `node --check` for new JS files.
- `git diff --check`.

Results:

- `node tests/rig_schema.mjs` - passed.
- `node scripts/check-client-architecture.mjs` - passed.
- `node --check client/src/renderer/rigs/schema.js && node --check tests/rig_schema.mjs` -
  passed.
- `git diff --check` - passed.

## Manual Test Focus

None required. This phase should not alter live rendering.

## Handoff Expectations

List the finalized normalized rig fields, rejected SVG/metadata cases represented in schema tests,
and any intentionally deferred schema features such as texture atlas parts or advanced curves.
