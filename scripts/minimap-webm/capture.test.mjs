import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { WebSocketServer } from 'ws';
import { ReplaySampler } from './sampler.mjs';

test('offline sampling preserves state and never backdates attacks across gaps',()=>{
  const sampler=new ReplaySampler(35,10), rows=[];
  const push=(tick,targets=[])=>sampler.push({tick,
    entities:[{id:1,owner:1,kind:'rifleman',x:tick,y:0,hp:100}],
    events:targets.map(to=>({e:'attack',to})),
  },row=>rows.push(row));
  push(0);push(7,[1]);push(9);push(25,[2]);push(35);
  assert.deepEqual(rows.map(r=>r.tick),[0,10,20,30]);
  assert.deepEqual(rows.map(r=>r.snapshot.entities[0].x),[0,9,9,25]);
  assert.deepEqual(rows.map(r=>[...new Set(r.snapshot.events.map(e=>e.to))]),[[],[1],[],[2]]);
});

test('offline sampling includes attacks at an exact sample tick',()=>{
  const sampler=new ReplaySampler(20,10), rows=[];
  const push=(tick,events=[])=>sampler.push({tick,entities:[],events},row=>rows.push(row));
  push(0);push(7,[{e:'attack',to:1}]);push(20,[{e:'attack',to:1},{e:'attack',to:2}]);
  assert.deepEqual(rows.map(r=>[...new Set(r.snapshot.events.map(e=>e.to))]),[[],[1],[1,2]]);
});

function run(file,args){return new Promise((resolve,reject)=>{
  const child=spawn(process.execPath,[new URL(file,import.meta.url).pathname,...args]);
  let stderr='';child.stderr.on('data',b=>stderr+=b);child.stdout.resume();
  child.on('error',reject);child.on('exit',code=>resolve({code,stderr}));
});}
for(const mode of ['complete','occupied','interrupted'])test(`capture ${mode}`,async()=>{
  const dir=await fs.mkdtemp(path.join(os.tmpdir(),'minimap-capture-test-'));
  const output=path.join(dir,'samples.jsonl');
  const server=http.createServer((req,res)=>{res.setHeader('content-type','application/json');res.end('{"room":"test"}');});
  const sockets=new WebSocketServer({server});
  sockets.on('connection',socket=>{
    const send=m=>socket.send(JSON.stringify(m));
    const snapshot=(tick,events=[])=>send({t:'snapshot',tick,events,visibleTiles:[1],exploredTiles:[1],entities:[{id:1,owner:1,kind:'rifleman',x:tick,y:0,hp:100}]});
    socket.on('message',raw=>{
      const m=JSON.parse(raw);
      if(m.t==='join')send({t:'lobby',players:mode==='occupied'?[{},{}]:[{}],canStart:true});
      if(m.t==='start')send({t:'start',map:{width:1,height:1},players:[],replay:{durationTicks:25,serverBuildSha:'test',mapName:'test'}});
      if(m.t==='seekRoomTimeTo'){send({t:'roomTimeSeekStarted',targetTick:0});snapshot(0);}
      if(m.t==='setRoomTimeSpeed'&&m.speed===8){
        snapshot(7,[{e:'attack',to:1}]);
        if(mode==='interrupted'){socket.close();return;}
        snapshot(8);snapshot(9);snapshot(20);snapshot(25);
      }
    });
  });
  server.listen(0,'127.0.0.1');await once(server,'listening');
  try{
    const result=await run('capture.mjs',[`http://127.0.0.1:${server.address().port}`,'1',output]);
    const rows=(await fs.readFile(output,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
    if(mode==='complete'){
      assert.equal(result.code,0,result.stderr);
      assert.deepEqual(rows.filter(r=>r.tick!==undefined).map(r=>r.tick),[0,10,20]);
      assert.equal(rows[2].snapshot.entities[0].x,9,'gap must hold previous snapshot');
      assert.deepEqual(rows[2].snapshot.events.map(e=>e.to),[1],'preserve attack event at sampled game time');
      assert.equal(rows.at(-1).lastTick,25);
      assert.equal(rows.at(-1).maxSnapshotGap,11);
    }else{
      assert.notEqual(result.code,0);
      assert.ok(!rows.some(r=>r.type==='summary'));
      if(mode==='interrupted'){
        const render=await run('render.mjs',[output,path.join(dir,'invalid.mkv'),'/unused-canvas']);
        assert.match(render.stderr,/capture is incomplete/);
        await assert.rejects(fs.stat(path.join(dir,'invalid.mkv')));
      }
    }
  }finally{for(const socket of sockets.clients)socket.terminate();sockets.close();await new Promise(r=>server.close(r));await fs.rm(dir,{recursive:true});}
});

test('fog follows the sampled state and preserves non-byte-aligned tile grids',()=>{
  const sampler=new ReplaySampler(10,10),rows=[];
  sampler.push({tick:0,entities:[],visibleTiles:[1,0,0,0,0,0,0,0,1],exploredTiles:[1,1,0,0,0,0,0,0,1]},r=>rows.push(r));
  sampler.push({tick:12,entities:[],visibleTiles:[0,1,0,0,0,0,0,0,0],exploredTiles:[1,1,1,0,0,0,0,0,1]},r=>rows.push(r));
  assert.deepEqual([...Buffer.from(rows[1].visibleBits,'base64')],[1,1]);
  assert.deepEqual([...Buffer.from(rows[1].exploredBits,'base64')],[3,1]);
});

test('production samples preserve full entities and inter-frame presentation data',()=>{
  const sampler=new ReplaySampler(20,10),rows=[];
  const entity={id:1,owner:2,kind:'artillery',x:12,y:13,hp:50,maxHp:100,visionOnly:true,facing:1.2};
  sampler.push({tick:0,entities:[]},r=>rows.push(r));
  sampler.push({tick:7,entities:[entity],events:[{e:'notice',msg:'alert:under_attack',x:12,y:13}],resourceDeltas:[{id:5,remaining:100}]},r=>rows.push(r));
  sampler.push({tick:8,entities:[entity],resourceDeltas:[{id:5,remaining:90}]},r=>rows.push(r));
  sampler.push({tick:20,entities:[]},r=>rows.push(r));
  assert.deepEqual(rows[1].snapshot.entities,[entity]);
  assert.deepEqual(rows[1].snapshot.resourceDeltas,[{id:5,remaining:100},{id:5,remaining:90}]);
  assert.equal(rows[1].snapshot.events[0].msg,'alert:under_attack');
  assert.equal(rows[1].timedEvents[0].tick,7);
  assert.deepEqual(rows[2].snapshot.events,[]);
  assert.deepEqual(rows[2].snapshot.resourceDeltas,[]);
});
