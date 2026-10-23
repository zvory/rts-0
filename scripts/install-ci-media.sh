#!/usr/bin/env bash
# Install only the media tools needed by selected encoding tests on Ubuntu CI runners.
set -euo pipefail

# Avoid the Azure mirror's slow package downloads. Only rewrite that known mirror;
# leave every other configured repository intact on the disposable runner.
sudo python3 - <<'PY'
from pathlib import Path
for source in [Path('/etc/apt/sources.list'), Path('/etc/apt/apt-mirrors.txt'), *Path('/etc/apt/sources.list.d').glob('*.sources'), *Path('/etc/apt/sources.list.d').glob('*.list')]:
    if source.exists():
        original = source.read_text()
        updated = original.replace('http://azure.archive.ubuntu.com/ubuntu', 'https://archive.ubuntu.com/ubuntu').replace('https://azure.archive.ubuntu.com/ubuntu', 'https://archive.ubuntu.com/ubuntu')
        if updated != original:
            source.write_text(updated)
PY
sudo apt-get update
sudo apt-get install --yes --no-install-recommends ffmpeg
