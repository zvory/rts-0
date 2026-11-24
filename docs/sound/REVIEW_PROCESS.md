# Sound Review Process

Use this process when auditioning candidate sound effects.

## Generated sound provenance (required)

Whenever generating sound effects, save the exact prompt and generation method alongside the
resulting audio before sharing it for review. This applies to every candidate, variation, and
regeneration, including rejected auditions; do not wait until a sound is selected for integration.

Use a per-file JSON sidecar or a batch `generation.json` in the same directory as the audio. A batch
manifest must map each output filename to its own complete record. Preserve:

- The exact prompt submitted, verbatim, including negative instructions. Record any additional
  prompt fields separately rather than replacing the submitted text with a summary.
- Provider, API endpoint and method, model ID/version, and the actual request parameters used,
  including duration, prompt influence, output format, and seed when supported. Mark provider
  defaults or unavailable values explicitly; never invent a seed or model version.
- The invocation used: CLI/package and version plus command arguments, or script path and version
  plus invocation. For procedural synthesis, save the generator source, parameters, and random
  seed; mark the prompt as not applicable when no text prompt was used.
- Generation date, raw output filename and SHA-256, and any returned request/generation ID when
  available. Keep separate records for repeated requests even when their prompts are identical.
- Every subsequent trim, fade, gain change, normalization, resample, encoding, or other processing
  step, with exact settings or the preserved processing script and invocation. Map the processed
  filename and SHA-256 back to its raw source; retain the original during review.

Never put credentials, authentication headers, access tokens, or signed URLs in these records.
Document the endpoint and invocation with credential values omitted. Preserve provenance with the
sound when copying or promoting it: integrated assets must have their matching records committed
under `docs/sound/` (or beside the asset), with repository-relative paths to the final files. Keep
existing source and license tracking as well; generation metadata does not establish usage rights.

## Selection Criteria

- Prefer realistic, real-world recordings or convincing naturalistic edits.
- Reject sounds that read as retro/game-style, 8-bit, MIDI-like, synthetic, cartoonish, or obviously generated unless the intended role explicitly calls for that style.
- For combat sounds, search for source material with real transient impact, pressure, low-end body, and believable decay instead of generic arcade SFX packs.

## Flow

1. Play candidate files one by one.
2. For each file, the reviewer says what the sound effect reads as in-game.
3. The listener gives notes and one verdict:
   - `keep`
   - `marginal keep`
   - `reject`
4. Record the verdict and notes before moving to the next file.
5. Do not wire sounds into gameplay during the review pass. Selection and integration are separate steps.

## Notes To Capture

- File path
- Intended role
- What the effect sounds like during playback
- Listener notes
- Verdict
- Any follow-up edit needed, such as trim, gain, EQ, layering, or replacement search

## Current Candidate Set

Review the files in `client/assets/sound/combat/examples/`:

- `combat_mortar_launch_01.mp3` through `combat_mortar_launch_05.mp3`
- `combat_mortar_explosion_01.mp3` through `combat_mortar_explosion_05.mp3`
- `combat_artillery_fire_01.mp3` through `combat_artillery_fire_05.mp3`
- `combat_artillery_explosion_01.mp3` through `combat_artillery_explosion_05.mp3`
- `combat_tank_round_hit_01.mp3` through `combat_tank_round_hit_05.mp3`
- `combat_at_round_hit_tank_01.mp3` through `combat_at_round_hit_tank_05.mp3`

Source links and license details are tracked in `COMBAT_EXAMPLES.md`.
