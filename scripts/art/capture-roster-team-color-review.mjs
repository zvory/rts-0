// Capture the production selective-color material treatment in an authoritative Lab.
import fs from 'node:fs';
import path from 'node:path';
import {runCli} from '../interact/cli.ts';
import {runtimePaths} from '../interact/runtime.ts';
import puppeteer from 'puppeteer-core';
const root=path.resolve(import.meta.dirname,'../..');process.chdir(root);
const command=async(name,input)=>{const r=await runCli(['lab',name,JSON.stringify(input)]);if(!r.ok)throw Error(JSON.stringify(r.error));return r.result;};
const out=path.join(root,'target/team-color-review-captures');fs.mkdirSync(out,{recursive:true});
const palette=[['Blue','#0072b2'],['Vermilion','#d55e00'],['Green','#009e73'],['Pink','#cc79a7'],['Sky blue','#56b4e9'],['Orange','#e69f00'],['Yellow','#f0e442']];
let sessionId;
const captures=[];
const scenarioDir=path.join(root,'server/assets/lab-scenarios');
const manifestPath=path.join(scenarioDir,'manifest.json'),manifestBytes=fs.readFileSync(manifestPath);
const scenarioPath=path.join(scenarioDir,'team-paint-review.json');
if(fs.existsSync(scenarioPath))throw Error('Temporary review scenario already exists');
const runtime=runtimePaths(root);
if([runtime.state,runtime.socket,runtime.lock].some(file=>fs.existsSync(file)))throw Error('Capture requires an unused Interact runtime so cleanup cannot stop another session');
try {
 ({sessionId}=await command('open',{seed:1,viewport:{width:480,height:280,deviceScaleFactor:2}}));
 await command('catalog',{sessionId,categories:['players','units']});
 await command('time',{sessionId,control:{action:'pause'}});
 const spawned=await command('spawn',{sessionId,spawns:[['rifleman',1888,'infantry'],['tank',1992,'vehicle'],['artillery',2096,'gun']].map(([kind,x,alias])=>({owner:1,kind,x,y:1824,alias}))});
 await command('camera',{sessionId,camera:{action:'set',snapshot:{version:1,focus:{x:1992,y:1824},framingScale:1,boundsPolicy:'mapOverscroll'}}});
 const subjectIds=spawned.spawned.details.map(row=>row.id);
 const base=await command('export',{sessionId,kind:'setup',name:'selective-paint-review'});
 const original=JSON.parse(fs.readFileSync(base.path,'utf8'));
 const manifest=JSON.parse(manifestBytes);manifest.scenarios.push({id:'team-paint-review',title:'Team paint material review',description:'Temporary paused material review',map:original.map.name,playerCount:2,filename:'team-paint-review.json'});
 fs.writeFileSync(manifestPath,JSON.stringify(manifest,null,2));
 await command('close',{sessionId});
 for(const [label,color] of palette){
  const setup=structuredClone(original),checkpoint=JSON.parse(setup.checkpointPayload);checkpoint.players[0].color=color;setup.checkpointPayload=JSON.stringify(checkpoint);
  fs.writeFileSync(scenarioPath,JSON.stringify(setup));
  ({sessionId}=await command('open',{scenario:'team-paint-review',viewport:{width:480,height:280,deviceScaleFactor:2}}));
  await command('catalog',{sessionId,categories:['players','units']});
  await command('time',{sessionId,control:{action:'pause'}});
  await command('camera',{sessionId,camera:{action:'set',snapshot:{version:1,focus:{x:1992,y:1824},framingScale:1,boundsPolicy:'mapOverscroll'}}});
  const capture=await command('screenshot',{sessionId,name:`selective-${label.toLowerCase().replaceAll(' ','-')}`,viewport:{width:480,height:280,deviceScaleFactor:2},subjects:subjectIds});
  if(capture.readiness.renderErrors.length||!capture.readiness.ready||!capture.readiness.roomTime.paused)throw Error(`Invalid selective-color capture: ${label}`);
  const destination=path.join(out,`${label.toLowerCase().replaceAll(' ','-')}.png`);
  if(capture.preview.localPath)fs.copyFileSync(capture.preview.localPath,destination);else {const r=await fetch(capture.preview.url);if(!r.ok)throw Error(`Capture download ${r.status}`);fs.writeFileSync(destination,Buffer.from(await r.arrayBuffer()));}
  captures.push({label,color,path:destination,url:capture.preview.url,tick:capture.readiness.snapshotTick,camera:capture.readiness.camera,roomTime:capture.readiness.roomTime,renderErrors:capture.readiness.renderErrors,subjects:capture.readiness.subjects});
  await command('close',{sessionId});
  console.log(`${label}: paused tick ${capture.readiness.snapshotTick}, assets ready, ${capture.readiness.renderErrors.length} render errors`);
 }
} finally {try {await command('shutdown',{});}finally{fs.writeFileSync(manifestPath,manifestBytes);fs.rmSync(scenarioPath,{force:true});}}
fs.writeFileSync(path.join(out,'evidence.json'),JSON.stringify(captures,null,2));
const browser=await puppeteer.launch({executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
try {
 const page=await browser.newPage();const encoded=captures.map(c=>({...c,data:fs.readFileSync(c.path).toString('base64')}));
 const board=await page.evaluate(async(captures)=>{
  const c=Object.assign(document.createElement('canvas'),{width:1920,height:1400}),x=c.getContext('2d');x.fillStyle='#20252b';x.fillRect(0,0,c.width,c.height);
  x.fillStyle='#fff';x.font='bold 34px sans-serif';x.fillText('Paused in-game renders — seven player colors',30,45);x.font='22px sans-serif';x.fillText('Rifleman  •  Tiger tank  •  A-19 artillery    |    Camera scale 1, DPR 2, normal lighting',30,80);
  for(let i=0;i<captures.length;i++) {const a=captures[i],col=i%2,row=Math.floor(i/2),left=col*960,top=105+row*320;const image=new Image();image.src=`data:image/png;base64,${a.data}`;await image.decode();
   // Crop empty top/bottom terrain only; copy game pixels 1:1 without resizing.
   x.drawImage(image,0,150,960,270,left,top+38,960,270);x.fillStyle='#fff';x.font='bold 25px sans-serif';x.fillText(`${a.label}  ${a.color}`,left+25,top+28);
  }
  x.fillStyle='#cdd5df';x.font='22px sans-serif';x.fillText('Each panel is a capture of the same paused scene.',990,1130);x.fillText('Shared artwork + soft masks; cached runtime palettes.',990,1170);x.fillText('No per-color AI artwork. Idle/component review only.',990,1210);return c.toDataURL();
 },encoded);
 fs.writeFileSync(path.join(out,'seven-color-game-renders.png'),Buffer.from(board.split(',')[1],'base64'));
} finally {await browser.close();}
console.log(path.join(out,'seven-color-game-renders.png'));
