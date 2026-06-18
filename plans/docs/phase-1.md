# Phase 1 - Advisory Routing Map

Status: done.

## Goal

Create the first version of a source-to-doc routing map. The map should help agents and future tools
answer "which docs are likely relevant to this changed source area?" without creating a blocking
ownership or waiver system.

## Scope

- Add a parseable routing map under `docs/`, preferably `docs/doc-map.json` unless the
  implementation has a strong reason to add a YAML parser.
- Cover the high-signal source areas first:
  - wire protocol and compact transport;
  - client UI, input, renderer, and app shell;
  - rules, balance, faction catalogs, and generated stats;
  - simulation `Game` API, tick services, pathing, combat, fog, and command/order semantics;
  - lobby/session/replay/match-history surfaces;
  - tests, CI, hooks, and dev scenarios.
- Route source globs or prefixes to one or more likely docs. Many-to-many mapping is expected.
- Include short notes for ambiguous mappings so future agents understand why a doc is listed.
- Keep the map advisory. Do not require a mapped doc to change when mapped source changes.

## Suggested Map Shape

Use a boring dependency-free structure:

```json
{
  "version": 1,
  "routes": [
    {
      "source": [
        "server/crates/protocol/src/lib.rs",
        "server/crates/contract/src/lib.rs",
        "client/src/protocol.js"
      ],
      "docs": [
        "docs/context/protocol.md",
        "docs/design/protocol.md",
        "docs/design/hardening.md"
      ],
      "notes": "Wire DTOs, compact transport, snapshot/event shape, fog-visible protocol data."
    }
  ]
}
```

The exact fields can change if the implementation finds a simpler shape, but the file should stay
human-readable and script-parseable.

## Expected Touch Points

- `docs/doc-map.json`
- `docs/context/README.md` only if it should mention the routing map as an optional navigation aid.
- `docs/context/planning.md` only if plan conventions need to mention future use of the map.

## Out of Scope

- Do not add CI enforcement in this phase.
- Do not add local hook changes in this phase.
- Do not build a semantic sweeper.
- Do not split large design docs.
- Do not block unmapped source files.

## Verification

Use simple checks:

```bash
node -e "JSON.parse(require('fs').readFileSync('docs/doc-map.json', 'utf8')); console.log('doc map parses')"
git diff --check
```

If `docs/context/README.md` or other Markdown docs change, run:

```bash
node scripts/check-wiki.mjs
```

## Manual Testing Focus

Read the map as an agent would. Pick three common source areas, such as protocol, combat, and
client renderer, and confirm the map sends the reader to plausible context and design docs without
claiming exclusive ownership.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff should list the map path, the source
areas covered, any intentionally unmapped areas, and whether Phase 2 can assume a stable schema.
