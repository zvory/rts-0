# Phase 3 - Catalog Parity Hardening

## Phase Status

- [ ] Not implemented.

## Objective

Make catalog parity fail on partial or accidental faction exposure.

## Work

- Strengthen `scripts/check-faction-catalog-parity.mjs` assertions.
- Compare every Rust catalog dump intended for client exposure against `client/src/config.js`.
- Include ids, loadouts, units, buildings, buildables, trainables, research, abilities, costs,
  compact codes, and command-card metadata where available.
- Check full ability and command-card descriptor metadata for every client-exposed catalog, not
  just the default catalog: labels, icons, hotkeys, titles, carriers, target modes, ranges,
  cooldowns, charges, costs, queue/autocast flags, protocol codes, and order-stage codes.
- Check `loadoutId`, forbid extra client catalog ids, and dump/check builder, gatherer, and
  production-anchor fields when they affect UI or command legality.
- Preserve fixture handling as explicit test-only behavior.

## Expected Touch Points

- `scripts/check-faction-catalog-parity.mjs`
- `server/crates/rules/src/bin/dump-faction-catalog.rs`
- `server/crates/rules/src/faction.rs`
- `client/src/config.js`
- `tests/hud_command_card.mjs`

## Implementation Checklist

- [ ] Define expected catalog ids and fixture handling.
- [ ] Extend Rust dump or parity assertions as needed.
- [ ] Fail loudly on missing, extra, or mismatched client entries.
- [ ] Update command-card metadata checks where practical.
- [ ] Confirm fixture-only ids cannot appear as playable client catalog options.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-rules faction`
- `node tests/hud_command_card.mjs`

## Manual Test Focus

In a local lobby, verify visible faction options match intended playable ids only.

## Handoff Expectations

Name every catalog id the parity script expects and state whether any client mirror data remains
hand-maintained.
