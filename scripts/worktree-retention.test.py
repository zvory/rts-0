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

    def test_staged_content_recoverable_when_working_file_reverted(self):
        p = self.tree('staged')
        (p / 'source').write_bytes(b'staged\0content\n')
        self.run_git(p, 'add', 'source')
        (p / 'source').write_text('base\n')
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertFalse(p.exists())
        recovery = next((self.main / '.git/worktree-recovery').iterdir())
        restored = self.tasks / 'restored'
        self.run_git(self.main, 'worktree', 'add', str(restored), 'zvorygin/staged')
        self.run_git(restored, 'apply', '--cached', str(recovery / 'staged.patch'))
        self.assertEqual(self.run_git(restored, 'show', ':source'), b'staged\0content\n')
        self.assertEqual((restored / 'source').read_text(), 'base\n')

    def test_staged_notes_protected_even_when_working_file_reverted(self):
        p = self.tree('staged-notes')
        (p / 'playtest_notes.md').write_text('precious notes\n')
        self.run_git(p, 'add', 'playtest_notes.md')
        (p / 'playtest_notes.md').write_text('notes\n')
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertTrue(p.exists())

    def test_embedded_repository_and_registered_child_protected(self):
        p = self.tree('embedded')
        child = p / 'independent'
        child.mkdir()
        self.run_git(child, 'init')
        (child / 'precious').write_text('independent source\n')
        parent = self.tree('parent')
        registered = parent / 'child'
        self.run_git(self.main, 'worktree', 'add', '--detach', str(registered))
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertTrue((child / 'precious').exists())
        self.assertTrue(registered.exists())

    def test_recent_deletion_protects_old_tree(self):
        p = self.tree('deletion')
        (p / 'subdir').mkdir()
        (p / 'subdir/file').write_text('source\n')
        self.run_git(p, 'add', '.')
        self.run_git(p, 'commit', '-m', 'unmerged')
        old = time.time() - 74 * 3600
        gitdir = Path(os.fsdecode(self.run_git(p, 'rev-parse', '--absolute-git-dir')).strip())
        for item in [p, p / 'source', p / 'playtest_notes.md', p / 'subdir',
                     p / 'subdir/file', gitdir / 'HEAD', gitdir / 'logs/HEAD', gitdir / 'index']:
            os.utime(item, (old, old))
        (p / 'subdir/file').unlink()
        retention.cleanup(self.main)
        self.assertTrue(p.exists())

    def test_status_does_not_keep_stale_dirty_tree_alive(self):
        p = self.tree('stale')
        (p / 'source').write_text('dirty\n')
        old = time.time() - 74 * 3600
        gitdir = Path(os.fsdecode(self.run_git(p, 'rev-parse', '--absolute-git-dir')).strip())
        for item in [p, p / 'source', p / 'playtest_notes.md', gitdir / 'HEAD',
                     gitdir / 'logs/HEAD', gitdir / 'index']:
            os.utime(item, (old, old))
        retention.cleanup(self.main)
        self.assertFalse(p.exists())

    def test_incomplete_git_operation_protected(self):
        p = self.tree('merging')
        gitdir = Path(os.fsdecode(self.run_git(p, 'rev-parse', '--absolute-git-dir')).strip())
        (gitdir / 'MERGE_HEAD').write_bytes(self.run_git(p, 'rev-parse', 'HEAD'))
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertTrue(p.exists())

    def test_detached_sibling_recovered(self):
        p = self.main.with_name('repo-detached')
        self.run_git(self.main, 'worktree', 'add', '--detach', str(p))
        head = self.run_git(p, 'rev-parse', 'HEAD').strip()
        retention.cleanup(self.main)
        self.assertFalse(p.exists())
        refs = self.run_git(self.main, 'for-each-ref', '--format=%(objectname)', 'refs/worktree-recovery/')
        self.assertIn(head, refs)

    def test_index_flags_cannot_hide_modified_source(self):
        for flag in ('--assume-unchanged', '--skip-worktree'):
            with self.subTest(flag=flag):
                p = self.tree(flag[2:])
                self.run_git(p, 'update-index', flag, 'source')
                (p / 'source').write_text('hidden source changes\n')
                retention.cleanup(self.main, now=time.time() + 73 * 3600)
                self.assertEqual((p / 'source').read_text(), 'hidden source changes\n')

    def test_hidden_untracked_files_still_backed_up(self):
        p = self.tree('hidden-untracked')
        self.run_git(p, 'config', 'status.showUntrackedFiles', 'no')
        (p / 'precious').write_text('untracked source\n')
        retention.cleanup(self.main)
        self.assertTrue(p.exists())
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertFalse(p.exists())
        recovery = next((self.main / '.git/worktree-recovery').iterdir())
        import tarfile
        with tarfile.open(recovery / 'source.tar.gz') as archive:
            self.assertEqual(archive.extractfile('precious').read(), b'untracked source\n')

    def test_ignored_embedded_repository_protected(self):
        p = self.tree('ignored-repo')
        (p / '.gitignore').write_text('cache/\n')
        self.run_git(p, 'add', '.gitignore')
        self.run_git(p, 'commit', '-m', 'ignore cache')
        child = p / 'cache' / 'independent'
        child.mkdir(parents=True)
        self.run_git(child, 'init')
        (child / 'precious').write_text('independent source\n')
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        self.assertTrue((child / 'precious').exists())

    def test_recovery_patch_ignores_user_prefix_configuration(self):
        p = self.tree('prefix')
        self.run_git(p, 'config', 'diff.noprefix', 'true')
        (p / 'source').write_text('changed\n')
        retention.cleanup(self.main, now=time.time() + 73 * 3600)
        recovery = next((self.main / '.git/worktree-recovery').iterdir())
        restored = self.tasks / 'restored'
        self.run_git(self.main, 'worktree', 'add', str(restored), 'zvorygin/prefix')
        self.run_git(restored, 'apply', str(recovery / 'changes.patch'))
        self.assertEqual((restored / 'source').read_text(), 'changed\n')

    def test_shell_wrapper_with_and_without_dry_run(self):
        import shutil
        scripts = self.main / 'scripts'
        scripts.mkdir()
        for name in ('cleanup-worktrees.sh', 'worktree-retention.py'):
            shutil.copy2(Path(__file__).with_name(name), scripts / name)
        env = dict(os.environ, RTS_CARGO_TARGET_BASE_DIR=str(self.tasks / 'no-targets'))
        p = self.tree('wrapper')
        subprocess.run(['bash', str(scripts / 'cleanup-worktrees.sh'), '--dry-run'],
                       cwd=self.main, env=env, check=True, capture_output=True)
        self.assertTrue(p.exists())
        subprocess.run(['bash', str(scripts / 'cleanup-worktrees.sh')],
                       cwd=self.main, env=env, check=True, capture_output=True)
        self.assertFalse(p.exists())


if __name__ == '__main__':
    unittest.main()
