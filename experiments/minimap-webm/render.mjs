// Offline POC renderer: shared terrain/forest/road code, simplified entity/attack overlay.
import fs from 'node:fs';
import readline from 'node:readline';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { minimapTerrainColor, paintMinimapRoadMarkings } from '../../client/src/minimap_terrain.js';
import { MinimapForestLayer } from '../../client/src/minimap_forest_layer.js';
import { COLORS, STATS } from '../../client/src/config.js';
import { isBuilding, isResource } from '../../client/src/protocol.js';

const [input, output, canvasPackage] = process.argv.slice(2);
if (!input || !output || !canvasPackage) throw Error('usage: node render.mjs samples.jsonl master.mkv /absolute/path/to/node_modules/@napi-rs/canvas');
// Refuse interrupted captures before launching an encoder.
const handle = fs.openSync(input, 'r');
const fileSize = fs.fstatSync(handle).size;
const tail = Buffer.alloc(Math.min(fileSize, 65536));
fs.readSync(handle, tail, 0, tail.length, fileSize-tail.length); fs.closeSync(handle);
const summary = JSON.parse(tail.toString('utf8').trim().split('\n').at(-1));
if (summary.type !== 'summary') throw Error('capture is incomplete (missing summary)');
const { createCanvas } = createRequire(import.meta.url)(canvasPackage);
const size = 480, speed = 10;
const hex = n => `#${n.toString(16).padStart(6, '0')}`;
const lines = readline.createInterface({ input: fs.createReadStream(input), crlfDelay: Infinity });
let header, canvas, ctx, background, encoder, done, frames = 0;
let scale, offX, offY, players;
const attacks = new Map();
let previousHealth = new Map();
const point = (x, y) => [offX + x * scale, offY + y * scale];
function dot(context, x, y, radius, fill, stroke) {
  context.beginPath(); context.arc(x, y, radius, 0, 2 * Math.PI);
  context.fillStyle = fill; context.fill();
  if (stroke) { context.strokeStyle = stroke; context.lineWidth = 1; context.stroke(); }
}
const started = performance.now();
for await (const line of lines) {
  const row = JSON.parse(line);
  if (row.type === 'summary') continue;
  if (row.type === 'header') {
    header = row;
    if (summary.lastTick < row.durationTicks) throw Error('capture did not reach replay end');
    const map = row.start.map;
    players = new Map(row.start.players.map(p => [p.id, p]));
    canvas = createCanvas(size, size); ctx = canvas.getContext('2d');
    background = createCanvas(size, size); const bg = background.getContext('2d');
    bg.fillStyle = hex(COLORS.bgVoid); bg.fillRect(0, 0, size, size);
    scale = size / (Math.max(map.width, map.height) * map.tileSize);
    offX = (size - map.width * map.tileSize * scale) / 2;
    offY = (size - map.height * map.tileSize * scale) / 2;
    const cell = map.tileSize * scale;
    for (let y = 0; y < map.height; y++) for (let x = 0; x < map.width; x++) {
      bg.fillStyle = hex(minimapTerrainColor(map.terrain[y * map.width + x], x, y));
      bg.fillRect(offX + x * cell, offY + y * cell, cell + 0.5, cell + 0.5);
    }
    new MinimapForestLayer().draw({ctx:bg,map,size,scale,offX,offY,presentationScale:size/220});
    paintMinimapRoadMarkings(bg, map, scale, (x,y) => {const p=point(x,y); return {x:p[0],y:p[1]};});
    for (const r of map.resources) {
      const [x,y] = point(r.x,r.y);
      dot(bg,x,y,2.5,hex(r.kind === 'oil' ? COLORS.oil : COLORS.steel));
    }
    encoder = spawn('ffmpeg', ['-hide_banner','-loglevel','error','-n','-f','rawvideo','-pixel_format','rgba',
      '-video_size',`${size}x${size}`,'-framerate',String(row.tickRate*speed/row.stepTicks),'-i','pipe:0',
      '-an','-c:v','ffv1','-level','3',output], {stdio:['pipe','inherit','inherit']});
    done = new Promise((resolve,reject) => {encoder.on('error',reject);encoder.on('close',code=>code===0?resolve():reject(Error(`ffmpeg exited ${code}`)));});
    // Surface encoder errors while streaming as well as at completion.
    done.catch(() => {});
    continue;
  }
  if (!header) throw Error('missing header');
  ctx.drawImage(background,0,0);
  for (const id of row.attacks) attacks.set(id,row.tick+30);
  const health = new Map();
  for (const [id,owner,kind,wx,wy,hp] of row.entities) {
    health.set(id,hp);
    if (previousHealth.has(id) && hp < previousHealth.get(id)) attacks.set(id,row.tick+30);
    if (!hp || isResource(kind)) continue;
    const [x,y] = point(wx,wy);
    const color = players.get(owner)?.color || '#999999';
    const building = isBuilding(kind);
    const radius = building ? 3.8 : 2 + Math.min(2, (STATS[kind]?.supply || 1)/4);
    if (building) {
      ctx.fillStyle=color;ctx.fillRect(x-radius,y-radius,radius*2,radius*2);
      ctx.strokeStyle='#ffffffbb';ctx.lineWidth=0.7;ctx.strokeRect(x-radius,y-radius,radius*2,radius*2);
    } else dot(ctx,x,y,radius,color,'#ffffff99');
    if ((attacks.get(id)||0)>row.tick) {
      ctx.beginPath();ctx.arc(x,y,radius+4,0,2*Math.PI);ctx.strokeStyle='#ffeb80';ctx.lineWidth=1.5;ctx.stroke();
    }
  }
  previousHealth = health;
  for (const [id,until] of attacks) if (until<=row.tick) attacks.delete(id);
  const seconds=Math.floor(row.tick/header.tickRate);
  const stamp=`${String(Math.floor(seconds/60)).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}  /  10×`;
  ctx.fillStyle='#10151dee';ctx.fillRect(163,8,154,34);
  ctx.fillStyle='#ffffff';ctx.font='bold 22px monospace';ctx.fillText(stamp,171,32);
  const bytes=ctx.getImageData(0,0,size,size).data;
  if (!encoder.stdin.write(bytes)) await once(encoder.stdin,'drain');
  if (frames === 0 || (row.tick >= header.durationTicks/2 && row.tick < header.durationTicks/2+header.stepTicks)) {
    fs.writeFileSync(`${output}.${frames===0?'start':'middle'}.png`,canvas.toBuffer('image/png'));
  }
  frames++;
}
if (!encoder) throw Error('no frames');
encoder.stdin.end(); await done;
console.log(JSON.stringify({frames,renderSeconds:(performance.now()-started)/1000,output}));
