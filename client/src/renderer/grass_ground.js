import { createWorkerSafeCanvas } from "./raster_primitives.js";

const SIZE = 256;
function hash(x,y) {
  let value = Math.imul(x+731,374761393) ^ Math.imul(y+19,668265263);
  value = Math.imul(value ^ (value >>> 13),1274126177);
  return ((value ^ (value >>> 16)) >>> 0)/4294967295;
}
function smooth(value) { return value*value*(3-2*value); }
function mix(a,b,t) { return a+(b-a)*t; }
function noise(x,y,periodX,periodY) {
  const ix=Math.floor(x), iy=Math.floor(y), fx=smooth(x-ix), fy=smooth(y-iy);
  const sample=(dx,dy)=>hash(((ix+dx)%periodX+periodX)%periodX,((iy+dy)%periodY+periodY)%periodY);
  return mix(mix(sample(0,0),sample(1,0),fx),mix(sample(0,1),sample(1,1),fx),fy);
}

/** Low-frequency, static ground art. No fine blade carpet and no time-varying grain. */
export function grassGroundPixels(technique) {
  const pixels=new Uint8ClampedArray(SIZE*SIZE*4);
  for (let y=0;y<SIZE;y++) for (let x=0;x<SIZE;x++) {
    const broad=noise(x/64,y/64,4,4);
    const middle=noise(x/16,y/16,16,16);
    const small=noise(x/8,y/8,32,32);
    let color;
    if (technique===9) {
      const amount=broad*.55+middle*.35+small*.10;
      color=[mix(59,112,amount),mix(75,119,amount),mix(33,59,amount)];
    } else if (technique===10) {
      const sweep=noise(x/32+Math.sin(y/SIZE*Math.PI*16)*.15,y/8,8,32);
      const amount=broad*.45+middle*.25+sweep*.30;
      color=[mix(60,112,amount),mix(75,120,amount),mix(33,60,amount)];
      // Subdued broad strokes suggest flattened turf without dense animated strands.
      const stroke=Math.max(0,(.43-sweep)*.16);
      color=color.map(channel=>channel*(1-stroke));
    } else {
      const amount=broad*.6+middle*.3+small*.1;
      const soil=Math.min(1,Math.max(0,(.48-broad)*4))* .62;
      color=[mix(57,110,amount),mix(76,119,amount),mix(34,58,amount)];
      color=color.map((channel,i)=>mix(channel,[100,83,49][i],soil));
    }
    const i=(y*SIZE+x)*4;
    pixels[i]=color[0]; pixels[i+1]=color[1]; pixels[i+2]=color[2]; pixels[i+3]=255;
  }
  return { width:SIZE,height:SIZE,pixels };
}

function paintGroundMarks(ctx,technique) {
  const count=technique===9 ? 450 : technique===10 ? 350 : 140;
  for(let i=0;i<count;i++) {
    const x=hash(i,41)*SIZE, y=hash(i,57)*SIZE;
    const r=hash(i,83), direction=hash(i,97)*Math.PI*2;
    for(const dx of [-SIZE,0,SIZE]) for(const dy of [-SIZE,0,SIZE]) {
      const px=x+dx, py=y+dy;
      if(technique===10) {
        const length=6+r*8;
        ctx.strokeStyle=r>.5 ? "rgba(158,164,99,.22)" : "rgba(29,43,22,.20)";
        ctx.lineWidth=1.2+r;
        ctx.beginPath();
        ctx.moveTo(px,py);
        ctx.quadraticCurveTo(px+length*.5,py-1.5,px+length,py+1);
        ctx.stroke();
      } else {
        const radius=technique===9 ? 1.4+r*2.5 : 3+r*6;
        ctx.fillStyle=technique===9
          ? (r>.5 ? "rgba(158,157,91,.20)" : "rgba(39,53,26,.17)")
          : (r>.5 ? "rgba(125,102,64,.22)" : "rgba(40,57,30,.17)");
        ctx.beginPath();
        for(let side=0;side<6;side++) {
          const angle=direction+side/6*Math.PI*2;
          const extent=radius*(.65+hash(i,side)*.5);
          const vx=px+Math.cos(angle)*extent, vy=py+Math.sin(angle)*extent*.55;
          if(side===0) ctx.moveTo(vx,vy); else ctx.lineTo(vx,vy);
        }
        ctx.closePath();
        ctx.fill();
      }
    }
  }
}

export function createGrassGroundMaterial(pixi,technique) {
  const field=grassGroundPixels(technique);
  const canvas=createWorkerSafeCanvas(field.width,field.height);
  const ctx=canvas.getContext("2d");
  const image=ctx.createImageData(field.width,field.height);
  image.data.set(field.pixels);
  ctx.putImageData(image,0,0);
  paintGroundMarks(ctx,technique);
  const texture=pixi.Texture.from(canvas);
  texture.source.scaleMode="linear";
  texture.source.addressMode="repeat";
  return {
    resources:{ uGround:texture.source },
    textureBytes:field.pixels.byteLength,
    destroy() { texture.destroy(true); },
  };
}
