# Phase 1 - Close the Client Config CI Gap

Status: Incomplete.

## Objective

Ensure that changes anywhere in the split client rules/config mirror select the same cross-language
balance and faction checks as changes to the public `config.js` facade. Change selection and parity
coverage only; do not alter config values or runtime behavior.

## Work

- Treat `client/src/config.js` and every production module under `client/src/config/` as one client
  rules-mirror surface for suite selection.
- Ensure those paths select the Rust/client balance coverage, faction assumptions, faction-catalog
  parity, JavaScript protocol contracts, and the ordinary client suites already required for client
  source changes.
- Add representative selector verification cases for the facade and internal split files. Include a
  directory-level case that would fail if a newly added internal mirror module bypassed the parity
  suites.
- Adjust `scripts/check-faction-catalog-parity.mjs` only if its current discovery logic also misses
  internal mirror data needed to perform the selected check.
- Keep all current balance values, faction catalogs, protocol shapes, and client exports unchanged.

## Non-goals

- Do not change documentation automation, size baselines, or architecture ratchets; later phases own
  those jobs.
- Do not reorganize the client config directory or introduce generated config code.
- Do not add a new CI lane.

## Expected Touch Points

- `tests/select-suites.mjs`
- `scripts/check-faction-catalog-parity.mjs`, only if checker discovery needs correction
- focused selector verification data colocated with `tests/select-suites.mjs`
- testing documentation only if the documented selection policy changes

## Verification

- `node tests/select-suites.mjs --verify`
- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-faction-assumptions.mjs`
- Run selector output for `client/src/config.js` and at least two representative files under
  `client/src/config/`, then confirm the expected parity suites appear.
- `git diff --check`

## Manual Test Focus

No gameplay test is expected. Inspect one selector result for a newly named internal config path and
confirm it receives the same contract coverage as the facade.

## Handoff

Mark this phase done in its implementation commit. Report the final path rule, the parity suites it
selects, and the focused selector evidence. Tell the Phase 2 agent that documentation automation is
still unchanged and should start from current `origin/main`.
