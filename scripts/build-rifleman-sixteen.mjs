import fs from 'node:fs';
import {PNG} from 'pngjs';
import {composeTeamPaintRgba} from '../client/src/renderer/rigs/team_paint.js';

const root=new URL('../client/assets/rigs/rifleman-experimental-v3/',import.meta.url);
const approved=new URL('sources/',root);
fs.mkdirSync(root,{recursive:true});
const read=name=>PNG.sync.read(fs.readFileSync(new URL(name,root)));
const headings=['south','ssw','sw','wsw','west','wnw','nw','nnw','north','nne','ne','ene','east','ese','se','sse'];
const approvedRead=name=>PNG.sync.read(fs.readFileSync(new URL(name,approved)));
const sources={south:approvedRead('south-source-v3.png'),north:approvedRead('north-source.png'),east:approvedRead('east-source.png'),westCarry:approvedRead('west-carry-source.png'),westAim:approvedRead('west-aim-source.png')};
const sourceManifest=JSON.parse(fs.readFileSync(new URL('prompts.json',root)));
for(const heading of headings)if(!sources[heading]&&heading!=='west')sources[heading]=read(sourceManifest.find(s=>s.heading===heading).file);
const sizeReference=JSON.parse(fs.readFileSync(new URL('scale-reference.json',root)));
const size=128,columns=16,width=2048,height=2048,frameCount=256;
const fixed=new PNG({width,height}),paint=new PNG({width,height}),coverage=new PNG({width,height});
const registrations=[],families=[];
const smooth=(lo,hi,v)=>{const t=Math.max(0,Math.min(1,(v-lo)/(hi-lo)));return t*t*(3-2*t);};
function materialMask(r,g,b){
 const chroma=Math.max(0,Math.min(r,b)-g);
 return (1-smooth(.35,.75,Math.abs(r-b)/Math.max(.01,Math.max(r,b)-g)))*smooth(.025,.09,chroma);
}
function bounds(points,w){
 let x0=w,y0=Infinity,x1=-1,y1=-1;
 for(const p of points){const x=p%w,y=Math.floor(p/w);x0=Math.min(x0,x);y0=Math.min(y0,y);x1=Math.max(x1,x);y1=Math.max(y1,y);}
 if(x1<x0)throw Error('Empty sprite');return {x:x0,y:y0,w:x1-x0+1,h:y1-y0+1,bottom:y1};
}
function beltX(points,w,rgba,b){
 const xs=points.filter(p=>{const y=Math.floor(p/w),i=p*4,[r,g,blue]=rgba.subarray(i,i+3);return y>b.y+b.h*.45&&y<b.y+b.h*.62&&r>g*1.12&&g>blue*1.12;}).map(p=>p%w).sort((a,b)=>a-b);
 if(xs.length<10)throw Error('Missing belt landmark');return xs[Math.floor(xs.length*.5)];
}
function footContact(points,w,rgba,b){
 const boots=points.filter(p=>{const y=Math.floor(p/w),i=p*4,[r,g,blue]=rgba.subarray(i,i+3);return y>b.y+b.h*.65&&Math.max(r,g,blue)<170&&r>=g*.95&&g>=blue*.9;});
 if(boots.length<30)throw Error('Missing boots');
 let left=Math.min(...boots.map(p=>p%w)),right=Math.max(...boots.map(p=>p%w));
 for(let pass=0;pass<8;pass++){
  const groups=[[],[]];for(const p of boots)groups[Math.abs(p%w-left)<=Math.abs(p%w-right)?0:1].push(p%w);
  if(groups.some(g=>!g.length))break;
  left=groups[0].reduce((a,b)=>a+b,0)/groups[0].length;right=groups[1].reduce((a,b)=>a+b,0)/groups[1].length;
 }
 const contacts=[left,right].map((center,k)=>{
  const other=k?left:right,group=boots.filter(p=>Math.abs(p%w-center)<=Math.abs(p%w-other));
  const bottom=Math.max(...group.map(p=>Math.floor(p/w)));
  const sole=group.filter(p=>Math.floor(p/w)>=bottom-Math.max(2,b.h*.008));
  return {x:sole.reduce((n,p)=>n+p%w,0)/sole.length,y:bottom};
 });
 return {x:(contacts[0].x+contacts[1].x)/2,y:(contacts[0].y+contacts[1].y)/2,contacts};
}
for(let frame=0;frame<frameCount;frame++){

 const direction=Math.floor(frame/16),pose=frame%16,heading=headings[direction];
 const src=heading==='west'?(pose<8?sources.westCarry:sources.westAim):sources[heading];
 const rows=heading==='west'?2:4,cell=heading==='west'?pose%8:pose;
 const left=Math.round(cell%4*src.width/4),top=Math.round(Math.floor(cell/4)*src.height/rows);
 const w=Math.round((cell%4+1)*src.width/4)-left,h=Math.round((Math.floor(cell/4)+1)*src.height/rows)-top;
 const rgba=new Uint8Array(w*h*4);
 for(let y=0;y<h;y++)rgba.set(src.data.subarray(((top+y)*src.width+left)*4,((top+y)*src.width+left+w)*4),y*w*4);
 const valid=(p,threshold)=>{
  const i=p*4,[r,g,b,a]=rgba.subarray(i,i+4);
  return a>=threshold && !(r>180&&g<100&&b<110) && !(r>200&&g>180&&b<90);
 };
 const neighbours=p=>{const x=p%w,y=Math.floor(p/w);return [x? p-1:-1,x+1<w?p+1:-1,y?p-w:-1,y+1<h?p+w:-1].filter(n=>n>=0);};
 const seen=new Uint8Array(w*h);let core=[];
 for(let p=0;p<seen.length;p++){
  if(seen[p]||!valid(p,64))continue;
  const component=[p];seen[p]=1;
  for(let k=0;k<component.length;k++)for(const n of neighbours(component[k])){
   if(seen[n]||!valid(n,64))continue;seen[n]=1;component.push(n);
  }
  if(component.length>core.length)core=component;
 }
 if(core.length<1000)throw Error('Missing sprite '+frame);
 // Keep the connected soft-alpha boundary, rather than turning the alpha threshold into a cutout.
 const body=[...core],mask=new Uint8Array(w*h);for(const p of body)mask[p]=1;
 for(let k=0;k<body.length;k++)for(const n of neighbours(body[k])){
  if(mask[n]||!valid(n,8))continue;mask[n]=1;body.push(n);
 }
 // Find nearby reliable interior colors for conservative white-matte decontamination.
 const nearest=new Int32Array(w*h).fill(-1),distance=new Uint8Array(w*h).fill(255),queue=[];
 for(const p of core)if(rgba[p*4+3]>=240 && neighbours(p).every(n=>mask[n]&&rgba[n*4+3]>=224)){
  nearest[p]=p;distance[p]=0;queue.push(p);
 }
 for(let k=0;k<queue.length;k++){
  const p=queue[k];if(distance[p]>=3)continue;
  for(const n of neighbours(p))if(mask[n]&&distance[n]>distance[p]+1){nearest[n]=nearest[p];distance[n]=distance[p]+1;queue.push(n);}
 }
 let decontaminated=0;
 for(const p of body){
  const i=p*4,n=nearest[p];if(rgba[i+3]>=224||n<0||distance[p]===0)continue;
  const [r,g,b]=rgba.subarray(i,i+3),j=n*4;
  if(Math.min(r,g,b)>100 && Math.max(r,g,b)-Math.min(r,g,b)<45 && materialMask(r/255,g/255,b/255)<.2
    && materialMask(rgba[j]/255,rgba[j+1]/255,rgba[j+2]/255)>.8){
   rgba.set(rgba.subarray(j,j+3),i);decontaminated++;
  }
 }
 const from=bounds(core,w),bx=frame%columns*size,by=Math.floor(frame/columns)*size;
 const feet=footContact(core,w,rgba,from);
 if(pose===0){
  const helmet=core.filter(p=>{const y=Math.floor(p/w),i=p*4;return y<from.y+from.h*.20 && rgba[i+3]>128 && materialMask(rgba[i]/255,rgba[i+1]/255,rgba[i+2]/255)>.7;});
  if(!helmet.length)throw Error('Missing helmet size landmark '+heading);
  const helmetWidth=Math.max(...helmet.map(p=>p%w))-Math.min(...helmet.map(p=>p%w))+1;
  // Preserve approved cardinal scales. New bearings target the same standing
  // height, bounded by the approved helmet-size range to avoid oversized heads.
  const cardinal=['south','west','north','east'].includes(heading);
  const scale=cardinal?(heading==='south'?sizeReference.southTargetHelmetWidth/helmetWidth:88/from.h)
    :Math.max(21/helmetWidth,Math.min(24/helmetWidth,88/from.h));
  families[direction]={scale,feet,belt:beltX(core,w,rgba,from),helmetWidth,renderedHelmetWidth:helmetWidth*scale};
 }
 const family=families[direction],scale=family.scale;
 // One ground-plane point: midpoint of the standing sole contacts. Gait frames
 // carry the idle ground reference and register at the belt, not a raised boot.
 const running=pose>=2&&pose<=7;
 const sourceX=running?beltX(core,w,rgba,from):feet.x;
 const ox=running?64+(family.belt-family.feet.x)*scale-sourceX*scale:64-sourceX*scale;
 const oy=112-(running?family.feet.y:feet.y)*scale;
 // Split material contributions at source resolution BEFORE filtering. This avoids blending
 // magenta with skin/steel and then misclassifying the mixed edge as a fixed pale material.
 const layers=new Float32Array(w*h*6);
 for(const p of body){
  const i=p*4,j=p*6,r=rgba[i]/255,g=rgba[i+1]/255,b=rgba[i+2]/255,a=rgba[i+3]/255;
  const m=materialMask(r,g,b),pa=a*m,fa=a-pa;
  layers.set([(r-(r-g)*m)*fa,g*fa,(b-(b-g)*m)*fa,fa,Math.max(r,g,b)*pa,pa],j);
 }
 let clipped=0;
 for(const p of body){const x=p%w,y=Math.floor(p/w);if(x*scale+ox<0||x*scale+ox>=size||y*scale+oy<0||y*scale+oy>=size)clipped++;}
 if(clipped)throw Error(`Clipped sprite ${frame}: ${clipped} pixels`);
 for(let y=0;y<size;y++)for(let x=0;x<size;x++){
  const x0=(x-.5-ox)/scale,x1=(x+.5-ox)/scale,y0=(y-.5-oy)/scale,y1=(y+.5-oy)/scale;
  const sum=[0,0,0,0,0,0];
  for(let sy=Math.max(0,Math.floor(y0));sy<Math.min(h,Math.ceil(y1));sy++){
   const wy=Math.max(0,Math.min(y1,sy+1)-Math.max(y0,sy));
   for(let sx=Math.max(0,Math.floor(x0));sx<Math.min(w,Math.ceil(x1));sx++){
    const weight=wy*Math.max(0,Math.min(x1,sx+1)-Math.max(x0,sx)),j=(sy*w+sx)*6;
    for(let c=0;c<6;c++)sum[c]+=layers[j+c]*weight;
   }
  }
  const area=(x1-x0)*(y1-y0),fa=sum[3]/area,pa=sum[5]/area,a=fa+pa;
  const d=((by+y)*width+bx+x)*4;
  if(a<=0)continue;
  const baseAlpha=pa<1?fa/(1-pa):0;
  fixed.data.set([sum[3]?Math.round(sum[0]/sum[3]*255):0,sum[3]?Math.round(sum[1]/sum[3]*255):0,sum[3]?Math.round(sum[2]/sum[3]*255):0,Math.round(baseAlpha*255)],d);
  const light=sum[5]?Math.round(sum[4]/sum[5]*255):0;
  paint.data.set([light,light,light,Math.round(pa*255)],d);
  coverage.data.set([255,255,255,Math.round(a*255)],d);
 }
 registrations.push({frame,heading,pose,from,scale,footContact:feet,running,ox,oy,decontaminated,clipped,originalRgbFallback:0});
}
const neutral=new PNG({width,height});neutral.data.set(composeTeamPaintRgba(fixed.data,paint.data,0xffffff));
const frameBounds=[];
for(let frame=0;frame<frameCount;frame++){
 let x0=size,y0=size,x1=-1,y1=-1;
 for(let y=0;y<size;y++)for(let x=0;x<size;x++)if(coverage.data[((Math.floor(frame/columns)*size+y)*width+(frame%columns)*size+x)*4+3]>=8){x0=Math.min(x0,x);y0=Math.min(y0,y);x1=Math.max(x1,x);y1=Math.max(y1,y);}
 frameBounds.push({x:x0,y:y0,width:x1-x0+1,height:y1-y0+1});
}
const topByDirection=headings.map((_,d)=>Math.min(...frameBounds.slice(d*16,d*16+16).map(b=>b.y)));
const muzzleByDirection=headings.map((heading,direction)=>{
 const frame=direction*16+12,bounds=frameBounds[frame],points=[];
 for(let y=bounds.y;y<bounds.y+bounds.height*(.58);y++)for(let x=bounds.x;x<bounds.x+bounds.width;x++){
  const i=((Math.floor(frame/columns)*128+y)*width+frame%columns*128+x)*4;
  const rgb=[...fixed.data.subarray(i,i+3)];
  if(fixed.data[i+3]>96&&paint.data[i+3]<32&&Math.max(...rgb)<160&&Math.max(...rgb)-Math.min(...rgb)<25)points.push({x,y});
 }
 if(!points.length)throw Error('Missing muzzle candidates '+heading);
 const angle=Math.PI/2+direction*Math.PI/8;
 const score=p=>p.x*Math.cos(angle)+p.y*Math.sin(angle);
 const maximum=Math.max(...points.map(score)),tip=points.filter(p=>score(p)>=maximum-1);
 return {x:tip.reduce((n,p)=>n+p.x,0)/tip.length-64,y:tip.reduce((n,p)=>n+p.y,0)/tip.length-112};
});
const geometry={anchor:[64,112],frameBounds,topByDirection,muzzleByDirection,headings,scaleFamilies:families.map(({scale,helmetWidth,renderedHelmetWidth})=>({scale,helmetWidth,renderedHelmetWidth}))};
const geometryFile=new URL('../client/src/renderer/rigs/rifleman_experimental_geometry.js',import.meta.url);
const geometryBytes=Buffer.from('export const RIFLEMAN_EXPERIMENTAL_GEOMETRY = '+JSON.stringify(geometry)+';\n');
if(process.argv.includes('--check')){if(!fs.readFileSync(geometryFile).equals(geometryBytes))throw Error('Stale geometry');}else fs.writeFileSync(geometryFile,geometryBytes);
const outputs={'geometry.json':Buffer.from(JSON.stringify(geometry,null,2)+'\n'),'fixed-atlas.png' :PNG.sync.write(fixed),'paint-atlas.png':PNG.sync.write(paint),'neutral-atlas.png':PNG.sync.write(neutral),'coverage-atlas.png':PNG.sync.write(coverage),
 'registration.json':Buffer.from(JSON.stringify({size,columns,originalAlphaLocked:false,sourceMaterialSplit:true,filter:'premultiplied area',registrations},null,2)+'\n')};
for(const [name,bytes]of Object.entries(outputs)){
 const file=new URL(name,root);
 if(process.argv.includes('--check')){if(!fs.readFileSync(file).equals(bytes))throw Error('Stale '+name);}else fs.writeFileSync(file,bytes);
}
console.log(JSON.stringify({frames:frameCount,oldRgbFallbacks:0,decontaminated:registrations.reduce((a,r)=>a+r.decontaminated,0),clipped:0}));
