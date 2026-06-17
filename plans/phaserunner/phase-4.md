# Phase 4 - Stable Entrypoint Cutover

## Status

Done.

## Objective

Make `scripts/phase-runner.sh` dispatch to the Agents SDK-capable Node runner.

## Scope

- Keep `scripts/phase-runner.sh` as the stable operator command.
- Make the wrapper execute `node scripts/phase-runner-agents.mjs`.
- Preserve `RTS_PHASERUNNER_BIN` as an explicit local override for debugging alternate runners.
- Remove the superseded native runner from the active Cargo workspace.
- Keep `scripts/phase-runner-agents.mjs` as the maintained implementation.

## Verification

- `scripts/phase-runner.sh --help`
- `scripts/phase-runner.sh --plan svg phase-0 --pr --dry-run`
- `node tests/phase_runner_agents.mjs`
- `cargo metadata --manifest-path server/Cargo.toml --no-deps`
- `git diff --check`

## Manual Testing Focus

Confirm the command operators already use still works and no longer implies Rust.

## Handoff Expectations

If a later canary fails, debug the Node runner and the Codex executor path first. Do not add another
runner implementation without a new explicit architecture decision.
