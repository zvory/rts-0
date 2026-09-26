#!/usr/bin/env python3
"""Remove merged or inactive task worktrees, retaining recoverable source state."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tarfile
import time


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args])


def paths(raw):
    return [os.fsdecode(p) for p in raw.split(b'\0') if p]


def managed(path, main, task_root):
    return (path.parent == task_root or path.parent == main / '.codex-worktrees'
            or (path.parent == main.parent and path.name.startswith(main.name + '-')))


def latest_activity(path, files, gitdir):
    candidates = [path, gitdir / 'HEAD', gitdir / 'logs/HEAD']
    candidates.extend(path / name for name in files)
    return max(p.lstat().st_mtime for p in candidates if p.exists() or p.is_symlink())


def cleanup(root, dry_run=False, now=None):
    root = Path(root).resolve()
    now = time.time() if now is None else now
    common = Path(os.fsdecode(git(root, 'rev-parse', '--path-format=absolute', '--git-common-dir')).strip())
    main = common.parent.resolve()
    task_root = Path(os.environ.get('RTS_WORKTREE_ROOT', '/tmp/rts-worktrees')).resolve()
    records = git(root, 'worktree', 'list', '--porcelain', '-z').split(b'\0\0')
    for record in records:
        fields = record.split(b'\0')
        values = dict(os.fsdecode(f).split(' ', 1) if b' ' in f else (os.fsdecode(f), '')
                      for f in fields if f)
        if 'worktree' not in values:
            continue
        path = Path(values['worktree']).resolve()
        branch = values.get('branch', '').removeprefix('refs/heads/')
        if (path in (root, main) or 'locked' in values or 'prunable' in values
                or not managed(path, main, task_root)):
            continue
        if branch and not branch.startswith('zvorygin/'):
            continue
        if branch and (task_root / 'phase-runner-active' / branch.replace('/', '__')).exists():
            continue
        dirty = bool(git(path, 'status', '--porcelain=v1', '-z'))
        changed = paths(git(path, 'diff', 'HEAD', '--name-only', '-z'))
        untracked = paths(git(path, 'ls-files', '--others', '--exclude-standard', '-z'))
        if 'playtest_notes.md' in changed or 'playtest_notes.md' in untracked:
            print(f'keep notes worktree: {path}')
            continue
        files = paths(git(path, 'ls-files', '-z')) + untracked
        gitdir = Path(os.fsdecode(git(path, 'rev-parse', '--absolute-git-dir')).strip())
        age = now - latest_activity(path, files, gitdir)
        head = values['HEAD']
        merged = any(subprocess.run(['git', '-C', str(root), 'merge-base', '--is-ancestor', head, ref],
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
                     for ref in ('main', 'origin/main'))
        if not (merged and not dirty) and age < 72 * 3600:
            continue
        reason = 'merged and clean' if merged and not dirty else f'inactive for {age / 86400:.1f} days'
        print(f'{"would remove" if dry_run else "remove"}: {path} ({reason})', flush=True)
        if dry_run:
            continue
        # A ref preserves detached and unmerged commits even after future branch pruning.
        recovery = common / 'worktree-recovery' / f'{path.name}-{time.time_ns()}'
        recovery.mkdir(parents=True)
        ref = 'refs/worktree-recovery/' + recovery.name
        subprocess.run(['git', '-C', str(root), 'update-ref', ref, head], check=True)
        (recovery / 'metadata.json').write_text(json.dumps({'path': str(path), 'branch': branch,
                                                         'head': head, 'ref': ref}, indent=2))
        if dirty:
            (recovery / 'changes.patch').write_bytes(git(path, 'diff', '--binary', 'HEAD'))
            with tarfile.open(recovery / 'source.tar.gz', 'w:gz', dereference=False) as archive:
                for name in sorted(set(changed + untracked)):
                    source = path / name
                    if source.exists() or source.is_symlink():
                        archive.add(source, arcname=name, recursive=False)
        # --force permits ignored build output and backed-up source changes. Locked trees stay protected.
        subprocess.run(['git', '-C', str(root), 'worktree', 'remove', '--force', str(path)], check=True)
        print(f'recovery: {recovery}', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()
    cleanup(os.fsdecode(git('.', 'rev-parse', '--show-toplevel')).strip(), args.dry_run)
