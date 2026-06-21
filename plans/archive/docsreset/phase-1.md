# Phase 1 - Obsolete Names and Generated Stats

Status: done.

## Goal

Remove or correct obvious stale player-facing names in active documentation and generated wiki
stats, with Steelworks versus Gun Works as the first confirmed target.

## Scope

- Search active docs, wiki generation, tests for wiki/stat labels, and current client/rules label
  sources for retired terminology.
- Fix `/wiki/stats` output if generated labels still derive stale player-facing names from internal
  ids.
- Fix active Markdown where it incorrectly tells a player or future agent that Steelworks exists as
  a player-facing building.
- Keep explicit internal ids such as `steelworks` when the text is clearly about protocol, compact
  kind codes, compatibility, or historical records.
- Ignore raw replay JSON, incident logs, and archived plans unless they are being presented as
  active guidance.

## Suggested Evidence

- `client/src/config.js` for current labels.
- `server/crates/rules/src/defs.rs` and `server/crates/rules/src/faction.rs` for current kind ids,
  train/build/research catalogs, and rule metadata.
- `server/src/wiki.rs` for generated stats labels.
- `docs/design/balance.md`, `docs/design/protocol.md`, and `docs/context/balance.md` for active
  references.

Useful searches:

```bash
rg -n "Steelworks|steelworks|Steel Works|Gun Works|requires .*Steel" docs client server tests -S
rg -n "kind_label|stable_id|label" server/src/wiki.rs client/src/config.js server/crates/rules/src
```

## Verification

Run focused checks that match the final diff. Likely commands:

```bash
node scripts/check-wiki.mjs
node scripts/check-faction-catalog-parity.mjs
node scripts/check-docs-health.mjs
git diff --check
```

If no content drift remains after the audit, run `node scripts/check-docs-health.mjs` and
`git diff --check`.

## Manual Testing Focus

After merge, open `/wiki/stats` and confirm player-facing building names and prerequisite cells use
current names while internal ids remain explicit only where they are intentionally documented.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must list stale terms found, terms
intentionally preserved as internal ids, verification run, and any remaining ambiguous references.
