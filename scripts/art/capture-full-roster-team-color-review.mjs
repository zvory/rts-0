// Representative-pose art review. Full animations and merge remain approval-gated.
import fs from 'node:fs';
import path from 'node:path';
import {runCli} from '../interact/cli.ts';
import {runtimePaths} from '../interact/runtime.ts';
import {buildRosterReviewBoards} from './roster-team-color-review-boards.mjs';
const root=path.resolve(import.meta.dirname,'../..');process.chdir(root);
const out=path.join(root,'target/full-roster-team-color-review');fs.mkdirSync(out,{recursive:true});
const command=async(name,input={})=>{const response=await runCli(['lab',name,JSON.stringify(input)]);if(!response.ok)throw Error(JSON.stringify(response.error));return response.result;};
const palette=[['Blue','#0072b2'],['Vermilion','#d55e00'],['Green','#009e73'],['Pink','#cc79a7'],['Sky blue','#56b4e9'],['Orange','#e69f00'],['Yellow','#f0e442']];
const unit=(kind,label,extra={})=>({kind,label,...extra});
const groups=[
 {id:'kriegsia',factionId:'kriegsia',units:[unit('worker','Engineer'),unit('rifleman','Rifleman'),unit('panzerfaust','Panzerfaust'),unit('machine_gunner','Machine Gunner'),unit('anti_tank_gun','Anti-Tank Gun'),unit('mortar_team','Mortar Team'),unit('artillery','A-19 Artillery'),unit('scout_car','Scout Car'),unit('tank','Tiger tank'),unit('command_car','Command Car'),unit('rocket_launcher','Rocket Truck'),unit('scout_plane','Scout Plane')]},
 {id:'cultivators',factionId:'cultivators',units:[unit('worker','Cultivator worker'),unit('warrior','Novice'),unit('bow_novice','Bow Novice'),unit('wind_adept','Wind Adept'),unit('war_adept','War Adept'),unit('magic_adept','Magic Adept'),unit('wind_adept','Wind Master',{daoMaster:true}),unit('war_adept','War Master',{daoMaster:true}),unit('magic_adept','Magic Master',{daoMaster:true})]},
 {id:'ekat',factionId:'ekat',units:[unit('ekat','Ekat'),unit('golem','Golem')]},
];
const camera={version:1,focus:{x:1760,y:1776},framingScale:1,boundsPolicy:'mapOverscroll'};
const viewport={width:1000,height:800,deviceScaleFactor:2};
const scenarioDir=path.join(root,'server/assets/lab-scenarios'),manifestPath=path.join(scenarioDir,'manifest.json'),scenarioPath=path.join(scenarioDir,'full-paint-review.json');
const manifestBytes=fs.readFileSync(manifestPath),runtime=runtimePaths(root);
if(fs.existsSync(scenarioPath))throw Error('Temporary full review scenario already exists');
if([runtime.state,runtime.socket,runtime.lock].some(file=>fs.existsSync(file)))throw Error('Full review capture requires an unused Interact runtime');
const selectedGroup=process.argv.find(arg=>arg.startsWith('--group='))?.slice(8);
if(selectedGroup&&!groups.some(g=>g.id===selectedGroup))throw Error('Unknown roster group');
const previous=path.join(out,'evidence.json');
const evidence=selectedGroup&&fs.existsSync(previous)?JSON.parse(fs.readFileSync(previous)).filter(e=>e.group!==selectedGroup):[];
try {
 const manifest=JSON.parse(manifestBytes);manifest.scenarios.push({id:'full-paint-review',title:'Full roster paint review',description:'Temporary representative-pose material review',map:'Classic',playerCount:2,filename:'full-paint-review.json'});
 for(const group of groups.filter(g=>!selectedGroup||g.id===selectedGroup)){
  let {sessionId}=await command('open',{seed:1,viewport});
  const catalog=await command('catalog',{sessionId,categories:['players','units','factions','abilities','commands','upgrades']});
  await command('time',{sessionId,control:{action:'pause'}});
  // Remove baseline seat-owned pieces before switching its faction; neutral scenery remains.
  const initial=await command('inspect',{sessionId,owners:[1],limit:400});
  const existing=initial.entities||initial.inspect?.entities||[];
  const entities=Array.isArray(existing)?existing:(existing.details||[]);
  const remove=entities.filter(e=>e.kind!=='resource_depot'||group.id==='ekat');
  if(remove.length)await command('remove',{sessionId,refs:remove.map(e=>e.id)});
  if(group.id==='ekat')await command('spawn',{sessionId,spawns:[{owner:1,kind:'zamok',factionId:'ekat',x:752,y:304}]});
  const spawns=group.units.map((u,i)=>({owner:1,kind:u.kind,factionId:group.factionId,x:1520+(i%4)*160,y:1616+Math.floor(i/4)*160,alias:`unit${i}`,...(u.daoMaster?{daoMaster:true}:{})}));
  const spawned=await command('spawn',{sessionId,spawns:spawns.filter(s=>s.kind!=='scout_plane'),details:true});
  const ids=spawned.spawned.details.map(e=>e.id);
  if(group.id==='kriegsia'){
   await command('update',{sessionId,updates:[{operation:'research',playerId:1,upgrade:'scout_plane_unlock'}]});
   const car=ids[group.units.findIndex(u=>u.kind==='command_car')];
   await command('update',{sessionId,updates:[{operation:'move',entity:car,x:2000,y:1936}]});
   await command('order',{sessionId,playerId:1,command:{c:'useAbility',ability:'scoutPlane',units:[car],x:2000,y:1936},ignoreCommandLimits:true});
   const planes=await command('inspect',{sessionId,kinds:['scout_plane'],owners:[1],limit:10});
   if(planes.entities.length!==1)throw Error('Expected one Command Car-launched Scout Plane');
   ids.push(planes.entities[0].id);
   await command('update',{sessionId,updates:[{operation:'move',entity:car,x:1680,y:1936}]});
  }
  if(ids.length!==group.units.length)throw Error('Missing review unit ids');
  const base=await command('export',{sessionId,kind:'setup',name:`full-${group.id}-paint-review`});
  const setup=JSON.parse(fs.readFileSync(base.path,'utf8'));
  await command('close',{sessionId});
  fs.writeFileSync(manifestPath,JSON.stringify(manifest,null,2));
  for(const [label,color] of palette){
   const variant=structuredClone(setup),checkpoint=JSON.parse(variant.checkpointPayload);checkpoint.players[0].color=color;variant.checkpointPayload=JSON.stringify(checkpoint);fs.writeFileSync(scenarioPath,JSON.stringify(variant));
   ({sessionId}=await command('open',{scenario:'full-paint-review',viewport}));
   await command('catalog',{sessionId,categories:['players','units']});
   await command('time',{sessionId,control:{action:'pause'}});
   await command('update',{sessionId,updates:ids.flatMap((entity,i)=>group.units[i].kind==='scout_plane'?[]:[{operation:'move',entity,x:spawns[i].x,y:spawns[i].y}])});
   await command('camera',{sessionId,camera:{action:'set',snapshot:camera}});
   const inspected=await command('inspect',{sessionId,refs:ids,limit:400});
   const capture=await command('screenshot',{sessionId,name:`${group.id}-${label.toLowerCase().replaceAll(' ','-')}`,viewport,subjects:ids});
   if(capture.readiness.renderErrors.length||!capture.readiness.ready||!capture.readiness.roomTime.paused)throw Error('Invalid full-roster capture');
   const file=path.join(out,`${group.id}-${label.toLowerCase().replaceAll(' ','-')}.png`);
   if(capture.preview.localPath)fs.copyFileSync(capture.preview.localPath,file);else {const response=await fetch(capture.preview.url);if(!response.ok)throw Error(`Screenshot fetch ${response.status}`);fs.writeFileSync(file,Buffer.from(await response.arrayBuffer()));}
   evidence.push({group:group.id,units:group.units,spawns,ids,color,label,path:file,url:capture.preview.url,readiness:capture.readiness,inspected,catalog});
   fs.writeFileSync(path.join(out,'evidence.json'),JSON.stringify(evidence,null,2));
   await command('close',{sessionId});
   console.log(`${group.id} / ${label}: ${ids.length} units, paused, assets ready, zero render errors`);
  }
  fs.writeFileSync(manifestPath,manifestBytes);fs.rmSync(scenarioPath,{force:true});
 }
}finally{try{await command('shutdown');}finally{fs.writeFileSync(manifestPath,manifestBytes);fs.rmSync(scenarioPath,{force:true});}}
await buildRosterReviewBoards(out,evidence);
