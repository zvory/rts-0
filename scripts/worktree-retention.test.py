import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import sys
sys.dont_write_bytecode = True
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('retention', Path(__file__).with_name('worktree-retention.py'))
retention = importlib.util.module_from_spec(spec)
spec.loader.exec_module(retention)


class RetentionTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.main = Path(self.temp.name) / 'repo'
        self.main.mkdir()
        self.run_git(self.main, 'init', '-b', 'main')
        self.run_git(self.main, 'config', 'user.email', 'test@example.com')
        self.run_git(self.main, 'config', 'user.name', 'Test')
        (self.main / 'source').write_text('base\n')
        (self.main / 'playtest_notes.md').write_text('notes\n')
        self.run_git(self.main, 'add', '.')
        self.run_git(self.main, 'commit', '-m', 'base')
        self.tasks = Path(self.temp.name) / 'tasks'
        self.tasks.mkdir()
        env = patch.dict(os.environ, RTS_WORKTREE_ROOT=str(self.tasks))
        env.start()
        self.addCleanup(env.stop)

    def run_git(self, root, *args):
        return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.DEVNULL)

    def tree(self, name, nested=False):
        p = (self.main / '.codex-worktrees' if nested else self.tasks) / name
        self.run_git(self.main, 'worktree', 'add', '-b', 'zvorygin/' + name, str(p))
        return p

    def test_merged_nested_removed_and_dry_run_safe(self):
        p = self.tree('merged', nested=True)
        retention.cleanup(self.main, dry_run=True)
        self.assertTrue(p.exists())
        retention.cleanup(self.main)
        self.assertFalse(p.exists())
        self.assertTrue(self.main.exists())

    def test_dirty_stale_recoverable_recent_protected(self):
        p = self.tree('unfinished')
        (p / 'source').write_text('changed\n')
        (p / 'new').write_text('untracked\n')
        retention.cleanup(self.main)
        self.assertTrue(p.exists())
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertFalse(p.exists())
        recovery = next((self.main / '.git/worktree-recovery').iterdir())
        self.assertIn(b'changed', (recovery / 'changes.patch').read_bytes())
        import tarfile
        with tarfile.open(recovery / 'source.tar.gz') as archive:
            self.assertEqual(archive.extractfile('new').read(), b'untracked\n')
        self.run_git(self.main, 'rev-parse', 'refs/heads/zvorygin/unfinished')

    def test_notes_locked_current_and_phase_marker_protected(self):
        notes = self.tree('notes')
        (notes / 'playtest_notes.md').write_text('keep me\n')
        locked = self.tree('locked')
        self.run_git(self.main, 'worktree', 'lock', str(locked))
        phase = self.tree('phase')
        marker = self.tasks / 'phase-runner-active/zvorygin__phase'
        marker.parent.mkdir()
        marker.touch()
        current = self.tree('current')
        retention.cleanup(current, now=time.time() + 73 * 3600)
        for p in (notes, locked, phase, current):
            self.assertTrue(p.exists())

    def test_unmerged_head_retained_and_new_activity_protects(self):
        p = self.tree('unmerged')
        (p / 'source').write_text('committed\n')
        self.run_git(p, 'commit', '-am', 'unfinished')
        retention.cleanup(self.main)
        self.assertTrue(p.exists())
        head = self.run_git(p, 'rev-parse', 'HEAD')
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertFalse(p.exists())
        refs = self.run_git(self.main, 'for-each-ref', '--format=%(objectname)', 'refs/worktree-recovery/')
        self.assertIn(head.strip(), refs)


if __name__ == '__main__':
    unittest.main()
