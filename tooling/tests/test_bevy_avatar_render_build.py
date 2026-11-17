"""The avatar renderer recipe builds the scene host beside the renderer that its wrapper runs."""
import importlib.util
import os
from pathlib import Path
import shlex
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('avatar_render_build',
    Path(__file__).resolve().parents[2] / 'apps/bevy-explorer/build-avatar-render.py')
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class AvatarRenderBuildTests(unittest.TestCase):
    def test_the_host_and_the_renderer_build_into_the_directory_the_wrapper_runs_from(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, dist = Path(temporary) / 'source', Path(temporary) / 'dist'
            with patch.object(build, 'SOURCE', source), patch.object(build, 'DIST', dist), \
                    patch.object(build.subprocess, 'run') as run:
                build.main()
            sources, *cargo = run.call_args_list
            self.assertEqual(list(sources.args[0]), [
                sys.executable, str(build.ROOT / 'tooling/patched-sources.py'), 'materialise', '--quiet'])
            self.assertEqual(sources.kwargs, {'check': True})
            self.assertEqual([list(call.args[0]) for call in cargo], [
                ['cargo', 'build', '--release', '--locked', '-p', 'dcl_deno_ipc'],
                ['cargo', 'build', '--release', '--locked', '--no-default-features',
                 '--features=avatar_render', '--bin', 'avatar_render'],
            ])
            self.assertTrue(all(call.kwargs == {'cwd': source, 'check': True} for call in cargo))
            wrapper = dist / 'avatar_render'
            self.assertTrue(os.access(wrapper, os.X_OK))
            renderer = shlex.quote(str(source / 'target/release/avatar_render'))
            self.assertEqual(wrapper.read_text().splitlines()[-1], f'exec {renderer} "$@"')


if __name__ == '__main__':
    unittest.main()
