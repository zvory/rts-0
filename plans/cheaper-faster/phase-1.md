# Phase 1 - Build and Try It on Beta

## Phase Status

- [ ] Pending.

## Objective

Implement the complete simple workflow and try it on beta before touching mainline.

## Work

- Add explicit beta and mainline Fly configs. Leave mainline's deployed lifecycle unchanged during
  this phase; configure beta as `performance-1x`/2 GB with autostart, clean autostop, and zero
  minimum running Machines.
- Update `deploy.sh` so each channel always selects its own config and cannot silently apply the
  other channel's size or lifecycle.
- Add a tiny always-on launcher with fixed beta and mainline buttons and plain startup status. It
  may contact only the two hard-coded game origins and must preserve the destination path and query.
- Redirect when the selected server is responsive. Do not invent detailed readiness states or make
  startup-time promises.
- Add a short bounded initial WebSocket retry in the game client. Keep one active socket and avoid
  duplicate automatic joins or leaked retry timers.
- Keep direct game URLs working.
- Document the launcher, beta lifecycle, headless-AI limitation, and exact rollback commands.

Expected touch points include the Fly configs, `deploy.sh`, a small launcher directory, the client
connection/bootstrap code, focused contract tests, deploy-asset checks, and deployment docs.

## Verification Before Deployment

- Run `node tests/select-suites.mjs --from=<base>` and the selected focused checks.
- Test only that the launcher rejects arbitrary upstream targets.
- Validate both Fly configs and run `node scripts/check-deploy-assets.mjs`.
- Run the normal owned-PR workflow and wait for the PR to merge.

Do not create a broad launcher matrix or cold-start statistics suite.

## Deployed Acceptance

After explicit approval for the paid remote changes:

1. Confirm beta has no active room, deploy the merged phase, and stop beta.
2. Use the launcher as a normal user. Confirm the starting message appears and the beta lobby opens
   without a manual refresh.
3. Play or spectate one short match and confirm beta remains running while connected.
4. Close all browser connections and confirm beta eventually stops.
5. Repeat the stopped-to-lobby workflow once, using one real room or spectator deep link.

If it works and feels reasonable, Phase 1 passes. If it is broken or irritating, make one obvious
small correction or restore beta's previous shared always-on configuration.

## Handoff

Report the deployed build and config, what the agent saw in both cold starts, whether the short
match and idle stop worked, any rough startup times noticed, and the exact rollback command. Mark
this phase done in its implementation commit.
