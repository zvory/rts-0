// Compare VP9 quality/resolution choices from one lossless 480px master.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
const [master, prefix, selection = "all"] = process.argv.slice(2);
if (!master || !prefix) throw Error('usage: node encode.mjs master.mkv output-prefix');
const variants = [
  {name:'480-q32',size:480,fps:30,crf:32},
  {name:'480-q44',size:480,fps:15,crf:44},
  {name:'240-q40',size:240,fps:15,crf:40},
  {name:'144-q44',size:144,fps:10,crf:44},
];
if (!["all", "compact", ...variants.map(v=>v.name)].includes(selection)) throw Error("unknown encoding selection");
const results=[];
for(const v of variants.filter(v=>selection==="all" || (selection==="compact" && ["480-q44","240-q40"].includes(v.name)) || selection===v.name)){
  const output=`${prefix}-${v.name}.webm`, started=performance.now();
  const result=spawnSync('ffmpeg',['-hide_banner','-loglevel','error','-n','-i',master,'-an',
    '-vf',`fps=${v.fps}:eof_action=pass,scale=${v.size}:${v.size}:flags=lanczos`,'-c:v','libvpx-vp9',
    '-b:v','0','-crf',String(v.crf),'-deadline','good','-cpu-used','4','-row-mt','1',
    '-pix_fmt','yuv420p','-g',String(v.fps*10),output],{stdio:'inherit'});
  if(result.error)throw result.error;
  if(result.status!==0)throw Error(`ffmpeg failed for ${output}`);
  const probe=spawnSync('ffprobe',['-v','error','-show_entries','format=duration:stream=codec_name,width,height,avg_frame_rate','-of','json',output],{encoding:'utf8'});
  if(probe.status!==0)throw Error('ffprobe failed');
  const row={...v,output,bytes:fs.statSync(output).size,encodeSeconds:(performance.now()-started)/1000,...JSON.parse(probe.stdout)};
  results.push(row);console.log(JSON.stringify(row));
}
fs.writeFileSync(`${prefix}-compression.json`,JSON.stringify(results,null,2)+'\n',{flag:'wx'});
