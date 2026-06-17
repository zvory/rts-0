# Phase 3 - Agents SDK Executor Mode

## Status

Done.

## Objective

Add the OpenAI Agents SDK path without destabilizing the default phase execution path.

## Scope

- Add root `package.json` and `package-lock.json` for repo-local agent tooling.
- Pin `@openai/agents` and `zod`.
- Keep `--executor codex-cli` as the default.
- Add `--executor agents-sdk` using the OpenAI Agents SDK with Codex MCP.
- Keep the same handoff JSON contract after executor completion.

## Verification

- `node tests/phase_runner_agents.mjs`
- Verify `@openai/agents` named imports used by the runner.
- `scripts/phase-runner-agents.mjs --executor agents-sdk --dry-run`
- `git diff --check`

## Manual Testing Focus

Do not run a live implementation phase through `--executor agents-sdk` until a deliberate canary is
approved. Dry-run the mode first and keep `codex-cli` as the default parity path.

## Handoff Expectations

Future agentic capabilities should extend the Agents SDK executor path rather than creating another
runner implementation.
