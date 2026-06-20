
oti vs matthew

implement pause game in live matches, 3 pauses

we need to make city centre take like five more seconds to build. and does it provide more supply than a supply depot? if so, we should make it provide the same amount of supply

record all player keystrokes and moue clicks so we can diagnose 

spectator should not get alerts that play sound or at all

a moving a unit that sees an enemy but can't attack it beause ther'es a building in the way, player reports that their unit just stands there

riflemen an infantry and cout cars just do too little damage to buidlings. maybe we should bump the HP of all armoed units by like 25% and then make armor block 50% of damage instead of 75%

buildings should block line of sight

get this working on electron


----


luke playtest

make the debug mdoe more obvious

shift right clicking on windows opens the context menu

selection box doesn't show the production queue (like the +2) obviously enough

would be good to see the ranges of units, either in the description, or when they're selected?

AI is not sending riflemen in waves properly

rally should be attack move

should allow attacking through buildings because it's causing confusing behaviour where units don't attack the enemy

AI should attack with first tank, and it doens't seem to be attacking?

AI is producing workers, but tbecuase the main base is fully saturated, the worker just idle


AI machine gunners seem to move to attack? like in the luke vs AI replay, luke's tanks show up, and the MG's seem to unset up, and then set up again?

they AI lost 178 units but luke's say it only killed 78? buildings killed and buildings lost is also fucked

when clicking different players vision, it doesn't like, switch to only showing what that player has explored. you know how there's the stuff you have explored and haven't?

in replay, it should show who is still viewing the replay

attempting to resume from replay, being denied because ther'es AI players, then seeking backwards, will kick the viweer into a kind of replay mode except they don't have seek controls anymore

need to make it so that it's not two tanks per selection

audio attenuation needs work, player looking at base, and has some machine gunners all teh way on the other end of the map, and it sounds like the machine gunners are halfway across the map, not the other end. we need to increase the attenuation

AI should always build vehicle works and gun works towards the enem,y never behind itself

Luke is annoyed that tanks will have backs to the enemy sometimes

two MGs should kill a scout car

auto attacking 

analysis tab is broken and does not update after the first tick

replay seek is broken, i think the player in this case clicked a couple of times, and then we get full fog screen, no units spawned, and no replay controls


target fire doens't seem to work, it give sthe player feedback that the unit is target firing, but it doesn't. in this ase it happened with riflemen while methamphetamines was researched, not sure if that's related

methamphetamines should icnrease rifleman attack speed evne more

increase all unit sight range by two

the tech is too quick, the movement out of early game units happens too quickly, such that riflemen just suck, and a player isn't realising they ahve to get off of rifleman ASAP

the riflemen are so slow, that you basically always want to go to the machine gunners, there's no such thing as a rush

should be able to attack own buildings 


cant deconstruct your own tank traps

tanks should prioritize shooting anti tank guns

reduce artillery range by 5 and, and put little artillery icons ont he minimap when yhey're firing, like use the ltieral exact artillery thing

make tanks eight supply and doule comand car bonus

pathing is still fucked, units getting suck

if a tank is not moving, it should rotate itself so the front is towards the enemy

AT gun cone qhen shift queued should originate at the position that it's projected to arrive at 


should be able to click on a resource patch and see how much is left


----


we currently make it so that users can double tap an order to send it, like double tapping A to send an attack order. but sometimes they mean to tap A then click to attack, but accidentally double tap A first and then click. so their units get deselected. i think it should be okay to like, ignore clicks in this case as long as the mouse doesn't move too far, we assume it's just a mistake? i'm not sure how to think about this, from a game control or human computer interaction
sense. we have a fix, but, who knows if it will cause more grief than it solves. 


add an option to see movement waypoints the debug setting while in replays


workers building tank traps should take damage if their tank trap is attacked? or, workers hould take attack priority from tank traps? idk, for some reason workers building tank traps seem invulnerable, so tank traps should not block shots like buildings do.

increase AT gun field of fire by 5 degrees





when machine gunner spawns, and if it doesn't move, it doesn't seem to attack units that come near it, like maybe it's not setting up or its otherwise stuck

in 2v2, you should not get alerts or sound alerts if your teammate is e.g. under attack


the vehicle works should take way longer to build or take longer, or need something more

when activating breakthrough on a command car, or when mousing over the ability, it shoudl dislpay the AOE ring, and icnrease the AOE of rbeakthrough by two


engineers should be able to make tank traps that vehicles can't pass through but units can

control groups don't work in replays, like the players don't inherit them

add audible countdown before match start

reduce scout car hp by 50%

make At guns available automatically from the Gun Works and not require AT gun research. 

magic anchor should anchor units, but it should be a visual effect iwth a radius so it's more obvious, and it should add movement speed towards the anchor. as in, walking away you move with reduced movement speed, walking toward the anchor should porivde increased movement speed


create a stats wiki, how armor works, etc


-----
make the line shot a deep blue color that leaves a streak

command cars should not count towards seleciton limit

make it so taht mortarts shoot instantly and have no setup time, and reduce their fly time to 1s, and they do 1.5x the damage they do now

ekat line shot and dash, if targetted outside of max range, should just do a max range use of the ability, and they should not have ekat walk towards a position wher ehtey can use the ability. you should be able to shift queue a dash also. also you should be able to dash over walla nd buildings. 
when a mortar is autocasting on a unit that is attack moved but stantionary, it keeps predicted their attack will not be at their location


make it so tanks take up more selection slots

attempting to join a replay in progress doesn't give you vision, it's all fog, but when reclaiming position from this all fog mode, the top bar actualy works

when host leaves the replay, it seems to kill the repolay?


add an are you sure you want to close the tab, because control w kill the tab



when one player leaves the match it should watch the replay

mortar teams shoudl should even if they don't hav eline of sight

when resuming from replay, the top bar hud which displayes resources and supply does not work, it's frozen at the dol values

whenresuming from replay, it should rpeserve the camera location form the replay, and not jump back to the camera start location

seeking backwards should also not reset the camera location


-----

lobby system which allows spectating live matches, and joining replay rooms 


configurable hotkeys

small countdown before the game starts

tanks still getting stuck on the sides of buildings!

users houdl not be able to end a replay lobby by hitting back to lobby

lat ! 82, slow 2, jit 7373

make alerts silent on replays

make tanks take up multiple spots in the selection box

replay playback should not end if one player in the replay leaves


confirm if scout car starts with smoke grenade


sometimes refreshing puts the player back in the replay they were just in


workers on build orders should ignore unit blockage 


when seeking in replays, add a visual indicator that it's working, and progress so far. potentially, could also play visual feedback as the game speeds forwrad

create checkpoints for replays


-------

- map editor hsould allow placement of resources in custom fashion




ecnomy rework:
- start with one engineer
- selecting an engineer will highlight available oil and steel patches
- engineers build mines and refineries by right clicking onto a patch. it costs fifty steel for either
 - refineries and mines have the same HP as workers and have no armor
 - 


- when the game is won or lost, it should automatically scrub back to the beginning of the replay, and start playing back at 2x speed. players can close the victory screen to see the replay better. 

 - beta should deploy automatically from github, but the server should wait until all matches are drained (or twenty minutes) whichever is shorter before killing itself

- tanks should not rotate their turrets back ot the centre, unless they start moving 
- dead infantry should leave a permanent black/red spot on the ground
- dead vehicles should leave a permanent blackened spot in their silhouette on the ground
- prevent any player from playing as black or red
- add an APM counter
- store all played replays in a database and make them replayable from the lobby
- implement roads that allow faster movement



