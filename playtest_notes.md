jeffrey playtest 5
- [ ] pause is not interacting well with movement prediction for the player who did not pause
- [x] select idle worker button (done: PR #936, the HUD shows an authoritative idle-worker count and selects all idle workers)

jeffrey playtest 4
- [x] units that have move while firing capability, on move command should always move toward the destination and shoot. but on attack move, they shoudl stop once they reach an enemy. (done: PR #887, moving-fire units keep Move destinations while Attack Move acquires and stops for enemies)
- [x] anti tank gun setting up direction is fucked up and doesn't reflect wher ethe mouse is, as in the preview cone is wrong. but it does set up in the right direction (done: PR #891, the setup preview uses the same authoritative direction as execution)
- [ ] open in lab button during replays
- [ ] when machine gunner is running towards a entrenched rifleman, he just sets up outside rifleman range and kills him, so bump rifleman range by 1
- [ ] we do get too much money because we max so fast
- [ ] workers should bounce to next additional base with free steel
- [x] make scout car breakthrough aura permanent (done: Command Cars continuously grant a permanent 1.2x speed aura; active Breakthrough remains stronger)
- [ ] make pumpjacks way easier to build.
- [ ] the gun sounds are too constant and loud, I want information I can use and pew pew pew isnt helping me much. if we could have less audio noise and more notifications for minerals running out and stuff like that I think it would be better



jeffrey playtest 3
- [ ] artillery does a quarter the damage and has a quarter the cost, but has the same build time and all other stats
- [x] get an artillery whizz bang noise (done: timed incoming-whistle and landing-blast audio reaches the target with the shell impact)
- [x] setting up and unsetting up an artillery should take twice as long (done: setup and teardown doubled from three to six seconds)
- [x] create 1v1 map (done: PR #932 updated the default authoritative 1v1 map)
- [ ] artillery is just unfun!



- [x] TODO: Tiger raster pass: hull/turret alignment is off; the turret appears to rotate around an offset point instead of its visual center. Fix the PNG rig pivot/anchor alignment later. (done: the active Tiger atlas uses a visible-center turret origin with runtime coverage)

- [ ] TODO: download an FW 189 scout plane sound effect

jeffrye playtest 2

- [x] artillery should be visible in the map when they fire, not just on the minimpa, they also get the full cycle visibilyt (done: artillery firing reveal projects the firing gun as a normal targetable world entity and adds global minimap firing markers)

- [x] artillery should have the optoin to blanket fire randomly within the firing cone. you should be able to stop this with stop. and resume with c for blanket fire. point fire is unaffected. just like point fire, this should be queueable after a setup. finally, increase the minimum range of artillery by 10. (done: Blanket Fire command, C hotkey, Stop cancellation, queued fire planning, and 25-tile artillery minimum range)

- [x] sometimes tank range does not increase when they stop moving. (done: stationary tank range ramps to 14 tiles after three seconds and resets on movement)

- [x] when a unit dies, it provides vision for a few seconds of the area it could see before it died. but unfortunately, enemy units and buildings in this revealed area are not targetable, which is difficult for players to understand. so units in this revealed area shoudl be targetable. (done: death vision is ordinary temporary team sight, with direct attacks, queued attacks, and auto-acquisition allowed)


jeffrey playtest

- [x] tanks should maintain their existing range while moving, but if they stop moving over the course of three seconds, their range should expand to the existing anti-tank gun range (done: PR #507, stationary tank range ramps from 5 to 14 tiles after 3s)
- [ ] observer unit  (sensor tower for commands?)
- [ ] haraass unit for gun works
- [ ] camoflauge for AT guns (probably not)
- [x] pause doens't work (done: live pause controls)
- [x] increase AT gun damage and range (done: PR #489)
- [x] when clicking inside of a cluster, they should group and ignore formations, i guess this is hard because sometimes we need to retain formation, so instead we should do it like the closer the move command is i think, the smaller the size of the resultant formation, like sc2 behaviour (done: distance-scaled formation goals)
- [x] riflemen need some kind of anti tank behaviour, not very strong, but enough to at least hold off an emergency situation under some situations (done: in-range armored fallback targeting)
- [x] meth should increase machine gunner move speed to rifleman speed without meth (1.6px/tick) (done: PR #467)
- [x] maybe we should return to the old charge behaviour so that riflemen with melee attack on tanks aren't chasing down tanks (done: Methamphetamines Riflemen no longer acquire out-of-range targets during move/attack-move orders and keep their path while firing)
- [x] watch replays as a group, hitting watch replay should create a lobby with all spectators (done: replay staging lobbies)
- [ ] proxy mortars, maybe like two mortars should be able to one shot a huge clump of workers
- [ ] increase the vision range of all units, or make that a training centre upgrade (binoculars)
- [x] stationary tanks should have more range (hmm, should i add attack range indicators to all units?) (done: PR #507 stationary range, PR #520 selected-unit range overlays)
- [x] if a worker arrives an'd there's not enough resources to build the building, it should continuously stand there trying to build the building when resources are availeble. (done: build-wait retry)

- [x] MORTAR ANIMATION (done: mortar launch/shell/impact visuals)
- [x] artillery does not requir AT guns (done: PR #521, Heavy Guns unlocks Artillery alongside AT Guns with no second artillery research)
- [x] pause button broken (done: live pause UI)
- [ ] side paths slowing field that A* is aware of, they are muddy/rocky or something
- [x] anti tank atack for riflement (done: in-range armored fallback targeting)
- [x] machine gunners dig in and attacks get change to miss OR increased machine gunner range so one can deter a scout car (done: Entrenchment lets eligible infantry, including Machine Gunners, dig trenches with +1 range, 70% direct miss chance, and 70% area damage reduction)
- [ ] remove the R&D structure, instead split it into two. steel press is a new building tthat unlocks building anti tank guns, and has an upgrade for artillery production and mortar auto cast. engine industry (name to be workshopped, ask user before implementing) unlocks tank and has a research for command cars
- [ ] oh actually better than above player sbuild factories, instead of gunworks or vehicle works. this produces mortar and scout car by default. player can then conver the factory into either a vehicle works or a gun works
- [x] workers should not mine oil directly, instead right clicking a worker onto a oil patch will have it build an pump jack for ten seconds. pump jack has 50 hp, cannot move, mines oil at the same rate as a worker, is NOT ARMORED, does not block shooting, requires no tech (done: PR #510, contextual Pump Jacks mine oil at the old worker rate)
- [ ] maybe move to having two oil patches per base instead of three
- [ ] new building: intelligence. building it allows riflemen to transform into observation posts (provides maybe, 17 tile radius vision which sees through buildings), rifleman gets consumed. costs 25 steel. engineer can build listening post, which listens for commands within a thirty tile radius. basically, enemy units that recieve commands get revealed on the minimap and in the world map as like question marks or X marks, so they can't be targetted, but it's like a sensor tower in
  sc2


- [ ] crazy idea: riflemen and machine gunners can climb on top of buildings, friendly or enemy, and have increased vision range, attacks on them have cahnge to miss?

oti vs matthew

- [x] smoke doens't always go off

- [x] implement pause game in live matches, 3 pauses (done: PR #190)

- [x] we need to make city centre take like five more seconds to build. and does it provide more supply than a supply depot? if so, we should make it provide the same amount of supply (done: PR #182)

- [ ] record all player keystrokes and moue clicks so we can diagnose (partial: net reports only)

- [x] spectator should not get alerts that play sound or at all (done: PRs #97/#183)

- [x] a moving a unit that sees an enemy but can't attack it beause ther'es a building in the way, player reports that their unit just stands there

- [ ] riflemen an infantry and scout cars just do too little damage to buidlings. maybe we should bump the HP of all armoed units by like 25% and then make armor block 50% of damage instead of 75% (partial: scout HP changed)

- [x] buildings should block line of sight


----


luke playtest

- [ ] make the debug mdoe more obvious

- [x] shift right clicking on windows opens the context menu (firefox only) (done: Shift+right-click is handled on mousedown, suppresses the browser context menu, preserves queued orders, and avoids duplicate contextmenu orders)

- [x] selection box doesn't show the production queue (like the +2) obviously enough (done: selected producer details show the active item, +N queue depth, and progress; producer buildings also render queue depth labels)

- [x] would be good to see the ranges of units, either in the description, or when they're selected? (done: PR #520, selected-unit range overlays)

- [x] AI is not sending riflemen in waves properly (done: AI 1.2 stages Rifleman wave cohorts on a line, launches four-Rifleman waves, and excludes already-launched attackers from fresh waves)

- [x] rally should be attack move

- [ ] should allow attacking through buildings because it's causing confusing behaviour where units don't attack the enemy

- [x] AI should attack with first tank, and it doens't seem to be attacking? (done: AI 1.1 launches the first ready Tank as an attack-move wave once the tech gates are met)

- [x] AI is producing workers, but tbecuase the main base is fully saturated, the worker just idle (done: AI 1.1 assigns idle main-base workers to expansion steel once the main steel line is fully saturated)

- [ ] AI machine gunners seem to move to attack? like in the luke vs AI replay, luke's tanks show up, and the MG's seem to unset up, and then set up again?

- [ ] they AI lost 178 units but luke's say it only killed 78? buildings killed and buildings lost is also fucked

- [x] when clicking different players vision, it doesn't like, switch to only showing what that player has explored. you know how there's the stuff you have explored and haven't? (done: replay fog perspective controls immediately rebuild snapshots for selected player vision, including explored/resource state)

- [ ] in replay, it should show who is still viewing the replay

- [ ] attempting to resume from replay, being denied because ther'es AI players, then seeking backwards, will kick the viweer into a kind of replay mode except they don't have seek controls anymore

- [x] need to make it so that it's not two tanks per selection (done: PR #181, now four)

- [x] audio attenuation needs work, player looking at own base, and has some machine gunners all teh way on the other end of the map, and it sounds like the machine gunners are halfway across the map, not the other end. we need to increase the attenuation (done: confirmed resolved in follow-up)

- [ ] AI should always build vehicle works and gun works towards the enem,y never behind itself

- [ ] Luke is annoyed that tanks will have backs to the enemy sometimes

- [ ] two MGs should kill a scout car (partial: scout HP reduced)

- [ ] auto attacking (unsure: too broad)

- [x] analysis tab is broken and does not update after the first tick (done: observer analysis tabs update from later payloads and render current production, units-lost, and resources-lost rows)

- [x] replay seek is broken, i think the player in this case clicked a couple of times, and then we get full fog screen, no units spawned, and no replay controls (done: replay seek restores from room-time/keyframe state and sends an immediate scoped snapshot, including while paused)

- [x] target fire doens't seem to work, it give sthe player feedback that the unit is target firing, but it doesn't. in this ase it happened with riflemen while methamphetamines was researched, not sure if that's related (done: ordered attacks retain the commanded visible hostile target so auto-acquisition does not steal target fire)

- [x] methamphetamines should icnrease rifleman attack speed evne more (done: Methamphetamines reduces Rifleman attack cooldown to 75% and preserves moving-fire behavior)

- [ ] increase all unit sight range by two

- [ ] the tech is too quick, the movement out of early game units happens too quickly, such that riflemen just suck, and a player isn't realising they ahve to get off of rifleman ASAP

- [ ] the riflemen are so slow, that you basically always want to go to the machine gunners, there's no such thing as a rush

- [x] should be able to attack own buildings (done: explicit self-attack orders are accepted and resolved, including Panzerfaust impacts)

- [x] cant deconstruct your own tank traps (done: PR #509, workers can deconstruct completed tank traps)

- [x] tanks should prioritize shooting anti tank guns (done: current targeting priority checks in-range AT guns first)

- [x] reduce artillery range by 5 and, and put little artillery icons ont he minimap when yhey're firing, like use the ltieral exact artillery thing (done: artillery max range was reduced and firing events draw the artillery rig icon on every player's minimap)

- [ ] make tanks eight supply and doule comand car bonus (partial: tanks now 8 supply; command car bonus increased from 12 to 20, not doubled to 24)

- [ ] pathing is still fucked, units getting suck

- [ ] if a tank is not moving, it should rotate itself so the front is towards the enemy

- [x] AT gun cone qhen shift queued should originate at the position that it's projected to arrive at (done: queued AT-gun setup computes facing from the arrived/projected position before preserving later queued orders)

- [ ] should be able to click on a resource patch and see how much is left


----


- [x] we currently make it so that users can double tap an order to send it, like double tapping A to send an attack order. but sometimes they mean to tap A then click to attack, but accidentally double tap A first and then click. so their units get deselected. i think it should be okay to like, ignore clicks in this case as long as the mouse doesn't move too far, we assume it's just a mistake? i'm not sure how to think about this, from a game control or human computer interaction sense. we have a fix, but, who knows if it will cause more grief than it solves. (done: PR #134)

- [x] add an option to see movement waypoints the debug setting while in replays (done: PR #98)

- [ ] workers building tank traps should take damage if their tank trap is attacked? or, workers hould take attack priority from tank traps? idk, for some reason workers building tank traps seem invulnerable, so tank traps should not block shots like buildings do. (partial: shots pass through tank traps)

- [x] increase AT gun field of fire by 5 degrees (done: PR #96)

- [ ] when machine gunner spawns, and if it doesn't move, it doesn't seem to attack units that come near it, like maybe it's not setting up or its otherwise stuck (unsure: same-tile arrival fixed)

- [x] in 2v2, you should not get alerts or sound alerts if your teammate is e.g. under attack (done: PR #97)

- [ ] the vehicle works should take way longer to build or take longer, or need something more

- [x] when activating breakthrough on a command car, or when mousing over the ability, it shoudl dislpay the AOE ring, and icnrease the AOE of rbeakthrough by two (done: Breakthrough increased from 7 to 9 tiles and shows hover and active aura rings)

- [x] engineers should be able to make tank traps that vehicles can't pass through but units can (done: PR #145)

- [ ] control groups don't work in replays, like the players don't inherit them

- [ ] add audible countdown before match start (partial: visual countdown exists)

- [ ] reduce scout car hp by 50% (partial: 150 to 100)

- [ ] make At guns available automatically from the Gun Works and not require AT gun research.

- [x] magic anchor should anchor units, but it should be a visual effect iwth a radius so it's more obvious, and it should add movement speed towards the anchor. as in, walking away you move with reduced movement speed, walking toward the anchor should porivde increased movement speed (done: PR #38)

- [x] create a stats wiki, how armor works, etc (done: wiki/stats exists)


-----

- [x] make the line shot a deep blue color that leaves a streak (done: Ekat Line Shot renders as a solid dark-blue projectile with a short fading trail)

- [x] command cars should not count towards seleciton limit (done: command budget adds each Command Car's own command weight plus the Command Car cap bonus)

- [ ] make it so taht mortarts shoot instantly and have no setup time, and reduce their fly time to 1s, and they do 1.5x the damage they do now (partial: no setup and higher inner damage; fly time still ~2.25s)

- [x] ekat line shot and dash, if targetted outside of max range, should just do a max range use of the ability, and they should not have ekat walk towards a position wher ehtey can use the ability. you should be able to shift queue a dash also. also you should be able to dash over walla nd buildings. (done: Ekat Dash and Line Shot clamp out-of-range targets, queue as ability intents, and Dash resolves immediately to a standable landing without staging a path)

- [x] when a mortar is autocasting on a unit that is attack moved but stantionary, it keeps predicted their attack will not be at their location (done: mortar autocast no longer leads stationary attack-move targets)

- [x] make it so tanks take up more selection slots (done: PR #181)

- [x] attempting to join a replay in progress doesn't give you vision, it's all fog, but when reclaiming position from this all fog mode, the top bar actualy works (done: confirmed replay joins receive replay start/room-time state and spectator snapshots from the current replay session)

- [x] when host leaves the replay, it seems to kill the repolay? (done: leaving one replay viewer keeps playback alive for remaining viewers)

- [x] add an are you sure you want to close the tab, because control w kill the tab (done: live player matches install a beforeunload warning; spectators, replays, labs, and resolved matches bypass it)

- [ ] when one player leaves the match it should watch the replay

- [x] mortar teams shoudl should even if they don't hav eline of sight (done: PR #512, mortars can fire indirectly at owner-visible targets behind LOS blockers)

- [x] when resuming from replay, the top bar hud which displayes resources and supply does not work, it's frozen at the dol values (done: HUD restores single-player steel/oil/supply spans after replay resource rows and resumes live updates)

- [x] whenresuming from replay, it should rpeserve the camera location form the replay, and not jump back to the camera start location (done: replay-branch starts carry the current camera x/y/zoom into the resumed match)

- [x] seeking backwards should also not reset the camera location (done: replay seek/start transitions carry the existing camera view instead of recentering)


-----

- [x] lobby system which allows spectating live matches, and joining replay rooms (done: room/lab phases and match history replay rooms)

- [x] configurable hotkeys (done: hotkey profiles)

- [x] small countdown before the game starts (done: matchCountdown)

- [ ] tanks still getting stuck on the sides of buildings!

- [ ] users houdl not be able to end a replay lobby by hitting back to lobby

- [ ] lat ! 82, slow 2, jit 7373 (partial: net/render diagnostics merged)

- [x] make alerts silent on replays (done: spectator/replay alert audio suppressed)

- [x] make tanks take up multiple spots in the selection box (done: PR #181)

- [x] replay playback should not end if one player in the replay leaves (done: `returnToLobby` detaches only that viewer; the replay room resets only after the last viewer leaves)

- [x] confirm if scout car starts with smoke grenade (done: 2 smoke uses configured)

- [ ] sometimes refreshing puts the player back in the replay they were just in

- [x] workers on build orders should ignore unit blockage (done: workers on build/gather orders are collision-exempt so traffic cannot strand construction/economy orders)

- [x] when seeking in replays, add a visual indicator that it's working, and progress so far. potentially, could also play visual feedback as the game speeds forwrad (done: room-time controls show timeline progress, keyframe marks, and a pending Seeking tick status)

- [x] create checkpoints for replays (done: replay playback records periodic keyframes and seeks by restoring the nearest prior keyframe before fast-forwarding)


-------

- [ ] map editor hsould allow placement of resources in custom fashion


- [ ] ecnomy rework:

- [ ] start with one engineer

- [ ] selecting an engineer will highlight available oil and steel patches

- [ ] engineers build mines and refineries by right clicking onto a patch. it costs fifty steel for either

- [ ] refineries and mines have the same HP as workers and have no armor


- [ ] when the game is won or lost, it should automatically scrub back to the beginning of the replay, and start playing back at 2x speed. players can close the victory screen to see the replay better.

- [ ] beta should deploy automatically from github, but the server should wait until all matches are drained (or twenty minutes) whichever is shorter before killing itself

- [ ] tanks should not rotate their turrets back ot the centre, unless they start moving

- [x] dead infantry should leave a permanent black/red spot on the ground (done: infantry deaths stamp permanent client-local ground decals)

- [x] dead vehicles should leave a permanent blackened spot in their silhouette on the ground (done: vehicle and support-weapon deaths stamp blackened scorch/hull decals)

- [x] prevent any player from playing as black or red (done: server/client player palettes assign colorblind-safer colors and exclude black/red)

- [ ] add an APM counter

- [x] store all played replays in a database and make them replayable from the lobby (done: match history exists)

- [x] implement roads that allow faster movement (done: five road terrain variants apply a server-authoritative 1.4x movement multiplier and are supported by the map editor)
