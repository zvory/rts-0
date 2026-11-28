// Review-only packing and reversible renderer wiring. No full animation generation.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';
import puppeteer from 'puppeteer-core';
import {SCOUT_CAR_PNG_RIG_ATLAS as scout} from '../../client/src/renderer/rigs/scout_car_png_atlas.js';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const dir=path.join(root,'client/assets/rigs/roster-team-color-review');
const generated=path.join(dir,'generated');
const write=(file,bytes)=>{if(process.argv.includes('--check')){if(!fs.existsSync(file)||!fs.readFileSync(file).equals(bytes))throw Error(`Stale review output: ${file}`);}else fs.writeFileSync(file,bytes);};
const files=['rifleman_png_strip.js','tank_png_atlas.js','artillery_png_atlas.js','png_routing.js','png_runtime.js'];
const rig=path.join(root,'client/src/renderer/rigs');
const backup=path.join(root,'target/team-color-review-backup');
if(process.argv.includes('--restore')) {
  for(const f of files) if(fs.existsSync(path.join(backup,f))) fs.copyFileSync(path.join(backup,f),path.join(rig,f));
  process.exit(0);
}
const sources={
 rifleman:'rifleman-no-pack-panzerfaust-pass-01/generated/white/recoil-pass-01/rifleman-recoil-review-strip.png',
 tank:'tank-ps1/tank-atlas.png',
 artillery:'artillery-a19-pass-03/generated/artillery-a19-components-pass-03-alpha.png',
};
const browser=await puppeteer.launch({executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
try {
 const page=await browser.newPage();
 for(const name of Object.keys(sources)) {
  const old=fs.readFileSync(path.join(root,'client/assets/rigs',sources[name])).toString('base64');
  const art=fs.readFileSync(path.join(dir,'source',`${name}.png`)).toString('base64');
  const gun=fs.readFileSync(path.join(root,'client',scout.image.split('?')[0])).toString('base64');
  const gunFrame=scout.sprites.find(s=>s.id==='sprite.rearMachineGun').frame;
  const result=await page.evaluate(async({name,old,art,gun,gunFrame})=>{
   const load=async(data)=>{const i=new Image();i.src=`data:image/png;base64,${data}`;await i.decode();return i;};
   const [original,source]=await Promise.all([load(old),load(art)]);
   const canvas=(w,h)=>Object.assign(document.createElement('canvas'),{width:w,height:h});
   const w=name==='rifleman'?160:original.width,h=name==='rifleman'?112:original.height;
   const aligned=canvas(w,h),ctx=aligned.getContext('2d');
   if(name==='rifleman') {
    const rotated=canvas(source.height,source.width),r=rotated.getContext('2d');
    r.translate(0,source.width);r.rotate(-Math.PI/2);r.drawImage(source,0,0);
    const bounds=(c)=>{const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data;let x0=c.width,y0=c.height,x1=0,y1=0;for(let y=0;y<c.height;y++)for(let x=0;x<c.width;x++)if(d[(y*c.width+x)*4+3]>128){x0=Math.min(x0,x);y0=Math.min(y0,y);x1=Math.max(x1,x);y1=Math.max(y1,y);}return [x0,y0,x1-x0+1,y1-y0+1];};
    const target=canvas(w,h);target.getContext('2d').drawImage(original,0,0);
    const from=bounds(rotated),to=bounds(target);ctx.drawImage(rotated,...from,...to);
   } else ctx.drawImage(source,0,0,w,h);
   const target=canvas(w,h);target.getContext('2d').drawImage(original,0,0);
   if(name==='tank') {const g=await load(gun),f=gunFrame;for(const c of [aligned,target])c.getContext('2d').drawImage(g,f.x,f.y,f.w,f.h,700,1100,f.w,f.h);}
   const authored=ctx.getImageData(0,0,w,h);
   const alpha=target.getContext('2d').getImageData(0,0,w,h).data;
   const base=ctx.createImageData(w,h),paint=ctx.createImageData(w,h);
   const smooth=(a,b,v)=>{const t=Math.max(0,Math.min(1,(v-a)/(b-a)));return t*t*(3-2*t);};
   for(let i=0;i<alpha.length;i+=4){
    // Original alpha rejects imagegen background/glow and locks production silhouettes.
    authored.data[i+3]=alpha[i+3];
    const r=authored.data[i]/255,g=authored.data[i+1]/255,b=authored.data[i+2]/255,a=alpha[i+3]/255;
    const chroma=Math.max(0,Math.min(r,b)-g),purity=1-smooth(.35,.75,Math.abs(r-b)/Math.max(.01,Math.max(r,b)-g));
    const mask=purity*smooth(.025,.09,chroma),p=a*mask,ba=p<1?(a-p)/(1-p):0;
    base.data.set([Math.round((r-(r-g)*mask)*255),Math.round(g*255),Math.round((b-(b-g)*mask)*255),Math.round(ba*255)],i);
    const light=Math.round(Math.max(r,g,b)*255);paint.data.set([light,light,light,Math.round(p*255)],i);
   }
   ctx.putImageData(authored,0,0);
   const fixed=canvas(w,h);fixed.getContext('2d').putImageData(base,0,0);
   const mask=canvas(w,h);mask.getContext('2d').putImageData(paint,0,0);
   // Existing Cultivator loader consumes four rows; the first two are neutral/effects.
   const packed=canvas(w,h*4),p=packed.getContext('2d');p.drawImage(aligned,0,0);p.drawImage(fixed,0,h*2);p.drawImage(mask,0,h*3);
   return {author:aligned.toDataURL(),fixed:fixed.toDataURL(),mask:mask.toDataURL(),packed:packed.toDataURL(),w,h};
  },{name,old,art,gun,gunFrame});
  for(const key of ['author','fixed','mask','packed']) {
   const bytes=Buffer.from(result[key].split(',')[1],'base64');
   write(path.join(generated,`${name}-${key}.png`),PNG.sync.write(PNG.sync.read(bytes)));
  }
 }
 // A labeled board uses the actual normalized authoring images and extracted masks.
 const imgs=Object.fromEntries(['rifleman','tank','artillery'].flatMap(n=>['author','mask'].map(k=>[`${n}-${k}`,fs.readFileSync(path.join(generated,`${n}-${k}.png`)).toString('base64')])));
 const board=await page.evaluate(async(imgs)=>{
  const c=Object.assign(document.createElement('canvas'),{width:1500,height:1050}),x=c.getContext('2d');
  x.fillStyle='#20252b';x.fillRect(0,0,c.width,c.height);x.fillStyle='#fff';x.font='bold 30px sans-serif';x.fillText('Selective team paint — initial material review',35,45);
  const rows=[['rifleman','Rifleman','Paint: uniform + helmet','Fixed: skin, leather, wood, gunmetal'],['tank','Tiger tank','Paint: hull panels + turret armor','Fixed: barrel, grilles, fittings, tracks'],['artillery','A-19 artillery','Paint: trails, crossbeams + shield plates','Fixed: tires, axle, pivots, tube + mechanisms']];
  for(let i=0;i<rows.length;i++) {
   const [name,label,paint,fixed]=rows[i],top=90+i*310;
   x.fillStyle='#fff';x.font='bold 25px sans-serif';x.fillText(label,35,top+30);x.font='18px sans-serif';x.fillStyle='#ff75e3';x.fillText(paint,35,top+65);x.fillStyle='#cdd5df';x.fillText(fixed,35,top+95);
   for(const [j,k] of ['author','mask'].entries()) {const image=new Image();image.src=`data:image/png;base64,${imgs[`${name}-${k}`]}`;await image.decode();const scale=Math.min(470/image.width,270/image.height);x.drawImage(image,510+j*490,top+20,image.width*scale,image.height*scale);x.font='16px sans-serif';x.fillStyle='#fff';x.fillText(k==='author'?'Magenta authoring artwork':'Soft grayscale paint mask',510+j*490,top+10);}
  }return c.toDataURL();
 },imgs);
 fs.writeFileSync(path.join(generated,'source-material-board.png'),Buffer.from(board.split(',')[1],'base64'));
} finally {await browser.close();}
if(process.argv.includes('--install-preview')) {
 fs.mkdirSync(backup,{recursive:true});
 for(const f of files) {if(!fs.existsSync(path.join(backup,f)))fs.copyFileSync(path.join(rig,f),path.join(backup,f));}
 const edit=(f,fn)=>fs.writeFileSync(path.join(rig,f),fn(fs.readFileSync(path.join(backup,f),'utf8')));
 edit('rifleman_png_strip.js',s=>s.replace(/image: .*\n/, 'image: "/assets/rigs/roster-team-color-review/generated/rifleman-packed.png",\n  sheetRows: 4,\n  teamPaint: {baseRow:2,paintRow:3},\n').replace('frameCount: 5','frameCount: 1').replace('movementFrames: [1, 2, 3]','movementFrames: [0]').replace('firingFrames: [4]','firingFrames: [0]'));
 for(const name of ['tank','artillery']) edit(`${name}_png_atlas.js`,s=>s.replace(/("?image"?): .*\n/,`image: "/assets/rigs/roster-team-color-review/generated/${name}-packed.png",\n  reviewTeamPaint: true,\n`).replace(/"?tintSlot"?: "team[^"\n]*"/g,'tintSlot: "fixed"').replaceAll('x: 0, y: 1373','x: 700, y: 1100').replace(/"runtimeColorAdjustment": relativeColorAdjustment\([\s\S]*?\),/,'runtimeColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,'));
 edit('png_routing.js',s=>'import { loadTeamPaintTexture } from "./team_paint.js";\n'+s.replace('const base = await loadColorAdjustedTexture(pixi, {','if (atlas.reviewTeamPaint) return loadTeamPaintTexture(pixi, {image:atlas.image, frameWidth:atlas.grid.width, frameHeight:atlas.grid.height, frameCount:1, sheetRows:4, teamPaint:{baseRow:2,paintRow:3}}, atlas.runtimeColorAdjustment);\n  const base = await loadColorAdjustedTexture(pixi, {'));
 edit('png_runtime.js',s=>'import { teamPaintTextureFor } from "./team_paint.js";\n'+s.replace('const atlasTexture = options.atlasTexture;','let atlasTexture = options.atlasTexture;').replace('if (typeof options.alpha', 'if (atlas.reviewTeamPaint) atlasTexture = teamPaintTextureFor(atlasTexture, context.teamColor);\n  if (typeof options.alpha'));
 console.log('Installed temporary idle/component review wiring. Restore with --restore.');
}
