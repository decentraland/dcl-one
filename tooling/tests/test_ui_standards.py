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
        self.assertTrue(ui_standards.is_ui_source('apps/dcl-scene-cli/src/start/deploy_page.js'))
        self.assertFalse(ui_standards.is_ui_source('apps/dcl-scene-cli/scripts/golden-runtime.mjs'))
        self.assertFalse(ui_standards.is_ui_source('apps/dcl-scene-cli/src/templates/main.ts'))
        self.assertFalse(ui_standards.is_ui_source('apps/dcl-scene-cli/testdata/golden/ui/src/index.tsx'))
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

    def test_forbidden_import_forms(self):
        for source in ("import styled from '@mui/material/styles/styled'",
                       "import styled from '@mui/system/styled'",
                       "import createStyled from '@emotion/styled/base'",
                       "const lazy = import('styled-components')",
                       "require('styled-components')",
                       "import 'styled-components'",
                       "export { default } from 'styled-components'",
                       "import styled from '@mui/styled-engine'",
                       "const { styled } = require('@mui/material')",
                       "import * as M from '@mui/material'\nconst Card = M.styled('div')({})"):
            with self.subTest(source=source):
                self.assertTrue(ui_standards.forbidden_imports(source))

    def test_strings_and_comments_are_not_imports(self):
        self.assertTrue(ui_standards.forbidden_imports(
            "const icons = import.meta.glob('./icons/*.svg')\nimport styled from 'styled-components'\n/** doc */"))
        self.assertFalse(ui_standards.forbidden_imports(
            "import x from './a'\nconst s = \"y from 'styled-components'\""))
        self.assertFalse(ui_standards.forbidden_imports("export function styled() { return '@mui/system' }"))
        self.assertFalse(ui_standards.forbidden_imports("const url = 'https://example.invalid/styled-components'"))

    def test_statements_without_semicolons_stay_separate(self):
        self.assertFalse(ui_standards.forbidden_imports(
            "import { styled } from './theme'\nimport { Box } from '@mui/material'"))
        self.assertFalse(ui_standards.forbidden_imports(
            "export const styled = 1\nimport { Box } from '@mui/material'"))
        self.assertFalse(ui_standards.forbidden_imports(
            "export { styled } from './theme'\nconst m = import('@mui/material')"))
        self.assertTrue(ui_standards.forbidden_imports(
            "import {\n  Box,\n  styled\n} from '@mui/material'\nconst a = 1"))
        self.assertTrue(ui_standards.forbidden_imports("const { styled } = await import('@mui/material')"))

    def test_namespace_styled_is_counted_once(self):
        source = "import * as M from '@mui/material'\nconst A = M.styled('a')({})\nconst B = M.styled('b')({})"
        self.assertEqual(ui_standards.forbidden_imports(source), ['M.styled'])

    def test_emotion_jsx_pragma_is_detected(self):
        self.assertTrue(ui_standards.forbidden_imports("/** @jsxImportSource @emotion/react */\nexport const A = 1"))
        self.assertTrue(ui_standards.mentions_css_in_js("/** @jsxImportSource @emotion/react */"))

    def test_mentions_css_in_js(self):
        self.assertTrue(ui_standards.mentions_css_in_js('  styled,'))
        self.assertTrue(ui_standards.mentions_css_in_js("} from 'styled-components'"))
        self.assertTrue(ui_standards.mentions_css_in_js("} from '@mui/material'"))
        self.assertFalse(ui_standards.mentions_css_in_js('  Box,'))

    def test_component_stylesheets(self):
        self.assertTrue(ui_standards.is_component_stylesheet('apps/sites/src/Button.css', {'apps/sites/src/Button.tsx'}))
        self.assertTrue(ui_standards.is_component_stylesheet('apps/sites/src/botón.css', {'apps/sites/src/Botón.tsx'}))
        self.assertTrue(ui_standards.is_component_stylesheet('apps/sites/src/Old.css', {'apps/sites/src/Old.jsx'}))
        self.assertFalse(ui_standards.is_component_stylesheet('apps/sites/src/Button.module.css', {'apps/sites/src/Button.tsx'}))
        self.assertFalse(ui_standards.is_component_stylesheet('apps/sites/src/global.css', {'apps/sites/src/Button.tsx'}))
        self.assertFalse(ui_standards.is_component_stylesheet('apps/sites/src/App.css', {'apps/sites/src/App.tsx'}))
        self.assertFalse(ui_standards.is_component_stylesheet('apps/sites/src/main.css', {'apps/sites/src/main.tsx'}))
        self.assertFalse(ui_standards.is_component_stylesheet(
            'rendering/bevy-explorer/source/react-web/src/App.css', {'rendering/bevy-explorer/source/react-web/src/App.tsx'}))
        self.assertTrue(ui_standards.is_component_stylesheet(
            'apps/sites/src/components/Main/Main.css', {'apps/sites/src/components/Main/Main.tsx'}))
        self.assertTrue(ui_standards.is_component_stylesheet(
            'apps/sites/src/Button/index.css', {'apps/sites/src/Button/index.tsx'}))

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

    def test_moving_an_existing_plain_stylesheet_passes(self):
        (self.path / 'Button.tsx').write_text('export const Button = () => null\n')
        (self.path / 'Button.css').write_text('.root {}\n')
        self.git('add', '.')
        self.commit('legacy')
        self.git('mv', 'apps/sites/src', 'apps/sites/moved')
        self.assertEqual(ui_standards.check('HEAD'), [])

    def test_added_line_inside_existing_multi_line_import_is_checked(self):
        component = self.path / 'Panel.tsx'
        component.write_text("import {\n  Box,\n} from '@mui/material'\n")
        self.git('add', '.')
        self.commit('panel')
        component.write_text("import {\n  Box,\n  styled,\n} from '@mui/material'\n")
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Panel.tsx: new CSS-in-JS imports are not allowed'])

    def test_changing_only_the_from_line_is_checked(self):
        component = self.path / 'Panel.tsx'
        component.write_text("import {\n  Box,\n  styled,\n} from './theme'\n")
        self.git('add', '.')
        self.commit('panel')
        component.write_text("import {\n  Box,\n  styled,\n} from '@mui/material'\n")
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Panel.tsx: new CSS-in-JS imports are not allowed'])

    def test_existing_import_is_not_reported_as_new(self):
        component = self.path / 'Legacy.tsx'
        component.write_text("import styled from 'styled-components'\nexport const A = styled.div``\n")
        self.git('add', '.')
        self.commit('legacy')
        component.write_text("import styled from 'styled-components'\nexport const A = styled.div``\nexport const B = styled.span``\n")
        self.assertEqual(ui_standards.check('HEAD'), [])

    def test_colored_diff_config_does_not_hide_added_lines(self):
        self.git('config', 'color.diff', 'always')
        component = self.path / 'Card.tsx'
        component.write_text('export const Card = () => null\n')
        self.git('add', '.')
        self.commit('card')
        component.write_text("import styled from 'styled-components'\nexport const Card = () => null\n")
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Card.tsx: new CSS-in-JS imports are not allowed'])

    def test_renaming_a_global_stylesheet_into_a_component_stylesheet_fails(self):
        (self.path / 'Card.tsx').write_text('export const Card = () => null\n')
        (self.path / 'global.css').write_text('.root {}\n')
        self.git('add', '.')
        self.commit('global')
        self.git('mv', 'apps/sites/src/global.css', 'apps/sites/src/Card.css')
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Card.css: new component styles must use .module.css'])

    def test_non_utf8_source_does_not_stop_the_check(self):
        (self.path / 'Latin.tsx').write_bytes(b"// caf\xe9\nimport styled from 'styled-components'\n")
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Latin.tsx: new CSS-in-JS imports are not allowed'])

    def test_shallow_clone_without_history_is_an_error(self):
        (self.path / 'Card.tsx').write_text('export const Card = () => null\n')
        self.git('add', '.')
        self.commit('card')
        shallow = Path(self.directory.name) / 'shallow'
        subprocess.run(['git', 'clone', '-q', '--depth', '1', f'file://{self.root}', str(shallow)], check=True,
                       capture_output=True)
        with patch.object(ui_standards, 'ROOT', shallow):
            with self.assertRaises(ValueError):
                ui_standards.check('0' * 40)

    def test_missing_origin_main_warns(self):
        with patch('sys.stderr') as stderr:
            self.assertEqual(ui_standards.default_base(), 'HEAD')
        self.assertIn('origin/main is not available', ''.join(call.args[0] for call in stderr.write.call_args_list))

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
