#!/usr/bin/env python3
"""Normalize the selected Silk Current audition without changing its timing/dynamics."""
import array
import hashlib
import json
import math
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / 'docs/sound/VECTOR_FIELD.json'


def run(args):
    return subprocess.run(args, check=True, capture_output=True, text=True)


def measure(path):
    log = run(['ffmpeg', '-hide_banner', '-i', str(path), '-af',
               'aformat=channel_layouts=mono,loudnorm=I=-20:TP=-2:LRA=11:print_format=json', '-f', 'null', '-']).stderr
    levels = json.JSONDecoder().raw_decode(log[log.rfind('{'):])[0]
    pcm = subprocess.run(['ffmpeg', '-v', 'error', '-i', str(path), '-ac', '1',
                          '-ar', '44100', '-f', 'f32le', '-'], check=True, capture_output=True).stdout
    samples = array.array('f')
    samples.frombytes(pcm)
    loudest = max(math.sqrt(sum(v*v for v in samples[i:i+2205])/2205)
                  for i in range(0, len(samples)-2204, 441))
    return {'lufs': float(levels['input_i']), 'true_peak_dbtp': float(levels['input_tp']),
            'max_50ms_rms_dbfs': 20*math.log10(loudest), 'duration_seconds': len(samples)/44100}


def main():
    data = json.loads(MANIFEST.read_text())
    selected = ROOT / data['selection']['selected_source']
    expected = next(step['sha256'] for step in data['processing']
                    if step.get('output') == data['selection']['selected_source'])
    assert hashlib.sha256(selected.read_bytes()).hexdigest() == expected
    # Measure mono before choosing gain, matching the spatial production format.
    source = measure(selected)
    casts = [measure(ROOT / f'client/assets/sound/combat/{name}.mp3')
             for name in ['combat_lightness_dash_01', 'combat_magic_shield_01']]
    rms_ceiling = min(c['max_50ms_rms_dbfs'] for c in casts) - 3
    gain = min(-20-source['lufs'], -2-source['true_peak_dbtp'],
               rms_ceiling-source['max_50ms_rms_dbfs'])
    output = ROOT / 'client/assets/sound/combat/combat_vector_field_01.mp3'
    filters = f'aformat=channel_layouts=mono,volume={gain:.9f}dB'
    command = ['ffmpeg', '-v', 'error', '-y', '-i', str(selected), '-af', filters,
               '-ar', '44100', '-c:a', 'libmp3lame', '-b:a', '128k', str(output)]
    run(command)
    decoded = measure(output)
    assert decoded['true_peak_dbtp'] <= -1.8
    assert decoded['max_50ms_rms_dbfs'] <= rms_ceiling + 0.1
    assert abs(decoded['duration_seconds'] - 10) < 0.001
    data['production'] = {
        'file': str(output.relative_to(ROOT)), 'sha256': hashlib.sha256(output.read_bytes()).hexdigest(),
        'script': 'scripts/prepare-vector-field-audio.py',
        'invocation': 'python3 scripts/prepare-vector-field-audio.py',
        'ffmpeg_version': run(['ffmpeg', '-version']).stdout.splitlines()[0],
        'filter': filters, 'target_lufs': -20, 'gain_db': gain,
        'true_peak_ceiling_dbtp': -2, 'max_50ms_rms_ceiling_dbfs': rms_ceiling,
        'source_measurements': source, 'cast_reference_measurements': casts,
        'decoded': decoded, 'runtime_gain': 0.7,
        'format': 'mono 44.1kHz MP3 128kbps',
    }
    MANIFEST.write_text(json.dumps(data, indent=2) + '\n')
    print(json.dumps(data['production'], indent=2))


if __name__ == '__main__':
    main()
