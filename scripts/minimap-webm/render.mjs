// Encode the regular client minimap, adding only a clock overlay.
import fs from 'node:fs';
import readline from 'node:readline';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createRegularMinimap } from './regular-minimap.mjs';

const [input, output, canvasPackage, sharpPackage] = process.argv.slice(2);
if (!input || !output || !canvasPackage) throw Error('usage: node render.mjs samples.jsonl master.mkv /absolute/path/to/node_modules/@napi-rs/canvas');
// Refuse interrupted captures before launching an encoder.
const handle = fs.openSync(input, 'r');
const fileSize = fs.fstatSync(handle).size;
const tail = Buffer.alloc(Math.min(fileSize, 65536));
fs.readSync(handle, tail, 0, tail.length, fileSize-tail.length); fs.closeSync(handle);
const summary = JSON.parse(tail.toString('utf8').trim().split('\n').at(-1));
if (summary.type !== 'summary') throw Error('capture is incomplete (missing summary)');
if (!sharpPackage) throw Error('render requires the Sharp package path; use scripts/record-minimap.mjs');
const canvasApi = createRequire(import.meta.url)(canvasPackage);
const size = 480, speed = 15;
const lines = readline.createInterface({ input: fs.createReadStream(input), crlfDelay: Infinity });
let header, canvas, ctx, view, encoder, done, frames = 0;
const started = process.hrtime.bigint();
for await (const line of lines) {
  const row = JSON.parse(line);
  if (row.type === 'summary') continue;
  if (row.type === 'header') {
    header = row;
    if (summary.lastTick < row.durationTicks) throw Error('capture did not reach replay end');
    const sharp = createRequire(import.meta.url)(sharpPackage);
    view = await createRegularMinimap(header, { ...canvasApi,
      rasterizeSvg: bytes => sharp(bytes).png().toBuffer(),
    }, size);
    canvas = view.canvas; ctx = canvas.getContext('2d');
    encoder = spawn('ffmpeg', ['-hide_banner','-loglevel','error','-n','-f','rawvideo','-pixel_format','rgba',
      '-video_size',`${size}x${size}`,'-framerate',String(row.tickRate*speed/row.stepTicks),'-i','pipe:0',
      '-an','-c:v','ffv1','-level','3',output], {stdio:['pipe','inherit','inherit']});
    done = new Promise((resolve,reject) => {encoder.on('error',reject);encoder.on('close',code=>code===0?resolve():reject(Error(`ffmpeg exited ${code}`)));});
    // Surface encoder errors while streaming as well as at completion.
    done.catch(() => {});
    continue;
  }
  if (!header) throw Error('missing header');
  await view.prepare(row);
  view.render(row);
  const seconds=Math.floor(row.tick/header.tickRate);
  const stamp=`${String(Math.floor(seconds/60)).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}`;
  ctx.fillStyle='#10151dee';ctx.fillRect(195,8,90,34);
  ctx.fillStyle='#ffffff';ctx.font='bold 22px monospace';ctx.fillText(stamp,207,32);
  const bytes=ctx.getImageData(0,0,size,size).data;
  if (!encoder.stdin.write(bytes)) await once(encoder.stdin,'drain');
  if (frames === 0 || (row.tick >= header.durationTicks/2 && row.tick < header.durationTicks/2+header.stepTicks)) {
    fs.writeFileSync(`${output}.${frames===0?'start':'middle'}.png`,canvas.toBuffer('image/png'));
  }
  frames++;
}
if (!encoder) throw Error('no frames');
encoder.stdin.end(); await done;
view.destroy();
console.log(JSON.stringify({frames,renderSeconds:Number(process.hrtime.bigint()-started)/1e9,output}));
