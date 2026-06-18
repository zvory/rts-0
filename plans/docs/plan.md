# Docs Routing and Health Plan

## Purpose

Keep the context-capsule split useful without building a heavy documentation governance system. The
first step is an advisory routing map that helps agents and future tools find the likely owning docs
for a changed source area. The second step is a deterministic docs-health check that keeps capsules
small, validates the routing map, catches broken local doc links, and runs in CI plus local commit
hooks.

## Terminology

- **Docs-health script:** a deterministic, dependency-light hygiene check. It should not judge
  semantic drift or require waivers; it only enforces simple repository rules such as parseable map
  data, existing referenced docs, local Markdown link validity, and context capsule size limits.
- **Sweeper:** a future semantic review pass that can use the routing map to compare code changes
  against likely docs. It is not part of this plan.

## Phase Summaries

### [Phase 1 - Advisory Routing Map](phase-1.md)

Create a small advisory source-to-doc routing map under `docs/`. The map should cover the major
contract and gameplay surfaces without pretending ownership is one-to-one or blocking architecture
changes. It should be easy for agents to read, easy for scripts to parse, and useful to a future
sweeper, but it should not add CI enforcement by itself.

### [Phase 2 - Docs Health Script, CI, and Hooks](phase-2.md)

Add a cheap docs-health script and wire it into the existing CI and local hook paths. The check
should validate the routing map, enforce a hard 5 KiB cap for `docs/context/*.md` capsules, and
catch broken local Markdown links in docs and plans. Before enabling the cap, trim any current
oversized capsules by moving details into existing design docs or narrowly scoped reference docs.

## Overall Constraints

- Keep this lightweight. Do not add a waiver system, PR labels, LLM review gate, or semantic drift
  classifier in these phases.
- Treat the routing map as navigation metadata, not architecture policy. Architecture checks remain
  owned by the existing crate, client, lobby, and sim architecture guardrails.
- Prefer dependency-free tooling. A JSON map is acceptable and likely simpler than YAML because Node
  can parse it without a new package.
- Keep context capsules as routers. They should point to design docs and code seams, not duplicate
  long scenario inventories or CI implementation history.
- Apply the 5 KiB hard limit only to `docs/context/*.md` capsules in this plan. Design docs may stay
  larger while they remain the source of truth; any future design-doc splitting should be a separate
  cleanup effort.
- Local hooks should stay cheap. The docs-health hook path must not compile Rust, start a server, or
  run browser tests.
- CI should run docs health early enough that docs-only PRs get the same hygiene coverage as code
  PRs.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what the next agent should do and what should be manually tested. Manual testing notes should
  cover core behavior, not an exhaustive test matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Suggested Execution

```bash
scripts/phase-runner.sh --plan docs 1 --pr --wait
scripts/phase-runner.sh --plan docs 2 --pr --wait
```
