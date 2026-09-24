import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { WebSocketServer } from 'ws';

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
    const snapshot=(tick,events=[])=>send({t:'snapshot',tick,events,entities:[{id:1,owner:1,kind:'rifleman',x:tick,y:0,hp:100}]});
    socket.on('message',raw=>{
      const m=JSON.parse(raw);
      if(m.t==='join')send({t:'lobby',players:mode==='occupied'?[{},{}]:[{}],canStart:true});
      if(m.t==='start')send({t:'start',map:{},players:[],replay:{durationTicks:25,serverBuildSha:'test',mapName:'test'}});
      if(m.t==='seekRoomTimeTo')snapshot(0);
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
      assert.equal(rows[2].entities[0][3],9,'gap must hold previous snapshot');
      assert.deepEqual(rows[2].attacks,[1],'preserve attack event at sampled game time');
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
