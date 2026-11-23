import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { PNG } from "pngjs";
import { MediaStore, MAX_MEDIA_BYTES } from "../scripts/interact/media_store.ts";
import { MediaFinisher } from "../scripts/interact/media_finish.ts";
import { validateMediaInput } from "../scripts/interact/media_inputs.ts";
import { createFixedCaptureEncoder } from "../scripts/interact/fixed_capture.ts";
import { runCli } from "../scripts/interact/cli.ts";
import { InteractService } from "../scripts/interact/command_service.ts";
import { presentFixedCaptureResult } from "../scripts/interact/capabilities/media.ts";

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rts-media-contracts-"));
const store = new MediaStore(path.join(tmp,"store"));
const finisher = new MediaFinisher({ store });
const id = `media_${"a".repeat(32)}`;
function tool(command,args,options={}) {
  const result = spawnSync(command,args,{encoding:"utf8",timeout:30000,maxBuffer:4*1024*1024,...options});
  assert.equal(result.status,0,result.stderr?.toString()); return result.stdout;
}
function frame(changed=false) {
  const png = new PNG({width:64,height:64});
  for(let y=0;y<64;y++) for(let x=0;x<64;x++) {
    const o=(y*64+x)*4; png.data[o]=x*3; png.data[o+1]=y*3; png.data[o+2]=40; png.data[o+3]=255;
  }
  if(changed) for(let y=20;y<40;y++) for(let x=20;x<40;x++) {const o=(y*64+x)*4;png.data[o]=255;png.data[o+1]=0;png.data[o+2]=255;}
  return PNG.sync.write(png);
}
const provenance = {
  kind:"interactFixedCapture",mapping:{outputFps:30,simulationHz:30},authoritative:{startTick:0,endTick:29},
  camera:{start:{x:100,y:100,zoom:1},end:{x:100,y:100,zoom:1}},
  capture:{frameCount:30,clip:{x:0,y:0,width:64,height:64},viewport:{width:1000,height:700,deviceScaleFactor:1},visualStartMs:120000},
  frames:Array.from({length:30},(_,i)=>({index:i,tick:i,visualTimeMs:120000+i*1000/30})),
  runtime:{browser:"fixture",platform:process.platform},scene:{identity:{map:"Classic",seed:42,scenario:"blank"}},
};
async function capture(name,changed=false,metadata=provenance,frameCount=30) {
  const dir=path.join(tmp,name);fs.mkdirSync(dir);
  const videoPath=path.join(dir,`${name}.mp4`),source=path.join(dir,"source.mkv"),manifestPath=path.join(dir,"manifest.json");
  const encoder=await createFixedCaptureEncoder({outputPath:videoPath,losslessPath:source,contactSheetPath:path.join(dir,"contact.png"),fps:30,frameCount});
  for(let i=0;i<frameCount;i++) await encoder.write(frame(changed));
  await encoder.finish();
  fs.writeFileSync(manifestPath,JSON.stringify(metadata));
  return {videoPath,source,manifestPath,artifact:store.register(videoPath,manifestPath,source)};
}
try {
  const service=new InteractService();
  await assert.rejects(service.execute("media-inspect",{artifactId:id}),e=>e.code==="localCommandOnly");
  await service.shutdown();
  const help=await runCli(["media","help","compare"],{cwd:os.tmpdir()});
  assert.equal(help.result.command,"compare"); assert.match(help.result.acceptedShape,/difference/);
  for(const args of [["media","--help"],["help","media"],["media","export","--help"],["help","media","export"]]) assert.equal((await runCli(args,{cwd:os.tmpdir()})).ok,true);
  for(const input of [{artifactId:"../escape"},{artifactId:id,speed:0},{artifactId:id,format:"gif"},{artifactId:id,preset:"constructor"},{artifactId:id,path:"/tmp/private"}]) assert.throws(()=>validateMediaInput("media-export",input));
  for(const input of [{artifacts:[id]},{artifacts:[id,id],labels:["a;movie=/etc/passwd","b"]},{artifacts:[id,id,id],layout:"difference"}]) assert.throws(()=>validateMediaInput("media-compare",input));
  assert.throws(()=>store.read(id),e=>e.code==="mediaArtifactMissing");
  assert.throws(()=>store.read("../../secret"),e=>e.code==="invalidInput");
  const a=await capture("baseline"), b=await capture("candidate",true);
  assert.equal(store.register(a.videoPath,a.manifestPath,a.source).artifactId,a.artifact.artifactId,"repeated record-wait registration keeps its ID");
  const decoded=tool("ffmpeg",["-v","error","-i",store.file(a.artifact,true),"-frames:v","1","-f","rawvideo","-pix_fmt","rgba","-"],{encoding:null});
  assert.deepEqual(decoded,PNG.sync.read(frame()).data,"FFV1 retains exact source RGBA pixels before H.264 encoding");
  let publications=0;
  const publishOnly=new MediaFinisher({store, runner:{run(){throw Error("republish must not encode");}},
    publisher:async()=>({url:`http://preview.invalid/${++publications}`,expiresAt:Date.now()+86400000})});
  const first=await publishOnly.execute("media-publish",{artifactId:a.artifact.artifactId});
  const renewed=await publishOnly.execute("media-publish",{artifactId:a.artifact.artifactId});
  assert.equal(first.artifactId,renewed.artifactId);assert.notEqual(first.preview.url,renewed.preview.url);
  const webm=await finisher.execute("media-export",{artifactId:a.artifact.artifactId,preset:"compact",speed:2});
  assert.equal(webm.probe.codec,"vp9");assert.equal(webm.probe.frames,15);assert.ok(Math.abs(webm.probe.duration-.5)<.05);
  const short=await capture("single-frame",false,provenance,1);
  const accelerated=await finisher.execute("media-export",{artifactId:short.artifact.artifactId,speed:8});
  assert.equal(accelerated.probe.frames,1,"even accelerated single-frame captures retain one output frame");
  const shortConcat=await finisher.execute("media-compare",{artifacts:Array(4).fill(short.artifact.artifactId),layout:"concat",speed:8});
  assert.equal(shortConcat.probe.frames,4,"concatenation retains each accelerated single-frame clip");
  const mixedConcat=await finisher.execute("media-compare",{artifacts:[short.artifact.artifactId,b.artifact.artifactId,short.artifact.artifactId,b.artifact.artifactId],layout:"concat",speed:8});
  assert.equal(mixedConcat.probe.frames,10,"concat rounds each accelerated clip independently");
  const mixedPixels=tool("ffmpeg",["-v","error","-i",store.file(store.read(mixedConcat.artifactId)),"-f","rawvideo","-pix_fmt","rgb24","-"],{encoding:null});
  // The changed square is at the center of the letterboxed 320x240 cell below its heading.
  const frameBytes=320*288*3, center=(168*320+160)*3;
  for (const [index,changed] of [[0,false],[1,true],[5,false],[6,true]]) {
    const red=mixedPixels[index*frameBytes+center], green=mixedPixels[index*frameBytes+center+1];
    assert.equal(red>180 && green<60,changed,`concat preserves clip order at frame ${index}`);
  }
  const mp4=await finisher.execute("media-export",{artifactId:a.artifact.artifactId,format:"mp4",preset:"quality"});
  assert.equal(mp4.probe.codec,"h264");
  for(const layout of ["grid","before-after","concat"]) {
    const comparison=await finisher.execute("media-compare",{artifacts:[a.artifact.artifactId,b.artifact.artifactId],layout,labels:["Before", "After"]});
    assert.equal(comparison.probe.codec,"vp9");
    assert.equal(comparison.probe.width,layout==="concat"?320:640);
    assert.equal(comparison.probe.height,288);
    assert.ok(Math.abs(comparison.probe.duration-(layout==="concat"?2:1))<.1);
  }
  const grid4=await finisher.execute("media-compare",{artifacts:[a.artifact.artifactId,b.artifact.artifactId,a.artifact.artifactId,b.artifact.artifactId],labels:["One","Two","Three","Four"]});
  assert.equal(grid4.probe.width,640); assert.equal(grid4.probe.height,576);
  const conflictingVideo=path.join(tmp,"conflicting-compression.mp4");
  fs.copyFileSync(b.videoPath,conflictingVideo);
  const sameRaw=store.register(conflictingVideo,a.manifestPath,a.source);
  const black=await finisher.execute("media-compare",{artifacts:[a.artifact.artifactId,sameRaw.artifactId],layout:"difference",preset:"quality"});
  const pixels=tool("ffmpeg",["-v","error","-i",store.file(store.read(black.artifactId)),"-frames:v","1","-f","rawvideo","-pix_fmt","rgb24","-"],{encoding:null});
  assert.ok(pixels.every(v=>v===0),"identical lossless sources produce a black difference video");
  const diff=await finisher.execute("media-compare",{artifacts:[a.artifact.artifactId,b.artifact.artifactId],layout:"difference",speed:2});
  const changed=tool("ffmpeg",["-v","error","-i",store.file(store.read(diff.artifactId)),"-frames:v","1","-f","rawvideo","-pix_fmt","rgb24","-"],{encoding:null});
  assert.ok(changed.some(v=>v>40),"changed source pixels appear in the difference video");
  await assert.rejects(finisher.execute("media-compare",{artifacts:[a.artifact.artifactId,webm.artifactId],layout:"difference"}),e=>e.code==="losslessSourceRequired");
  const shifted=store.register(b.videoPath,b.manifestPath,b.source);
  const recordPath=path.join(store.root,shifted.artifactId,"artifact.json");const record=JSON.parse(fs.readFileSync(recordPath));
  record.provenance.capture.viewport.deviceScaleFactor=2;fs.writeFileSync(recordPath,JSON.stringify(record));
  await assert.rejects(finisher.execute("media-compare",{artifacts:[a.artifact.artifactId,shifted.artifactId],layout:"difference"}),e=>e.code==="mediaNotSynchronized");
  const publishedCapture=await presentFixedCaptureResult({videoPath:a.videoPath,losslessPath:a.source,manifestPath:a.manifestPath,contactSheetPath:path.join(tmp,"baseline/contact.png"),frameSummary:{count:30}},{publish:async()=>({url:"http://preview.invalid/capture"})});
  assert.match(publishedCapture.artifactId,/^media_[a-f0-9]{32}$/);
  // Remove only this test's globally registered artifact (the service uses the default store).
  const defaultStore=new MediaStore();fs.rmSync(path.join(defaultStore.root,publishedCapture.artifactId),{recursive:true,force:true});
  fs.rmSync(path.join(tmp,"baseline"),{recursive:true,force:true});
  assert.equal((await finisher.execute("media-inspect",{artifactId:a.artifact.artifactId})).probe.frames,30,"artifact survives removal of its originating capture directory");
  const oversized=path.join(tmp,"oversized.mp4");const fd=fs.openSync(oversized,"w");fs.ftruncateSync(fd,MAX_MEDIA_BYTES+1);fs.closeSync(fd);
  assert.throws(()=>store.register(oversized,b.manifestPath),e=>e.code==="mediaTooLarge");
  const failed=new MediaFinisher({store,env:{RTS_INTERACT_FFMPEG:"/missing/ffmpeg"}});
  await assert.rejects(failed.execute("media-export",{artifactId:b.artifact.artifactId}),e=>e.code==="processSpawnFailed");
  console.log("✅ interact_media_contracts: lossless pixels, exports, labeled layouts, synchronized diffs, persistence, input bounds and failure cleanup passed");
} finally {fs.rmSync(tmp,{recursive:true,force:true});}
