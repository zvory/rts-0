# Phased plan convention

Use this directory for multi-phase or phased implementation plans. Each plan gets its own
`plans/<one-word-name>/` directory, with a short lowercase directory name that is easy to reference
in later tasks.

Each plan directory must contain a brief `plan.md` entry point and one file per phase. Use simple
phase filenames such as `phase-1.md`, `phase-2.md`, and `phase-3.md` unless a more specific name is
clearer.

`plan.md` must include:

- A plain-language three sentence summary of each phase.
- Overall constraints and important considerations that apply across the whole effort.
- A requirement that, after implementing each phase, the agent provides a handoff message for the
  next agent.
- A requirement that each handoff message names the core features that should be manually tested.
  This should not be a comprehensive test matrix.
- A requirement to merge to `main` after each phase before starting the next phase.

Each phase document should describe its scope, expected code or documentation touch points,
verification, manual testing focus, and handoff expectations. When a phase is complete, mark that
phase document as done in the implementation commit for that phase.
