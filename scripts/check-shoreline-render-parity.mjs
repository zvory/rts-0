#!/usr/bin/env node
// Compare the approved swash material against the optimized Pixi worker material.
import fs from "node:fs";
import { execFileSync } from "node:child_process";
import { SHORELINE_SWASH_FRAGMENT } from "../client/src/renderer/shoreline_swash.js";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import puppeteer from "puppeteer-core";
import { selectDeterministicTicks, compareDecodedRgba, analyzeDecodedRgba, assertCaptureContent } from "./client-render-parity.mjs";

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),"..");
const output=path.join(root,"target/shoreline-parity");
const map=JSON.parse(fs.readFileSync(path.join(root,"server/assets/maps/classic.json")));
const baselineCommit="4c429b3d2"; // Approved study 3, before the optimization/default change.
const referenceSource=execFileSync("git",["show",`${baselineCommit}:client/src/renderer/shoreline_swash.js`],{cwd:root,encoding:"utf8"});
const referenceFragment=referenceSource.match(/export const SHORELINE_SWASH_FRAGMENT = `([\s\S]*)`;/)?.[1];
if(!referenceFragment) throw new Error("Reference swash fragment is unavailable");
const input={referenceFragment,candidateFragment:SHORELINE_SWASH_FRAGMENT,width:map.width,height:map.height,tileSize:32,
 terrain:map.terrain.flatMap(row=>Array.from(row,c=>c==="~"?2:0)),
 ticks:selectDeterministicTicks({frameCount:1800,samples:12,seed:"shoreline-swash-band-v1"})};
const worker=`
import { installPixiWorkerEnvironment, configurePixiForWorker, PIXI_WORKER_URL } from '/src/renderer/worker_environment.js';
import { ShorelineLayer } from '/src/renderer/shoreline.js';
self.onmessage=async ({data:map})=>{
 try {
  const canvas=new OffscreenCanvas(1000,700);
  installPixiWorkerEnvironment(canvas,{widthCssPx:1000,heightCssPx:700});
  const pixi=await import(PIXI_WORKER_URL); configurePixiForWorker(pixi);
  const app=new pixi.Application();
  await app.init({canvas,width:1000,height:700,resolution:1,autoStart:false,preference:'webgl',antialias:false,preserveDrawingBuffer:true,backgroundColor:0x3b472c});
  const layer=new pixi.Container(); app.stage.addChild(layer);
  let shaderSpec;
  const materialPixi={...pixi,Shader:{from(spec){shaderSpec=spec;return pixi.Shader.from(spec);}}};
  const bank=new ShorelineLayer(materialPixi,layer,map,0xfff3e2,3);
  if(!bank.mesh||!shaderSpec.gl.fragment.includes(map.candidateFragment)||map.referenceFragment===map.candidateFragment)
   throw new Error('Parity must compare two distinct active shoreline fragments');
  const optimized=bank.shader;
  const reference=pixi.Shader.from({...shaderSpec,gl:{...shaderSpec.gl,
   fragment:shaderSpec.gl.fragment.replace(map.candidateFragment,map.referenceFragment),name:'shoreline-reference'}});
  const captures=[];
  for(let i=0;i<map.ticks.length;i++) {
   const dpr=i%2+1,zoom=[1.5,.73,1.13][i%3];
   app.renderer.resize(1000,700,dpr);
   layer.scale.set(zoom); layer.position.set(500-2750*zoom,350-1900*zoom);
   bank.update(map.ticks[i]*1000/30);
   const pair=[];
   for(const shader of [reference,optimized]) {
    bank.mesh.shader=shader; app.render();
    const bytes=new Uint8Array(await (await canvas.convertToBlob({type:'image/png'})).arrayBuffer());
    pair.push(bytes);
   }
   captures.push({tick:map.ticks[i],dpr,zoom,pair});
  }
  bank.mesh.shader=optimized; bank.destroy(); reference.destroy();
  app.destroy(false,{children:true});
  self.postMessage({captures},captures.flatMap(c=>c.pair.map(p=>p.buffer)));
 } catch(error) { self.postMessage({error:error.stack||String(error)}); }
};`;
const server=http.createServer((req,res)=>{
 const url=new URL(req.url,"http://localhost");
 if(url.pathname==="/worker.mjs") return res.writeHead(200,{"content-type":"application/javascript"}).end(worker);
 if(url.pathname==="/") return res.writeHead(200,{"content-type":"text/html"}).end("<!doctype html><title>Shoreline parity</title>");
 const file=path.resolve(root,"client",`.${url.pathname}`);
 if(!file.startsWith(path.join(root,"client")+path.sep)||!fs.existsSync(file)) return res.writeHead(404).end();
 res.writeHead(200,{"content-type":file.endsWith(".js")?"application/javascript":"application/octet-stream"});
 fs.createReadStream(file).pipe(res);
});
let browser;
try {
 await new Promise(resolve=>server.listen(0,"127.0.0.1",resolve));
 browser=await puppeteer.launch({executablePath:process.env.CHROME||"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",headless:true,args:["--no-sandbox"]});
 const page=await browser.newPage();
 await page.goto(`http://127.0.0.1:${server.address().port}/`);
 const results=await page.evaluate(async input=>{
  const worker=new Worker('/worker.mjs',{type:'module'});
  try {
   const result=await new Promise((resolve,reject)=>{
    worker.onmessage=event=>resolve(event.data); worker.onerror=event=>reject(new Error(event.message)); worker.postMessage(input);
   });
   if(result.error) throw new Error(result.error);
   for(const capture of result.captures) capture.pair=capture.pair.map(bytes=>{
    let text='';for(let i=0;i<bytes.length;i+=32768) text+=String.fromCharCode(...bytes.subarray(i,i+32768));return btoa(text);
   });
   return result.captures;
  } finally { worker.terminate(); }
 },input);
 fs.mkdirSync(output,{recursive:true});
 const comparisons=results.map(({tick,dpr,zoom,pair})=>{
  const pngs=pair.map((base64,i)=>{
   const bytes=Buffer.from(base64,"base64");fs.writeFileSync(path.join(output,`${tick}-${i?"optimized":"reference"}.png`),bytes);return PNG.sync.read(bytes);
  });
  pngs.forEach((png,i)=>assertCaptureContent(analyzeDecodedRgba(png),`${tick} ${i?"optimized":"reference"}`));
  const diff=compareDecodedRgba(...pngs);
  if(!diff.identical) fs.writeFileSync(path.join(output,`${tick}-diff.png`),PNG.sync.write(diff.diff));
  return {tick,dpr,zoom,changedPixels:diff.changedPixels};
 });
 const summary={baselineCommit,samples:comparisons.length,comparisons,passed:comparisons.every(c=>c.changedPixels===0)};
 fs.writeFileSync(path.join(output,"summary.json"),JSON.stringify(summary,null,2));
 console.log(JSON.stringify(summary));
 if(!summary.passed) process.exitCode=1;
} finally {
 await browser?.close(); await new Promise(resolve=>server.close(resolve));
}
