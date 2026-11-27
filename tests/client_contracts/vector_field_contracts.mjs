import assert from 'node:assert/strict';
import {ABILITY,KIND} from '../../client/src/protocol.js';
import {Input} from '../../client/src/input/index.js';
import {ClientIntent} from '../../client/src/client_intent.js';
import {drawVectorField} from '../../client/src/renderer/vector_field.js';
import {RecordingGraphics} from './pixi_fakes.mjs';
const input=Object.create(Input.prototype);
const caster={id:9,owner:1,kind:KIND.MAGIC_ADEPT,x:200,y:200,hp:150};
input.state={playerId:1,map:{tileSize:32},selectedEntities:()=>[caster]};
input.clientIntent=new ClientIntent();
input._selectedOwnUnitIds=()=>[9];input._resourceAtScreen=()=>null;
input._groundAtScreen=(x,y)=>({x,y});
const commands=[];input.commandInteraction={issueCommand:c=>commands.push(c)};
input.clientIntent.beginCommandTarget({kind:'ability',ability:ABILITY.VECTOR_FIELD});
assert.equal(input._issueTargetedCommand({x:300,y:250}),false);
assert.deepEqual(input.clientIntent.vectorFieldCenter,{x:300,y:250});
assert.equal(commands.length,0);
assert.equal(input._issueTargetedCommand({x:300,y:250}),false,'zero-length direction cannot place');
assert.equal(input._issueTargetedCommand({x:240,y:210}),true);
assert.deepEqual(commands[0],{c:'vectorField',units:[9],x:300,y:250,directionX:-60,directionY:-40});
assert.equal(input.clientIntent.vectorFieldCenter,null);
input.clientIntent.vectorFieldCenter={x:1,y:2};input.clientIntent.endCommandTarget();
assert.equal(input.clientIntent.vectorFieldCenter,null,'cancel clears the fixed center');
for(const direction of [[1,0],[0,1],[-1,-1]]){
 const g=new RecordingGraphics();drawVectorField(g,400,300,direction);
 const points=g.calls.filter(c=>['moveTo','lineTo'].includes(c[0]));
 assert(points.length>=10);
 assert(points.every(c=>Number.isFinite(c[1])&&Number.isFinite(c[2])));
}

const {selectedAbilityAffordances}=await import('../../client/src/hud_command_card.js');
for(const daoMaster of [false,true]){
 const entries=selectedAbilityAffordances({playerId:1,factionId:'cultivators',groupCooldownClocks:()=>[]},
   [{id:1,owner:1,kind:KIND.MAGIC_ADEPT,dao:'magic',daoMaster,hp:150}]);
 assert(entries.some(e=>e.definition.ability===ABILITY.VECTOR_FIELD));
 assert(!entries.some(e=>e.definition.ability===ABILITY.EKAT_MAGIC_ANCHOR));
}
const ekatEntries=selectedAbilityAffordances({playerId:1,factionId:'ekat',groupCooldownClocks:()=>[]},
 [{id:1,owner:1,kind:KIND.EKAT,hp:150}]);
assert(ekatEntries.some(e=>e.definition.ability===ABILITY.EKAT_MAGIC_ANCHOR));
assert(!ekatEntries.some(e=>e.definition.ability===ABILITY.VECTOR_FIELD));

// Particle travel is physical world speed, independent of field rank or render rate.
for (const sizeTiles of [4,5]) {
 const frames=[1000,1010].map(now=>{
  const g=new RecordingGraphics();drawVectorField(g,400,300,[1,0],32,{sizeTiles,now,particles:true,seed:7});
  return g.calls.filter(c=>c[0]==='drawCircle');
 });
 assert.equal(frames[0].length,sizeTiles*sizeTiles*2);
 let checked=0;
 for(let i=0;i<frames[0].length;i++) {
  const dx=frames[1][i][1]-frames[0][i][1];
  if(dx<0)continue; // edge respawn
  assert(Math.abs(dx-0.3)<1e-7); // 30 world px/s over 10 ms
  assert.equal(frames[1][i][2],frames[0][i][2]);checked++;
 }
 assert(checked>0);
}

// Minimap clicks use the same fixed center and dedicated command as viewport targeting.
const {Minimap} = await import('../../client/src/minimap.js');
const minimap = Object.create(Minimap.prototype);
minimap.state = input.state;
minimap.clientIntent = new ClientIntent();
minimap.commandInteraction = input.commandInteraction;
minimap.clientIntent.beginCommandTarget({kind:'ability', ability:ABILITY.VECTOR_FIELD});
const beforeMinimap = commands.length;
assert.equal(minimap._issueOrder(300, 250, true), false);
assert.equal(minimap._issueOrder(300, 250, true), false);
assert.equal(commands.length, beforeMinimap);
minimap._issueOrder(240, 210, true);
assert.deepEqual(commands.at(-1), commands[0], 'minimap uses vectorField, without queueing');
assert.equal(minimap.clientIntent.vectorFieldCenter, null);

console.log('vector_field_contracts: ok');
