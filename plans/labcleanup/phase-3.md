# Phase 3 - Directly Executable TypeScript

## Phase Status

- [ ] Not started.

## Objective

Migrate the settled Node-side Lab Interact implementation to strict TypeScript without adding a build
product or starting another architecture redesign. Execute source directly with Node 22.18 or newer,
use a no-emit compiler pass for type checking, and keep the browser client/bridge in native
JavaScript.

## Runtime Decision

Node 22.18 enabled built-in TypeScript type stripping by default without the former experimental
warning. Use that runtime support for erasable TypeScript syntax and follow Node's documented
`nodenext`, `erasableSyntaxOnly`, and `verbatimModuleSyntax` guidance; do not add `tsx`, `ts-node`, a
transpiler, or a bundler solely for this developer tool.

References:

- [Node.js 22.18.0 release notes](https://nodejs.org/en/blog/release/v22.18.0/)
- [Node.js TypeScript execution guidance](https://nodejs.org/download/release/v22.16.0/docs/api/typescript.html)

## Work

### Establish direct TypeScript execution and checking

- Require Node 22.18 or newer for Lab Interact and enforce it with a concise preflight. The repository
  and CI already use Node 22; pin/document the minimum patch level wherever Lab dependencies and
  checks are installed.
- Keep a tiny `scripts/lab-interact/cli.mjs` bootstrap so every operator command, help example,
  recovery string, and skill instruction can continue using the same entry point until the separate
  deep rename. Its only jobs are the Node version check and importing `cli.ts`.
- Rename the remaining Node implementation files under `scripts/lab-interact/` from `.mjs` to `.ts`,
  bottom-up, with explicit `.ts` import specifiers. Spawn the TypeScript daemon using
  `process.execPath` rather than a shell wrapper.
- Add a Lab-scoped TypeScript configuration and a repository script such as
  `check:lab-interact-types` with:
  - `noEmit: true` and `strict: true`;
  - `target: "ESNext"`;
  - `module: "NodeNext"` and `moduleResolution: "NodeNext"`;
  - `allowImportingTsExtensions: true`;
  - `erasableSyntaxOnly: true` and `verbatimModuleSyntax: true`;
  - `skipLibCheck: true`;
  - the minimum DOM library/ambient page declarations required by Puppeteer evaluation closures.
- Add TypeScript 5.8 or newer and Node typings to the repository-owned dependency manifest/lock from
  Phase 2. Do not commit emitted JavaScript, declarations, source maps, or `dist/`.
- Use only erasable syntax: no runtime enums, namespaces, parameter properties, decorators, or path
  aliases. Prefer `import type` and small structural browser/page port interfaces over leaking full
  Puppeteer types into the application layer.

### Type the valuable seams, not the universe

- Type command registry definitions so each command's scope, lane, timeout class, validator, handler,
  and help projection remain associated.
- Type daemon request/response envelopes, normalized errors, session state, service/driver ports,
  process results, capture/media results, artifacts, and browser/page RPC projections.
- Treat CLI JSON, IPC JSON, page RPC data, filesystem JSON, and server responses as `unknown` until
  existing runtime validators narrow them. TypeScript strengthens trusted code after those boundaries;
  it must not replace exact/bounded validation.
- Avoid blanket `any`, `@ts-ignore`, or assertion casts that erase a whole boundary. Localized casts
  at a runtime-validated edge are acceptable when documented and covered by its contract test.
- Keep `client/src/lab_interact_bridge.js` as native browser JavaScript because the client has no
  build step. Add small JSDoc or `@ts-check` only if it is nearly free and does not turn this phase
  into client migration.

### Update repository policy and callers

- Update Lab contracts/smoke imports, fake-driver injection, direct daemon spawn paths, source-text
  assertions, help/recovery strings, and documentation source pointers for `.ts` modules.
- Teach `scripts/check-source-file-sizes.mjs`, the Phase 2 architecture checker, and
  `tests/select-suites.mjs` to cover `.ts` files.
- Include the no-emit typecheck in the focused static/Node gate and ensure the dependency install used
  by CI provides TypeScript, Node types, and the declared browser runtime.
- Do not rename the product, CLI command vocabulary, socket/runtime directory, environment variables,
  or artifact namespace in this phase.

## Expected Touch Points

- `scripts/lab-interact/cli.mjs` plus new `cli.ts`
- `scripts/lab-interact/*.mjs -> *.ts`
- new `scripts/lab-interact/tsconfig.json` or root `tsconfig.lab-interact.json`
- repository npm manifest/lock
- `tests/lab_interact_*.mjs`
- `tests/fixtures/lab_interact_fake_driver.mjs`
- `tests/run-all.sh`
- `tests/select-suites.mjs`
- `scripts/check-source-file-sizes.mjs`
- `scripts/check-lab-interact-architecture.mjs`
- `.github/workflows/main-tests.yml` only if the existing dependency-install step cannot run the check
- `docs/lab-interact-cli.md`
- relevant context/design source pointers and `.agents/skills/lab-interact/` instructions

## Implementation Checklist

- [ ] Add Node-version preflight, strict no-emit config, dependencies, and check script.
- [ ] Convert Node implementation modules bottom-up to `.ts` with erasable syntax.
- [ ] Keep `cli.mjs` as the only compatibility JavaScript implementation file in the tool directory.
- [ ] Type command/IPC/session/error/adapter/capture seams.
- [ ] Preserve runtime validation on all untrusted boundaries.
- [ ] Update tests, fake-driver injection, source policies, selectors, CI, docs, and skill instructions.
- [ ] Prove direct execution has no loader warning or build prerequisite.
- [ ] Confirm no generated output or client build pipeline was added.
- [ ] Mark this phase done in this file in the implementation commit.

## Verification

Use the final script names established during implementation; the intended focused checks are:

```bash
npm ci
npm run check:lab-interact-types
node scripts/lab-interact/cli.mjs help
node scripts/check-lab-interact-architecture.mjs
node scripts/check-source-file-sizes.mjs
node tests/lab_interact_cli_contracts.mjs
node tests/lab_interact_driver_contracts.mjs
node tests/lab_interact_bulk_contracts.mjs
node tests/lab_interact_artifact_contracts.mjs
node tests/lab_interact_recording_contracts.mjs
node tests/lab_interact_fixed_capture_contracts.mjs
node tests/lab_interact_tailnet_preview_contracts.mjs
node tests/select-suites.mjs --verify
node tests/lab_interact_cli_smoke.mjs
node scripts/check-docs-health.mjs
git diff --check
```

Also run the repository's no-Rust Node/static gate and confirm it includes the Lab typecheck. Verify
the tool emits no `dist/`, compiled JavaScript, declaration, or source-map files.

## Acceptance Criteria

- `cli.mjs` is the only compatibility JavaScript implementation file remaining under
  `scripts/lab-interact/`; Node-side implementation modules are strict `.ts`.
- The CLI runs source directly on Node 22.18+ with no loader warning, build prerequisite, runtime TS
  dependency, or emitted code. Below-minimum Node gets a concise version error.
- The strict no-emit check passes without blanket type escapes.
- High-value internal seams are typed and untrusted inputs still pass through runtime validators.
- The Phase 1 live workflow and all focused Lab contracts pass with the TypeScript daemon and fake
  driver paths.
- Architecture/source-size/selector policy includes `.ts`, and the ordinary Node gate runs the
  typecheck.
- The browser bridge/client remain buildless JavaScript and no rename occurred.

## Manual Test Focus

Run help, open a blank scene, spawn and move one unit, inspect it, capture a PNG, and close/shutdown.
Confirm the JSON response and Tailnet preview behavior remain recognizable and that no compile step
was needed before the first command.

## Non-Goals

- Client-wide TypeScript or bundling, conversion of tests, or generated runtime schemas/codegen.
- Exhaustive per-command result modeling, declaration emit/publishing, packaging, or a single binary.
- ESLint/Prettier adoption, stricter optional/index flags unless nearly free, or type perfection in
  Puppeteer third-party surfaces.
- Any deep rename or another architecture redesign.

## Handoff Expectations

Report the Node minimum, dependency/config/check commands, modules converted, remaining JavaScript
boundary, runtime validators preserved, and any localized type assertions. Provide checkpoint evidence
for the live semantic smoke, direct no-build startup, architecture ratchets, and manual
help/open/spawn/order/screenshot/preview/shutdown workflow; recommend rename planning only after those
results are reviewed.
