# Phase 7 - Final Drift Review And Archive Policy

## Phase Status

- [ ] Not implemented.

## Objective

Audit final faction guardrails and establish archive policy so drift does not return.

## Work

- Search for stale active references to `plans/faction`, contradictory boundary language, direct
  faction special-case growth, and unguarded client/server faction surfaces.
- Rewrite stale active references unless they intentionally point at historical archive evidence.
- Add archive-policy wording if needed so scripts do not depend on moved plan files again.
- Leave a concise guardrail map for future faction work.

## Expected Touch Points

- `plans/factionguardrails/*`
- Stale references in docs, scripts, tests, server, or client files
- `plans/README.md` only if a general archive policy is needed

## Implementation Checklist

- [ ] Run final drift searches.
- [ ] Remove or document stale active archive references.
- [ ] Add archive policy if needed.
- [ ] Produce the final guardrail map.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `rg -n "plans/faction|plans/archive/faction|reserved future.*ekat|single-faction assumptions" docs plans scripts tests server client`
- `node scripts/check-faction-assumptions.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

## Manual Test Focus

Review final docs and checker output. No gameplay test expected unless earlier phases touched runtime
behavior.

## Handoff Expectations

Provide a concise guardrail map naming the scripts, tests, and docs future agents should update
when changing faction behavior, and state any known drift intentionally deferred.
