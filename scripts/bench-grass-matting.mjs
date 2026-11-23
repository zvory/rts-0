import { performance } from 'node:perf_hooks';
import { GrassPressureField } from '../client/src/renderer/grass_matting.js';
import { KIND } from '../client/src/protocol.js';
for (const count of [100,1000]) {
 const f=new GrassPressureField({width:128,height:128,tileSize:32});
 const es=Array.from({length:count},(_,i)=>({id:i,kind:i%4===0?KIND.TANK:KIND.RIFLEMAN,hp:100,x:100+(i%40)*90,y:100+Math.floor(i/40)*90}));
 f.update(es); const times=[];
 for(let frame=0;frame<600;frame++) { for(const e of es) e.x+=2.1; const start=performance.now(); f.update(es); if(frame>=100) times.push(performance.now()-start); }
 times.sort((a,b)=>a-b);
 console.log(JSON.stringify({units:count,medianMs:times[250],p95Ms:times[475],maskBytes:f.pixels.byteLength}));
}
