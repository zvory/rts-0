# Phase 2 - Swap App Roles Without DNS Changes

## Phase Status

- [x] Done.

## Objective

Move the two game channels to clearly named Fly apps, repurpose the existing canonical-domain app
as the launcher, and confirm the ordinary friends-game workflow without touching Squarespace DNS.

## Preconditions

- Phase 1 is merged.
- The launcher and beta performance/autostop configs have been deployed successfully to temporary
  or legacy app names, but no custom-domain DNS has changed.
- The user explicitly selected the no-DNS role swap and approved creation of
  `bewegungskrieg-mainline` and `bewegungskrieg-beta`.

## Work

- Recheck current Fly pricing and the expected cost at the actual play schedule.
- Record both legacy game apps' current builds, Machine configurations, secret names, and rollback
  commands.
- Change the explicit game app/config defaults to `bewegungskrieg-mainline` and
  `bewegungskrieg-beta`. Configure both as `performance-1x`/2 GB with autostart, clean autostop, and
  zero minimum running Machines.
- Make the launcher's fixed destinations `https://bewegungskrieg-mainline.fly.dev` and
  `https://bewegungskrieg-beta.fly.dev`; keep arbitrary upstreams impossible.
- Configure the launcher channel to deploy to the existing canonical-domain app
  `rts-0-zvorygin`, preserving its current certificates and Squarespace DNS.
- Preserve separate beta/mainline releases and secrets. Document the raw Fly game URLs as direct
  recovery paths.
- Document a safe cutover order: create and verify both new game apps first, then replace the
  existing mainline app with the launcher last. Leave `rts-0-zvorygin-beta` and
  `rts-0-zvorygin-launcher` stopped as rollback paths; do not destroy them in this phase.
- Update the short operator documentation and mark the plan complete.

## Verification Before Deployment

- Run `node tests/select-suites.mjs --from=<base>` and the selected focused checks.
- Validate all three Fly configs and run `node scripts/check-deploy-assets.mjs`.
- Run the normal owned-PR workflow and wait for the PR to merge.

No new reliability framework or observation tooling is required.

## Deployed Acceptance

After explicit approval for the two paid game-app creations and role swap:

1. Create `bewegungskrieg-mainline` and `bewegungskrieg-beta`, copy the required channel secrets,
   and deploy both game configs.
2. Verify both raw Fly game origins report the merged build and have no active room. Stop each one
   and cold-start it directly once before changing the canonical app.
3. Confirm the legacy mainline app has no active room, then deploy the launcher config to
   `rts-0-zvorygin`. Do not change or remove its certificates or Squarespace DNS records.
4. Open `bewegungskrieg.net`, use the launcher to cold-start beta twice, and reach the beta lobby
   without manually refreshing. Play or spectate briefly and confirm beta remains running while
   connected, then closes down after browser connections leave.
5. Use the launcher to cold-start mainline, play one short match or equivalent normal session, and
   confirm mainline stops after browser connections leave.
6. Open an existing canonical mainline deep link and confirm its path, query, and fragment survive
   the redirect to `bewegungskrieg-mainline.fly.dev`.
7. Confirm both channels report their own builds and remain independent. Stop the two superseded
   apps, but retain them for rollback.

If that works, the rollout is done. If it does not, redeploy the game config to
`rts-0-zvorygin` and restart `rts-0-zvorygin-beta`; DNS remains unchanged throughout.

## Handoff

Report the final beta, mainline, and launcher configs; what the agent experienced in both cold-start
workflows; the rough expected monthly cost; and the exact no-DNS rollback commands. Mark this phase
and the plan done in the implementation commit.
