#!/usr/bin/env python3
"""Rebuild selected Dao ability cues and record measured normalization (requires FFmpeg)."""
from pathlib import Path
import array, hashlib, json, math, subprocess
ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / 'docs/sound/DAO_ABILITIES.json'

def run(args):
    return subprocess.run(args, check=True, capture_output=True, text=True)

def measure(path):
    result = run(['ffmpeg', '-hide_banner', '-i', str(path), '-af',
                  'loudnorm=I=-16:TP=-2:LRA=11:print_format=json', '-f', 'null', '-']).stderr
    stats = json.loads(result[result.rfind('{'):result.rfind('}') + 1])
    decoded = subprocess.run(['ffmpeg', '-v', 'error', '-i', str(path), '-ac', '1',
                              '-ar', '44100', '-f', 'f32le', '-'], check=True, capture_output=True).stdout
    samples = array.array('f'); samples.frombytes(decoded)
    windows = [samples[j:j+2205] for j in range(0, len(samples), 441) if j+2205 <= len(samples)]
    rms = max(math.sqrt(sum(x*x for x in w)/len(w)) for w in windows)
    return {'lufs': float(stats['input_i']), 'true_peak_dbtp': float(stats['input_tp']),
            'max_50ms_rms_dbfs': 20*math.log10(rms), 'duration_seconds': len(samples)/44100}

def main():
    manifest = json.loads(MANIFEST.read_text())
    output = ROOT/'client/assets/sound/combat'
    cast_levels = []
    for asset in manifest['assets']:
        source = ROOT/asset['raw_file']
        assert hashlib.sha256(source.read_bytes()).hexdigest() == asset['raw_sha256']
        anchor = asset['id'] == 'combat_magic_anchor_01'
        target = -20 if anchor else -16
        # Power-crossfade the last 0.8s into the first 0.8s, then rotate the buffer:
        # start and end meet at original time 0.8s; resulting period is 7.2 seconds.
        source_filter = ('[0:a]aformat=sample_rates=44100:channel_layouts=mono,asplit=3[h][b][t];'
          '[h]atrim=0:0.8,asetpts=PTS-STARTPTS,afade=t=in:d=0.8:curve=qsin[head];'
          '[t]atrim=7.2:8,asetpts=PTS-STARTPTS,afade=t=out:d=0.8:curve=qsin[tail];'
          '[tail][head]amix=inputs=2:normalize=0[mix];'
          '[b]atrim=0.8:7.2,asetpts=PTS-STARTPTS[body];'
          '[body][mix]concat=n=2:v=0:a=1[out]') if anchor else None
        # Intermediate sources stay next to provenance and are reproducible from the raw inputs.
        master = ROOT/'docs/sound/sources/dao-abilities'/f'{asset["id"]}-premaster.wav'
        command = ['ffmpeg','-v','error','-y','-i',str(source)]
        command += ['-filter_complex',source_filter,'-map','[out]'] if anchor else ['-ac','1','-ar','44100']
        run(command+['-c:a','pcm_f32le',str(master)])
        stats = measure(master)
        gain = min(target-stats['lufs'], -2-stats['true_peak_dbtp'])
        if anchor:
            gain = min(gain, min(cast_levels)-3-stats['max_50ms_rms_dbfs'])
        dest = output/f'{asset["id"]}.mp3'
        filters = f'volume={gain:.6f}dB'
        if not anchor:
            filters += f',afade=t=in:d=0.004,afade=t=out:st={stats["duration_seconds"]-.025:.6f}:d=0.025'
        run(['ffmpeg','-v','error','-y','-i',str(master),'-af',filters,
             '-ar','44100','-ac','1','-c:a','libmp3lame','-b:a','128k',str(dest)])
        measured = measure(dest)
        assert measured['true_peak_dbtp'] <= -1.8
        if anchor:
            assert measured['max_50ms_rms_dbfs'] <= min(cast_levels)-3
        else:
            cast_levels.append(measured['max_50ms_rms_dbfs'])
        asset.update(file=str(dest.relative_to(ROOT)), production_sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),
          processing={'script':'scripts/prepare-dao-ability-audio.py','invocation':'python3 scripts/prepare-dao-ability-audio.py',
          'target_lufs':target,'gain_db':gain,'maximum_premaster_true_peak_dbtp':-2,
          'source_filter':source_filter,'final_filter':filters,'output':'mono 44.1kHz MP3 128kbps',
          'runtime_gain':0.7,'decoded':measured})
        master.unlink()
        print(asset['id'], measured, flush=True)
    manifest['ffmpeg_version'] = run(['ffmpeg','-version']).stdout.splitlines()[0]
    manifest['anchor_mix'] = '4 LUFS below cast mastering target, additionally at least 3 dB below both cast cues in loudest decoded 50ms RMS before shared runtime gain.'
    MANIFEST.write_text(json.dumps(manifest,indent=2)+'\n')

if __name__ == '__main__': main()
