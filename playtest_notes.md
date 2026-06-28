jeffrey playtest

- [ ] tanks should maintain their existing range while moving, but if they stop moving over the course of three seconds, their range should expand to the existing anti-tank gun range
- [ ] observer unit  (sensor tower for commands?)
- [ ] haraass unit for gun works
- [ ] camoflauge for AT guns (probably not)
- [ ] pause doens't work
- [ ] increase AT gun damage and range
- [ ] when clicking inside of a cluster, they should group and ignore formations, i guess this is hard because sometimes we need to retain formation, so instead we should do it like the closer the move command is i think, the smaller the size of the resultant formation, like sc2 behaviour
- [ ] riflemen need some kind of anti tank behaviour, not very strong, but enough to at least hold off an emergency situation under some situations
- [ ] meth should increase machine gunner move speed to rifleman speed without meth (1.6px/tick)
- [ ] maybe we should return to the old charge behaviour so that riflemen with melee attack on tanks aren't chasing down tanks
- [ ] watch replays as a group, hitting watch replay should create a lobby with all spectators
- [ ] proxy mortars, maybe like two mortars should be able to one shot a huge clump of workers
- [ ] increase the vision range of all units, or make that a training centre upgrade (binoculars)
- [ ] stationary tanks should have more range (hmm, should i add attack range indicators to all units?)
- [ ] if a worker arrives an'd there's not enough resources to build the building, it should continuously stand there trying to build the building when resources are availeble.

- [ ] MORTAR ANIMATION
- [ ] artillery does not requir AT guns
- [ ] pause button broken
- [ ]
- [ ] side paths slowing field that A* is aware of, they are muddy/rocky or something
- [ ] anti tank atack for riflement
- [ ] machine gunners dig in and attacks get change to miss OR increased machine gunner range so one can deter a scout car
- [ ] remove the R&D structure, instead split it into two. steel press is a new building tthat unlocks building anti tank guns, and has an upgrade for artillery production and mortar auto cast. engine industry (name to be workshopped, ask user before implementing) unlocks tank and has a research for command cars
- [ ] oh actually better than above player sbuild factories, instead of gunworks or vehicle works. this produces mortar and scout car by default. player can then conver the factory into either a vehicle works or a gun works
- [ ] workers should not mine oil directly, instead right clicking a worker onto a oil patch will have it build an pump jack for ten seconds. pump jack has 50 hp, cannot move, mines oil at the same rate as a worker, is NOT ARMORED, does not block shooting, requires no tech
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

- [ ] shift right clicking on windows opens the context menu (firefox only) (unsure: fullscreen control-group fix only)

- [ ] selection box doesn't show the production queue (like the +2) obviously enough

- [ ] would be good to see the ranges of units, either in the description, or when they're selected?

- [ ] AI is not sending riflemen in waves properly

- [x] rally should be attack move

- [ ] should allow attacking through buildings because it's causing confusing behaviour where units don't attack the enemy

- [ ] AI should attack with first tank, and it doens't seem to be attacking?

- [ ] AI is producing workers, but tbecuase the main base is fully saturated, the worker just idle

- [ ] AI machine gunners seem to move to attack? like in the luke vs AI replay, luke's tanks show up, and the MG's seem to unset up, and then set up again?

- [ ] they AI lost 178 units but luke's say it only killed 78? buildings killed and buildings lost is also fucked

- [ ] when clicking different players vision, it doesn't like, switch to only showing what that player has explored. you know how there's the stuff you have explored and haven't?

- [ ] in replay, it should show who is still viewing the replay

- [ ] attempting to resume from replay, being denied because ther'es AI players, then seeking backwards, will kick the viweer into a kind of replay mode except they don't have seek controls anymore

- [x] need to make it so that it's not two tanks per selection (done: PR #181, now four)

- [ ] audio attenuation needs work, player looking at own base, and has some machine gunners all teh way on the other end of the map, and it sounds like the machine gunners are halfway across the map, not the other end. we need to increase the attenuation

- [ ] AI should always build vehicle works and gun works towards the enem,y never behind itself

- [ ] Luke is annoyed that tanks will have backs to the enemy sometimes

- [ ] two MGs should kill a scout car (partial: scout HP reduced)

- [ ] auto attacking (unsure: too broad)

- [ ] analysis tab is broken and does not update after the first tick

- [ ] replay seek is broken, i think the player in this case clicked a couple of times, and then we get full fog screen, no units spawned, and no replay controls

- [ ] target fire doens't seem to work, it give sthe player feedback that the unit is target firing, but it doesn't. in this ase it happened with riflemen while methamphetamines was researched, not sure if that's related

- [ ] methamphetamines should icnrease rifleman attack speed evne more

- [ ] increase all unit sight range by two

- [ ] the tech is too quick, the movement out of early game units happens too quickly, such that riflemen just suck, and a player isn't realising they ahve to get off of rifleman ASAP

- [ ] the riflemen are so slow, that you basically always want to go to the machine gunners, there's no such thing as a rush

- [ ] should be able to attack own buildings

- [ ] cant deconstruct your own tank traps

- [ ] tanks should prioritize shooting anti tank guns

- [ ] reduce artillery range by 5 and, and put little artillery icons ont he minimap when yhey're firing, like use the ltieral exact artillery thing

- [ ] make tanks eight supply and doule comand car bonus (partial: tanks now 6 supply; bonus unchanged)

- [ ] pathing is still fucked, units getting suck

- [ ] if a tank is not moving, it should rotate itself so the front is towards the enemy

- [ ] AT gun cone qhen shift queued should originate at the position that it's projected to arrive at

- [ ] should be able to click on a resource patch and see how much is left


----


- [x] we currently make it so that users can double tap an order to send it, like double tapping A to send an attack order. but sometimes they mean to tap A then click to attack, but accidentally double tap A first and then click. so their units get deselected. i think it should be okay to like, ignore clicks in this case as long as the mouse doesn't move too far, we assume it's just a mistake? i'm not sure how to think about this, from a game control or human computer interaction sense. we have a fix, but, who knows if it will cause more grief than it solves. (done: PR #134)

- [x] add an option to see movement waypoints the debug setting while in replays (done: PR #98)

- [ ] workers building tank traps should take damage if their tank trap is attacked? or, workers hould take attack priority from tank traps? idk, for some reason workers building tank traps seem invulnerable, so tank traps should not block shots like buildings do. (partial: shots pass through tank traps)

- [x] increase AT gun field of fire by 5 degrees (done: PR #96)

- [ ] when machine gunner spawns, and if it doesn't move, it doesn't seem to attack units that come near it, like maybe it's not setting up or its otherwise stuck (unsure: same-tile arrival fixed)

- [x] in 2v2, you should not get alerts or sound alerts if your teammate is e.g. under attack (done: PR #97)

- [ ] the vehicle works should take way longer to build or take longer, or need something more

- [ ] when activating breakthrough on a command car, or when mousing over the ability, it shoudl dislpay the AOE ring, and icnrease the AOE of rbeakthrough by two

- [x] engineers should be able to make tank traps that vehicles can't pass through but units can (done: PR #145)

- [ ] control groups don't work in replays, like the players don't inherit them

- [ ] add audible countdown before match start (partial: visual countdown exists)

- [ ] reduce scout car hp by 50% (partial: 150 to 100)

- [ ] make At guns available automatically from the Gun Works and not require AT gun research.

- [x] magic anchor should anchor units, but it should be a visual effect iwth a radius so it's more obvious, and it should add movement speed towards the anchor. as in, walking away you move with reduced movement speed, walking toward the anchor should porivde increased movement speed (done: PR #38)

- [x] create a stats wiki, how armor works, etc (done: wiki/stats exists)


-----

- [ ] make the line shot a deep blue color that leaves a streak

- [ ] command cars should not count towards seleciton limit

- [ ] make it so taht mortarts shoot instantly and have no setup time, and reduce their fly time to 1s, and they do 1.5x the damage they do now (partial: no setup and higher inner damage; fly time still ~2.25s)

- [ ] ekat line shot and dash, if targetted outside of max range, should just do a max range use of the ability, and they should not have ekat walk towards a position wher ehtey can use the ability. you should be able to shift queue a dash also. also you should be able to dash over walla nd buildings.

- [ ] when a mortar is autocasting on a unit that is attack moved but stantionary, it keeps predicted their attack will not be at their location

- [x] make it so tanks take up more selection slots (done: PR #181)

- [ ] attempting to join a replay in progress doesn't give you vision, it's all fog, but when reclaiming position from this all fog mode, the top bar actualy works

- [ ] when host leaves the replay, it seems to kill the repolay?

- [ ] add an are you sure you want to close the tab, because control w kill the tab

- [ ] when one player leaves the match it should watch the replay

- [ ] mortar teams shoudl should even if they don't hav eline of sight

- [ ] when resuming from replay, the top bar hud which displayes resources and supply does not work, it's frozen at the dol values

- [ ] whenresuming from replay, it should rpeserve the camera location form the replay, and not jump back to the camera start location

- [ ] seeking backwards should also not reset the camera location


-----

- [x] lobby system which allows spectating live matches, and joining replay rooms (done: room/lab phases and match history replay rooms)

- [x] configurable hotkeys (done: hotkey profiles)

- [x] small countdown before the game starts (done: matchCountdown)

- [ ] tanks still getting stuck on the sides of buildings!

- [ ] users houdl not be able to end a replay lobby by hitting back to lobby

- [ ] lat ! 82, slow 2, jit 7373 (partial: net/render diagnostics merged)

- [x] make alerts silent on replays (done: spectator/replay alert audio suppressed)

- [x] make tanks take up multiple spots in the selection box (done: PR #181)

- [ ] replay playback should not end if one player in the replay leaves

- [x] confirm if scout car starts with smoke grenade (done: 2 smoke uses configured)

- [ ] sometimes refreshing puts the player back in the replay they were just in

- [ ] workers on build orders should ignore unit blockage

- [ ] when seeking in replays, add a visual indicator that it's working, and progress so far. potentially, could also play visual feedback as the game speeds forwrad

- [ ] create checkpoints for replays


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

- [ ] dead infantry should leave a permanent black/red spot on the ground

- [ ] dead vehicles should leave a permanent blackened spot in their silhouette on the ground

- [ ] prevent any player from playing as black or red

- [ ] add an APM counter

- [x] store all played replays in a database and make them replayable from the lobby (done: match history exists)

- [ ] implement roads that allow faster movement
