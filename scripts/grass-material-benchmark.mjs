#!/usr/bin/env node
// Isolated full-screen material GPU cost; deliberately not an RTS renderer/frame-rate benchmark.
import http from "node:http";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.resolve(process.argv[2] || path.join(root, "target/grass-material-benchmark.json"));
async function workerMain() {
  try {
    const env = await import("/client/src/renderer/worker_environment.js");
    const { GrassSurveyLayer } = await import("/client/src/renderer/grass_survey.js");
    const { AsyncGpuTimerQueries, runRafIndependentGpuSamples } = await import("/client/src/renderer/gpu_timer_queries.js");
    const canvas = new OffscreenCanvas(1100,720);
    env.installPixiWorkerEnvironment(canvas, { widthCssPx: 1100, heightCssPx: 720 });
    const pixi = await import(env.PIXI_WORKER_URL);
    env.configurePixiForWorker(pixi);
    const app = new pixi.Application();
    await app.init({ canvas, width:1100, height:720, preference:"webgl", antialias:false, autoStart:false });
    const gl = app.renderer.gl;
    const timer = new AsyncGpuTimerQueries(gl);
    if (!timer.supported) throw new Error("Material benchmark requires EXT_disjoint_timer_query_webgl2.");
    const debug = gl.getExtension("WEBGL_debug_renderer_info");
    const result = { methodology:"Isolated grass-only full-screen Pixi worker mesh; 3 rotated rounds, 96 GPU samples each after 16 warmups. No simulation, units, water or frame-rate claim.",
      viewport:[1100,720], gpu:debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
      supported:timer.supported, rows:[] };
    if (timer.supported) {
      const map = { width:64, height:64, tileSize:32, terrain:new Uint8Array(64*64) };
      for (const dpr of [1,2]) {
        app.renderer.resolution = dpr;
        app.renderer.resize(1100,720);
        for (let round=0;round<3;round++) {
          const order=[5,6,7,8];
          for (const technique of [...order.slice(round),...order.slice(0,round)]) {
            const started=performance.now();
            const layer = new GrassSurveyLayer(pixi,app.stage,map,technique,0xfff3e2);
            const bakeMs=performance.now()-started;
            const summary=await runRafIndependentGpuSamples({ timer, label:`grass.${technique}`, count:96, warmup:16,
              sample(index) { layer.update(index*1000/60); app.render(); },
              yieldTask:()=>new Promise(resolve=>setTimeout(resolve,8)) });
            const group=summary.groups[0];
            if (!group || group.samples!==96 || summary.pending || summary.dropped || summary.disjoint) {
              throw new Error(`Incomplete GPU sample set: ${JSON.stringify(summary)}`);
            }
            result.rows.push({ technique,dpr,round,bakeMs,textureBytes:layer.material?.textureBytes || 0,...group });
            layer.destroy();
          }
        }
      }
    }
    timer.destroy();
    app.destroy({ removeView:true },{ children:true });
    postMessage({ ok:true,result });
  } catch(error) { postMessage({ ok:false,error:String(error?.stack || error) }); }
}
const workerSource = `(${workerMain.toString()})()`;
const server = http.createServer(async (req,res) => {
  try {
    const pathname=new URL(req.url,"http://localhost").pathname;
    if (pathname==="/") { res.setHeader("Content-Type","text/html"); res.end("<!doctype html><title>Grass material GPU microbenchmark</title>"); return; }
    if (pathname==="/favicon.ico") { res.writeHead(204).end(); return; }
    if (pathname==="/bench-worker.js") { res.setHeader("Content-Type","text/javascript"); res.end(workerSource); return; }
    if (!pathname.startsWith("/client/src/") || !pathname.endsWith(".js")) { res.writeHead(404).end(); return; }
    const filename=path.resolve(root,`.${pathname}`);
    if (!filename.startsWith(path.join(root,"client/src/"))) { res.writeHead(403).end(); return; }
    res.setHeader("Content-Type","text/javascript"); res.end(await fs.readFile(filename));
  } catch { res.writeHead(404).end(); }
});
let browser;
try {
  await new Promise(resolve=>server.listen(0,"127.0.0.1",resolve));
  browser=await puppeteer.launch({ executablePath:process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", headless:true,
    args:["--no-sandbox"],defaultViewport:{ width:1100,height:720 } });
  const page=await browser.newPage();
  const renderErrors=[];
  page.on("console",message=>{ if(message.type()==="error") renderErrors.push(message.text()); });
  page.on("pageerror",error=>renderErrors.push(String(error)));

  page.setDefaultTimeout(180000);
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const payload=await page.evaluate(()=>new Promise((resolve,reject)=>{
    const worker=new Worker("/bench-worker.js",{ type:"module" });
    const timeout=setTimeout(()=>{ worker.terminate(); reject(new Error("Material benchmark exceeded 180 seconds")); },180000);
    worker.onmessage=event=>{ clearTimeout(timeout); worker.terminate(); resolve(event.data); };
    worker.onerror=event=>{ clearTimeout(timeout); worker.terminate(); reject(new Error(event.message)); };
  }));
  if (!payload.ok) throw new Error(payload.error);
  if (renderErrors.length) throw new Error(`Benchmark render errors: ${renderErrors.join("; ")}`);
  await fs.mkdir(path.dirname(output),{ recursive:true });
  await fs.writeFile(output,JSON.stringify(payload.result,null,2)+"\n");
  console.log(JSON.stringify({ output,...payload.result },null,2));
} finally {
  await browser?.close();
  await new Promise(resolve=>server.close(resolve));
}
