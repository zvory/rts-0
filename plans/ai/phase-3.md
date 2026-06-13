# Phase 3 - Macro Managers

Status: not started

## Goal

Implement the economy, build, and production managers for the launch AI. By the end of this phase,
the new AI should fairly build a strong economy, expand for oil, tech toward Scout Cars and Tanks,
and avoid common macro bugs from authored states and normal starts.

## Scope

- Economy manager:
  - keep worker production active toward phase targets
  - saturate main steel
  - assign oil earlier than the old saturation AI when expansion/tech phases need it
  - preserve local resource assignment after expansion where possible
- Build manager:
  - build Depots before supply stalls
  - build Barracks for first Rifleman production
  - build an earlier second City Centre when expansion prerequisites are met
  - build the tech path for Scout Cars and Tanks using current rules requirements
  - handle blocked placement and no-builder cases without repeated bad commands
- Production manager:
  - produce Riflemen first
  - transition into Scout Cars when tech is ready
  - transition into Tanks when tank tech is ready
  - keep queues bounded and reserve resources so managers do not overspend the same steel/oil
- Route build, train, research, and gather execution through `AiActionContext` / `ai_core::actions`.
  If a manager needs a new macro action, add or extend a shared action helper rather than emitting
  raw commands and duplicating budget or reservation logic inside the manager.
- Keep Machine Gunners, AT Teams, Artillery, and Command Cars out of the required launch path for
  this phase unless existing rules require a small defensive fallback.

## Expected Touch Points

- `server/crates/ai/src/ai_core/`
- `server/crates/ai/src/ai_core/actions.rs`
- `server/crates/ai/src/ai_core/facts.rs`
- `server/crates/ai/src/ai_core/profiles.rs`
- Scenario tests from Phase 1

## Verification

- Fast manager tests for each macro target and blocker:
  - supply low -> Depot
  - main steel below target -> worker/steel assignment
  - expansion ready -> City Centre
  - Scout Car phase -> required tech and production
  - Tank phase -> required tech, unlock, and production
  - unaffordable or no-builder states -> no invalid command spam
  - competing managers cannot double-spend the same resources or reserve the same worker/building
- Short scenario tests that jump directly to expansion, Scout Car, and Tank states.
- A bounded normal-start smoke proving the new profile reaches at least Rifleman, expansion, and
  Scout Car milestones.

## Manual Testing Focus

- Watch one self-play run from a normal start and confirm the AI does not idle with banked
  resources during early macro.
- Watch one seeded mid-game scenario and confirm the AI can continue teching instead of needing a
  full opening history.

## Handoff

The handoff should include the fastest macro test command, the current milestone timings, and any
known tuning concerns such as expansion timing, oil starvation, or supply stalls.
