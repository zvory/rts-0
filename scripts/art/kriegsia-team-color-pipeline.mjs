#!/usr/bin/env node
// Pack approved material boundaries through every original frame and modular component.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {PNG} from 'pngjs';
import puppeteer from 'puppeteer-core';
import {composeTeamPaintRgba} from '../../client/src/renderer/rigs/team_paint.js';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const directory=path.join(root,'client/assets/rigs/kriegsia-team-color');
const specs=JSON.parse(fs.readFileSync(path.join(directory,'specs.json')));
const check=process.argv.includes('--check');
const write=(file,png)=>{const bytes=PNG.sync.write(png);if(check){if(!fs.existsSync(file)||!fs.readFileSync(file).equals(bytes))throw Error(`Stale Kriegsia team paint: ${file}`);}else fs.writeFileSync(file,bytes);};
const encode=file=>fs.readFileSync(path.join(root,file)).toString('base64');
const browser=await puppeteer.launch({executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
try{
 const page=await browser.newPage();
 for(const spec of specs){
  const original=PNG.sync.read(fs.readFileSync(path.join(root,spec.original)));
  const authored=spec.idle?await page.evaluate(async({spec,poses,idle,original})=>{
   const load=async(data)=>{const i=new Image();i.src='data:image/png;base64,'+data;await i.decode();return i;};
   const [sheet,approved,old]=await Promise.all([load(poses),load(idle),load(original)]);
   const canvas=(w,h)=>Object.assign(document.createElement('canvas'),{width:w,height:h});
   const bounds=c=>{const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data;let x0=c.width,y0=c.height,x1=-1,y1=-1;for(let y=0;y<c.height;y++)for(let x=0;x<c.width;x++)if(d[(y*c.width+x)*4+3]>96){x0=Math.min(x0,x);y0=Math.min(y0,y);x1=Math.max(x1,x);y1=Math.max(y1,y);}if(x1<x0)throw Error('Empty authored pose');return [x0,y0,x1-x0+1,y1-y0+1];};
   const result=canvas(spec.width*spec.count,spec.height),ctx=result.getContext('2d');
   for(let n=0;n<spec.count;n++){
    if(n===0){ctx.drawImage(approved,0,0);continue;}
    const sw=sheet.width/spec.columns,sh=sheet.height/spec.rows,cell=canvas(Math.round(sw),Math.round(sh));
    cell.getContext('2d').drawImage(sheet,n%spec.columns*sw,Math.floor(n/spec.columns)*sh,sw,sh,0,0,cell.width,cell.height);
    const target=canvas(spec.width,spec.height);target.getContext('2d').drawImage(old,-n*spec.width,0);
    const to=bounds(target);to[0]+=n*spec.width;ctx.drawImage(cell,...bounds(cell),...to);
   }
   return result.toDataURL();
  },{spec,poses:encode(spec.author),idle:encode(spec.idle),original:encode(spec.original)}):null;
  let author=authored?PNG.sync.read(Buffer.from(authored.split(',')[1],'base64')):PNG.sync.read(fs.readFileSync(path.join(root,spec.author)));
  let width=spec.width*spec.count,height=spec.height;
  // Keep the existing tank roof-gun frame coordinates/anchors; pack its fixed source below the body.
  if(spec.name==='tank'){
   const old=author; height=1538;author=new PNG({width,height});PNG.bitblt(old,author,0,0,old.width,old.height,0,0);
   const gun=PNG.sync.read(fs.readFileSync(path.join(root,'client/assets/rigs/scout-car-white-pass-01/generated/scout-car-white-atlas.png')));
   PNG.bitblt(gun,author,59,339,538,165,0,1373);
  }
  const fixed=new PNG({width,height}),paint=new PNG({width,height});
  const smooth=(a,b,v)=>{const t=Math.max(0,Math.min(1,(v-a)/(b-a)));return t*t*(3-2*t);};
  for(let y=0;y<height;y++)for(let x=0;x<width;x++){
   const i=(y*width+x)*4,oldIndex=(y*original.width+x)*4;
   const alpha=y<original.height?original.data[oldIndex+3]:author.data[i+3];
   author.data[i+3]=alpha;
   // The original tank's attachment used this same fixed gun source at these same coordinates.
   const roof=spec.name==='tank'&&y>=1373;
   const frame=Math.floor(x/spec.width),localX=x%spec.width;
   const flash=['rifleman','panzerfaust'].includes(spec.name)&&frame===4&&localX>=149;
   if(roof||flash){
    const source=flash?original.data:author.data,index=flash?oldIndex:i;
    fixed.data.set(source.subarray(index,index+4),i);if(flash)author.data.set(source.subarray(index,index+4),i);continue;
   }
   const r=author.data[i]/255,g=author.data[i+1]/255,b=author.data[i+2]/255,a=alpha/255;
   const chroma=Math.max(0,Math.min(r,b)-g),purity=1-smooth(.35,.75,Math.abs(r-b)/Math.max(.01,Math.max(r,b)-g));
   const mask=purity*smooth(.025,.09,chroma),p=a*mask,baseAlpha=p<1?(a-p)/(1-p):0;
   fixed.data.set([Math.round((r-(r-g)*mask)*255),Math.round(g*255),Math.round((b-(b-g)*mask)*255),Math.round(baseAlpha*255)],i);
   const light=Math.round(Math.max(r,g,b)*255);paint.data.set([light,light,light,Math.round(p*255)],i);
  }
  const packed=new PNG({width,height:height*4});
  packed.data.set(composeTeamPaintRgba(fixed.data,paint.data,0xffffff));
  PNG.bitblt(fixed,packed,0,0,width,height,0,height*2);PNG.bitblt(paint,packed,0,0,width,height,0,height*3);
  for(const [suffix,png]of [['author',author],['fixed',fixed],['paint',paint],['atlas',packed]])write(path.join(directory,'generated',`${spec.name}-${suffix}.png`),png);
  console.log(`${check?'Verified':'Packed'} ${spec.name}: ${spec.count} frames/components, ${width}x${height}`);
 }
}finally{await browser.close();}
