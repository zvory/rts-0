# Phase 1 - Make Client Guardrails Truthful

Status: Incomplete.

## Objective

Close the known CI-selection gaps in the split client config surface and perform the mechanical
source-size baseline cleanup already identified by the checker. Select cross-language suites only
for files that actually mirror server-owned data, extract the one mixed palette mirror so the
classification is truthful, and add production CSS to the existing size inventory without changing
runtime values, exports, styling, or CI topology.

## Work

- Extract `PLAYER_PALETTE` from the otherwise client-owned `client/src/config/presentation.js` into
  `client/src/config/player_palette_mirror.js`. Re-export it through the existing config surface so
  callers, values, rendering, and protocol parity remain unchanged.
- Replace the facade-only config path rules with an explicit classification of the resulting split:
  - `client/src/config.js` is the stable facade and selects all rules, faction, parity, protocol,
    and ordinary client coverage selected by its mirrored exports.
  - `client/src/config/rules_mirror.js` is the balance/rules mirror and selects Rust rules/sim,
    Rust/client config parity, JavaScript protocol contracts, and ordinary client coverage.
  - `client/src/config/factions.js` is the faction-catalog mirror and selects Rust rules/sim,
    faction assumptions, faction-catalog parity, JavaScript protocol contracts, and ordinary
    client coverage.
  - `client/src/config/timing.js` contains the server-timing mirror, including `TICK_HZ`, and
    selects the Rust/client config parity and supporting Rust rules/sim coverage in addition to
    ordinary client coverage.
  - `client/src/config/player_palette_mirror.js` is the server-assigned fallback-palette mirror and
    selects Rust plus protocol parity and ordinary client coverage.
  - After the extraction, `client/src/config/presentation.js` is wholly client-owned presentation
    policy. Keep it on ordinary client coverage; do not select Rust or faction suites solely because
    it shares the directory.
- Make selector verification fail when a production module under `client/src/config/` has no
  explicit mirror-or-client-owned classification. A future file must be classified deliberately;
  do not make the whole directory select faction suites by default.
- Prove through `--ci-policy` cases that the facade and each genuine mirror path above, including
  `player_palette_mirror.js`, produce `ci_class=full` and `run_rust=true`, while the now-pure
  `presentation.js` remains `client_only` with `run_rust=false` when changed alone. Suite-list
  assertions are not sufficient evidence.
- Keep the existing parity checker as the coverage owner. Change its imports/discovery only if a
  direct internal-module import is required to keep the checked surface truthful; do not duplicate
  its assertions in the selector.
- Extend the source-size inventory to the checked-in production stylesheet at
  `client/styles.css`. Exclude vendor, generated, and artifact CSS just as their source equivalents
  are excluded.
- Refresh `scripts/source-file-size-baseline.json` mechanically from current tracked files: remove
  exceptions now at or below the cap, lower frozen counts for files that have shrunk but remain
  above it, remove paths no longer tracked, and add the current oversized stylesheet with a concise
  responsibility-based reason. Do not split or otherwise edit an oversized source file in this
  phase.
- Preserve the ratchet's useful asymmetry: growth above a frozen exception fails, while beneficial
  shrinkage and obsolete exceptions remain advisory notes for the next mechanical refresh.
- Keep all balance values, faction catalogs, timing values, public config exports, CSS bytes, and
  runtime behavior unchanged.

## Non-goals

- Do not classify every config module as a faction surface or broaden Rust selection to unrelated
  client-owned presentation changes.
- Do not reorganize config modules beyond the exact palette-mirror extraction, generate config
  code, split oversized files, or reformat the stylesheet.
- Do not turn advisory shrinkage into a blocking baseline-update chore.
- Do not add a new CI lane, architecture ratchet, or source-size metric.

## Likely Touch Points

- `tests/select-suites.mjs`
- `client/src/config/player_palette_mirror.js` and the preserved re-export surface
- `scripts/check-source-file-sizes.mjs`
- `scripts/source-file-size-baseline.json`
- `scripts/check-faction-catalog-parity.mjs`, only if checker discovery must change
- focused selector or source-size verification colocated with the owning checks
- testing documentation only if the operator-visible selection or inventory policy changes

## Verification

- `node tests/select-suites.mjs --verify`
- Run `node tests/select-suites.mjs --ci-policy` separately for `client/src/config.js`,
  `client/src/config/rules_mirror.js`, `client/src/config/factions.js`,
  `client/src/config/timing.js`, `client/src/config/player_palette_mirror.js`, and
  `client/src/config/presentation.js`; assert the exact `ci_class` and `run_rust` outcomes above.
- Run selector output for the same six paths and assert their path-specific suites, including
  protocol parity for the palette mirror and the absence of Rust/faction suites for a
  presentation-only change.
- `node scripts/check-source-file-sizes.mjs`
- Exercise focused size-check fixtures proving that an unbaselined oversized production CSS file
  fails, growth above a frozen CSS count fails, and shrinkage below a frozen count succeeds with an
  advisory note.
- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-faction-assumptions.mjs`
- `git diff --check`

## Manual Test Focus

No gameplay or visual test is expected. Inspect the six config policy results once, confirm the
palette mirror pulls protocol parity while `presentation.js` does not pull Rust/faction work, and
confirm the source-size diff changes only the exact palette extraction, inventory policy, and
baseline data—not values, styling, or other source contents.

## Handoff

Mark this phase done in its implementation commit. Report the explicit config classification, exact
`--ci-policy` evidence, CSS inventory coverage, and baseline entries removed, lowered, or added.
Tell the next agent that config CI selection and source-size hygiene are complete and that
documentation automation remains unchanged.
