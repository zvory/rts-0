# Phase 2 - Retired Pending Specification

## Phase Status

- [x] Done. Retired without implementation.

## Why It Was Retired

The former specification combined route-specific rig construction, animation sampling,
frame-entity derivation, selection detachment, and Pixi compatibility cleanup. It was written from
coarse frame-phase timings before repeatable V8 function profiling existed.

A local CPU flame-graph trial confirmed meaningful rig and copying cost but also showed a different
priority distribution, including dominant fog work and measurable diagnostic overhead. Keeping the
old bundle would turn provisional guesses into executor instructions, so its implementation scope,
targets, and ordered work were removed.

## Replacement Gate

Do not execute this phase with `phase-runner`. Capture a fresh profile from current `origin/main`
with `node scripts/client-flamegraph.mjs --preview`, inspect the ranked functions and source, and
write a new small plan whose boundaries follow current evidence.
