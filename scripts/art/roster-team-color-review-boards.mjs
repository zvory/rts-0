// Finish existing authoritative captures; never creates a game or changes image pixels.
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import puppeteer from 'puppeteer-core';
export async function buildRosterReviewBoards(out,evidence){
const browser=await puppeteer.launch({executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
try{
 const page=await browser.newPage();
 const groups=['kriegsia','cultivators','ekat'].map(id=>({id,units:evidence.find(e=>e.group===id)?.units})).filter(g=>g.units);
 for(const group of groups){
  const captures=evidence.filter(e=>e.group===group.id).map(e=>({...e,data:fs.readFileSync(e.path).toString('base64')}));
  const result=await page.evaluate(async({group,captures})=>{
   const labelWidth=290,cell=400,header=140;
   const canvas=Object.assign(document.createElement('canvas'),{width:labelWidth+7*cell,height:header+group.units.length*cell});
   const ctx=canvas.getContext('2d');ctx.fillStyle='#20252b';ctx.fillRect(0,0,canvas.width,canvas.height);ctx.fillStyle='#fff';ctx.font='bold 36px sans-serif';ctx.fillText(`${group.id==='kriegsia'?'Kriegsia':group.id==='ekat'?'Ekat faction':'Cultivators'} — all units in seven colors`,25,48);ctx.font='23px sans-serif';ctx.fillText('Paused in-game captures • Camera scale 1 • DPR 2 • Representative poses',25,85);
   for(let column=0;column<captures.length;column++){
    const c=captures[column],image=new Image();image.src=`data:image/png;base64,${c.data}`;await image.decode();ctx.font='bold 23px sans-serif';ctx.fillText(c.label,labelWidth+column*cell+15,125);
    for(let row=0;row<group.units.length;row++){
     const p=group.units[row].kind==='scout_plane'?c.inspected.entities.find(e=>e.id===c.ids[row]):c.spawns[row],sx=(p.x-1760+500)*2,sy=(p.y-1776+400)*2;
     // Copy the game's unit/terrain/shadow pixels at 1:1; external labels only.
     ctx.drawImage(image,sx-cell/2,sy-cell/2,cell,cell,labelWidth+column*cell,header+row*cell,cell,cell);
     if(column===0){ctx.fillStyle='#fff';ctx.font='bold 25px sans-serif';ctx.fillText(group.units[row].label,20,header+row*cell+cell/2);}
    }
   }return canvas.toDataURL();
  },{group,captures});
  const file=path.join(out,`${group.id}-all-units-seven-colors.png`);fs.writeFileSync(file,Buffer.from(result.split(',')[1],'base64'));console.log(file);
 }

 const blue=evidence.filter(e=>e.label==='Blue').sort((a,b)=>['kriegsia','cultivators','ekat'].indexOf(a.group)-['kriegsia','cultivators','ekat'].indexOf(b.group)).map(e=>({...e,data:fs.readFileSync(e.path).toString('base64')}));
 const overview=await page.evaluate(async(captures)=>{
  const all=captures.flatMap(c=>c.units.map((u,i)=>({capture:c,unit:u,index:i}))),cell=360,columns=4,header=110;
  const canvas=Object.assign(document.createElement('canvas'),{width:cell*columns,height:header+Math.ceil(all.length/columns)*cell}),ctx=canvas.getContext('2d');ctx.fillStyle='#20252b';ctx.fillRect(0,0,canvas.width,canvas.height);ctx.fillStyle='#fff';ctx.font='bold 32px sans-serif';ctx.fillText('Full roster — 23 appearances and ranks',25,43);ctx.font='21px sans-serif';ctx.fillText('Actual paused game pixels • Blue review overview • Full seven-color sheets supplied separately',25,82);
  const images={};for(const c of captures){const i=new Image();i.src='data:image/png;base64,'+c.data;await i.decode();images[c.group]=i;}
  for(let i=0;i<all.length;i++){
   const {capture:c,unit:u,index}=all[i],p=u.kind==='scout_plane'?c.inspected.entities.find(e=>e.id===c.ids[index]):c.spawns[index],x=i%columns*cell,y=header+Math.floor(i/columns)*cell;
   const sx=(p.x-1760+500)*2,sy=(p.y-1776+400)*2;
   ctx.drawImage(images[c.group],sx-180,sy-150,360,310,x,y+45,360,310);ctx.fillStyle='#fff';ctx.font='bold 24px sans-serif';ctx.fillText(u.label,x+14,y+31);
  }return canvas.toDataURL();
 },blue);
 fs.writeFileSync(path.join(out,'full-roster-blue-overview.png'),Buffer.from(overview.split(',')[1],'base64'));
}finally{await browser.close();}

}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const out=path.resolve(process.argv[2]||'target/full-roster-team-color-review');
 await buildRosterReviewBoards(out,JSON.parse(fs.readFileSync(path.join(out,'evidence.json'),'utf8')));
}
