# Phase 2 - Roll It Out to Mainline

## Phase Status

- [ ] Pending.

## Objective

Apply the beta-proven setup to mainline and confirm the ordinary friends-game workflow still works.

## Preconditions

- Phase 1 is merged.
- The agent successfully used the launcher to cold-start beta twice, connected to a lobby, played
  or spectated briefly, and observed beta stop after leaving.
- No beta problem makes the workflow meaningfully worse than leaving the server always on.

## Work

- Recheck current Fly pricing and the expected cost at the actual play schedule.
- Record mainline's current build, Machine configuration, and rollback command.
- Configure mainline as `performance-1x`/2 GB with autostart, clean autostop, and zero minimum
  running Machines.
- Preserve separate beta/mainline configs, releases, hostnames, secrets, and direct URLs.
- Make the launcher the normal documented entry point while keeping direct game URLs as recovery
  paths.
- Update the short operator documentation and mark the plan complete.

## Verification Before Deployment

- Run `node tests/select-suites.mjs --from=<base>` and the selected focused checks.
- Validate both Fly configs and run `node scripts/check-deploy-assets.mjs`.
- Run the normal owned-PR workflow and wait for the PR to merge.

No new reliability framework or observation tooling is required.

## Deployed Acceptance

After explicit approval for the paid mainline change:

1. Confirm mainline has no active room, then deploy the merged phase.
2. Stop mainline and enter it through the launcher without manually refreshing.
3. Play one short match or equivalent normal session.
4. Confirm beta still reports its own build and remains independent.
5. Close all browser connections and confirm mainline eventually stops.

If that works, the rollout is done. If it does not, restore mainline's previous shared always-on
configuration and leave beta available for any small follow-up fix.

## Handoff

Report the final beta, mainline, and launcher configs; what the agent experienced in the mainline
workflow; the rough expected monthly cost; and the rollback commands. Mark this phase and the plan
done in the implementation commit.
