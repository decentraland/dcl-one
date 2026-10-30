import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('ui_standards', Path(__file__).parents[1] / 'ui_standards.py')
ui_standards = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ui_standards)


class UiStandardsTest(unittest.TestCase):
    def test_scope_includes_web_and_excludes_non_web(self):
        self.assertTrue(ui_standards.is_ui_source('apps/sites/src/Button.jsx'))
        self.assertTrue(ui_standards.is_ui_source('apps/future-web/src/Button.mjs'))
        self.assertTrue(ui_standards.is_ui_source('library/ui/components/Button.tsx'))
        self.assertTrue(ui_standards.is_ui_source('rendering/bevy-explorer/source/react-web/src/Button.tsx'))
        self.assertFalse(ui_standards.is_ui_source('apps/dcl-scene-cli/tests/scene.ts'))
        self.assertFalse(ui_standards.is_ui_source('apps/bevy-explorer/build.ts'))
        self.assertFalse(ui_standards.is_ui_source('rendering/bevy-explorer/source/src/file.ts'))
        self.assertFalse(ui_standards.is_ui_source('services/catalyst/src/main.rs'))

    def test_forbidden_imports(self):
        self.assertTrue(ui_standards.forbidden_imports("import styled from 'styled-components'"))
        self.assertTrue(ui_standards.forbidden_imports("import { styled } from '@mui/material'"))
        self.assertTrue(ui_standards.forbidden_imports("import {\n Box, styled,\n} from '@mui/material'"))
        self.assertTrue(ui_standards.forbidden_imports("import { css } from '@emotion/css'"))
        self.assertTrue(ui_standards.forbidden_imports("++i; import styled from 'styled-components'"))
        self.assertTrue(ui_standards.forbidden_imports("const styled = require('styled-components')"))
        self.assertFalse(ui_standards.forbidden_imports(
            "// import styled from 'styled-components'\nconst x = \"import styled from 'styled-components'\""))
        self.assertFalse(ui_standards.forbidden_imports("import styles from './Button.module.css'"))

    def test_component_stylesheets(self):
        self.assertTrue(ui_standards.is_component_stylesheet('apps/sites/src/Button.css', {'apps/sites/src/Button.tsx'}))
        self.assertTrue(ui_standards.is_component_stylesheet('apps/sites/src/botón.css', {'apps/sites/src/Botón.tsx'}))
        self.assertTrue(ui_standards.is_component_stylesheet('apps/sites/src/Old.css', {'apps/sites/src/Old.jsx'}))
        self.assertFalse(ui_standards.is_component_stylesheet('apps/sites/src/Button.module.css', {'apps/sites/src/Button.tsx'}))
        self.assertFalse(ui_standards.is_component_stylesheet('apps/sites/src/global.css', {'apps/sites/src/Button.tsx'}))

    def test_added_lines_keep_lines_starting_with_plus(self):
        diff = '--- a/x.ts\n+++ b/x.ts\n@@ -0,0 +1,2 @@\n+++i\n+const a = 1\n'
        self.assertEqual(ui_standards.added_lines(diff), ['++i', 'const a = 1'])


class UiStandardsIntegrationTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.git('init', '-q')
        self.commit('initial')
        self.patch = patch.object(ui_standards, 'ROOT', self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.path = self.root / 'apps/sites/src'
        self.path.mkdir(parents=True)

    def git(self, *args):
        return subprocess.run(['git', *args], cwd=self.root, check=True, capture_output=True, text=True).stdout

    def commit(self, message):
        self.git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false',
                 'commit', '--allow-empty', '-qm', message)

    def test_untracked_unicode_styles_fail_then_module_passes(self):
        (self.path / 'Botón.tsx').write_text('export const Button = () => null\n')
        style = self.path / 'botón.css'
        style.write_text('.root {}\n')
        self.assertTrue(ui_standards.check('HEAD'))
        style.rename(self.path / 'botón.module.css')
        self.assertEqual(ui_standards.check('HEAD'), [])

    def test_tracked_non_ascii_path_is_checked(self):
        (self.path / 'Botón.tsx').write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Botón.tsx: new CSS-in-JS imports are not allowed'])

    def test_renamed_stylesheet_is_checked(self):
        (self.path / 'Chat.tsx').write_text('export const Chat = () => null\n')
        (self.path / 'Chat.module.css').write_text('.root {}\n')
        self.git('add', '.')
        self.commit('component')
        base = self.git('rev-parse', 'HEAD').strip()
        self.git('mv', 'apps/sites/src/Chat.module.css', 'apps/sites/src/Chat.css')
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Chat.css: new component styles must use .module.css'])
        self.commit('rename')
        self.assertEqual(ui_standards.check(base), ['apps/sites/src/Chat.css: new component styles must use .module.css'])

    def test_staged_snapshot_does_not_read_unstaged_or_untracked_files(self):
        component = self.path / 'Button.tsx'
        component.write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        component.write_text('export const Button = () => null\n')
        self.assertTrue(ui_standards.check('HEAD', staged=True))
        self.git('add', '.')
        (self.path / 'Unrelated.css').write_text('untracked')
        self.assertEqual(ui_standards.check('HEAD', staged=True), [])

    def test_default_base_includes_committed_branch_changes(self):
        self.git('update-ref', 'refs/remotes/origin/main', 'HEAD')
        (self.path / 'Button.tsx').write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        self.commit('change')
        self.assertTrue(ui_standards.check(ui_standards.default_base()))

    def test_zero_or_unknown_revision_falls_back(self):
        self.assertEqual(ui_standards.resolve_base('0' * 40), 'HEAD')
        self.assertEqual(ui_standards.resolve_base('f' * 40), 'HEAD')
        (self.path / 'Card.tsx').write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        self.commit('card')
        self.assertEqual(ui_standards.resolve_base('0' * 40), 'HEAD~1')
        self.assertTrue(ui_standards.check('0' * 40))

    def test_option_like_base_is_rejected(self):
        with self.assertRaises(ValueError):
            ui_standards.check('--output=/dev/null')


if __name__ == '__main__':
    unittest.main()
