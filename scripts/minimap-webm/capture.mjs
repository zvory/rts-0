// Capture a normal spectator replay stream; no database credentials needed.
import fs from 'node:fs';
import { once } from 'node:events';
import WebSocket from 'ws';
import { ReplaySampler } from './sampler.mjs';
import { msg, S, parseServerFrame, decodeServerMessage } from '../../client/src/protocol.js';
const [base, id, output] = process.argv.slice(2);
if (!base || !/^\d+$/.test(id||'') || !output) throw Error('usage: node capture.mjs <server-url> <match-id> <output.jsonl>');
const response = await fetch(`${base}/api/matches/${id}/replay`,{method:'POST',signal:AbortSignal.timeout(30000)});
if (!response.ok) throw Error(`replay launch: ${response.status} ${await response.text()}`);
const {room} = await response.json();
const ws = new WebSocket(`${base.replace(/^http/,'ws')}/ws`);
const out = fs.createWriteStream(output,{flags:'wx'});
let soloLobby=false, seekStarted=false, header, sampler, snapshots=0, lastTick=-1, maxSnapshotGap=0, finished=false, startAt=performance.now();
const step=10;
const send = m => ws.send(JSON.stringify(m));
const write = row => out.write(JSON.stringify(row)+'\n');
const heartbeat = setInterval(()=>{if(ws.readyState===WebSocket.OPEN)send(msg.ping(Date.now()));},10000);
const timeout = setTimeout(()=>fail(Error('capture exceeded 20 minutes')),20*60*1000);
function fail(error){console.error(error);process.exitCode=1;finished=true;clearTimeout(timeout);clearInterval(heartbeat);ws.close();out.end();}
function finish(){
 if(finished)return;finished=true;clearTimeout(timeout);clearInterval(heartbeat);
 write({type:'summary',captureSeconds:(performance.now()-startAt)/1000,snapshots,lastTick,maxSnapshotGap,frames:sampler.nextTick/step});
 console.log(JSON.stringify({id,captureSeconds:(performance.now()-startAt)/1000,lastTick,snapshots,output}));
 ws.close();out.end();
}
ws.on('open',()=>send(msg.join('Minimap export',room,true,true)));
ws.on('message',(data,binary)=>{
 if(finished)return;
 try{
  const m=decodeServerMessage(parseServerFrame(binary?new Uint8Array(data):data.toString()));
  if(m.t===S.LOBBY){
   if(m.players.length!==1)throw Error('replay room has other viewers; refusing to control it');
   soloLobby=true;if(m.canStart)send(msg.start());
  }
  else if(m.t===S.MATCH_COUNTDOWN){send(msg.matchLoadReady(m.countdownId));}
  else if(m.t===S.START){
   if(header) return;
   if(!soloLobby)throw Error('replay already active; refusing to control an existing session');
   header={type:'header',start:m,durationTicks:m.replay.durationTicks,stepTicks:step,tickRate:30,
    recordedBuild:m.replay.serverBuildSha,mapName:m.replay.mapName,source:'spectator-stream-hold-last-sample',fog:'combined',sampleSchema:2};
   sampler=new ReplaySampler(header.durationTicks,step);
   write(header);console.log(JSON.stringify({id,durationTicks:header.durationTicks,build:header.recordedBuild,map:header.mapName}));
   send(msg.setRoomTimeSpeed(0));send(msg.visionSelectionAll());send(msg.seekRoomTimeTo(0));
   // Seek completion is reflected in snapshots; start playback only from tick zero.
  } else if(m.t===S.ROOM_TIME_SEEK_STARTED){
   seekStarted=true;
  } else if(m.t===S.SNAPSHOT && header){
   if(!seekStarted)return;
   const cells=header.start.map.width*header.start.map.height;
   if(m.visibleTiles?.length!==cells || m.exploredTiles?.length!==cells)throw Error('missing authoritative fog grids');
   if(m.tick===0 && lastTick<0)send(msg.setRoomTimeSpeed(8));
   if(m.tick<lastTick)throw Error('replay moved backwards during capture');
   if(lastTick<0 && m.tick>0)return;
   sampler.push(m,write);
   maxSnapshotGap=Math.max(maxSnapshotGap,lastTick<0?0:m.tick-lastTick);lastTick=m.tick;snapshots++;
   if(m.tick>=header.durationTicks)finish();
  } else if(m.t==='error'){throw Error(JSON.stringify(m));}
 }catch(e){fail(e);}
});
ws.on('error',fail);
ws.on('close',()=>{if(!finished)fail(Error(`connection closed at tick ${lastTick}`));});
await once(out,'close');
