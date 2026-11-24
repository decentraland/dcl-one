import json
import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).parents[1] / 'ui_standards.py'
SPEC = importlib.util.spec_from_file_location('ui_standards', SCRIPT)
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

    def test_deep_shared_imports(self):
        found = ui_standards.deep_shared_imports(
            'import { Button } from "@dcl/one-ui"\n'
            'import { useChromeAuth } from "@dcl/one-ui/components/AppChrome/chrome-auth"\n'
            "import type { IconProps } from '@dcl/one-ui/foundations/icons'\n"
            'export { theme } from "@dcl/one-ui/foundations"\n'
            'const page = await import("@dcl/one-ui/components/NavBar/NavBar")\n'
            'vi.mock("@dcl/one-ui/components/AppChrome/chrome-nav", () => ({}))\n'
            'import { EventsPage } from "@dcl/one-ui/apps/sites/events/EventsPage"\n'
            'import logo from "@dcl/one-ui/apps/sites/assets/dcl-logo.svg?url"\n'
            'import { shopFlags } from "@dcl/one-ui/apps/shopping/flags"\n')
        self.assertEqual(found, ['@dcl/one-ui/components/AppChrome/chrome-auth', '@dcl/one-ui/foundations/icons',
                                 '@dcl/one-ui/foundations', '@dcl/one-ui/components/NavBar/NavBar',
                                 '@dcl/one-ui/components/AppChrome/chrome-nav',
                                 '@dcl/one-ui/apps/sites/events/EventsPage',
                                 '@dcl/one-ui/apps/sites/assets/dcl-logo.svg?url',
                                 '@dcl/one-ui/apps/shopping/flags'])

    def test_deep_shared_imports_allow_stylesheets_shop_and_hud_ui_and_comments(self):
        self.assertEqual(ui_standards.deep_shared_imports(
            'import "@dcl/one-ui/foundations.css"\n'
            'import "@dcl/one-ui/foundations/colors/colors.css"\n'
            'import styles from "@dcl/one-ui/text-styles.module.css"\n'
            'import { shopSession } from "@dcl/one-ui/apps/shop/session"\n'
            'import { ARM_MS } from "@dcl/one-ui/apps/hud/design"\n'
            '// import { NavBar } from "@dcl/one-ui/components/NavBar/NavBar"\n'
            'const ALIAS = "@dcl/one-ui/"\n'
            'const other = "@dcl/one-ui-extra/components/X"\n'), [])

    def test_shared_import_scope(self):
        deep = 'import { NavBar } from "@dcl/one-ui/components/NavBar/NavBar"\n'
        sources = {
            'apps/sites/src/Page.tsx': deep,
            'apps/sites/scripts/check.mts': deep,
            'tooling/shop-parity/page.mts': deep,
            'library/shop/cart.ts': deep,
            'library/ui/test/consumer/types.ts': deep,
            'rendering/bevy-explorer/source/react-web/src/App.tsx': deep,
            'apps/sites/README.md': deep,
            'apps/sites/src/Clean.tsx': 'import { NavBar } from "@dcl/one-ui"\n',
            'apps/ui3/src/shop/discovery/components/LivePromo.tsx':
                f'const A = lazy(() => import("{ui_standards.LAZY_ANIMATED_BACKGROUND}"))\n',
            'apps/ui3/src/shop/discovery/components/LivePromo.test.tsx': deep,
            'apps/ui3/src/Other.tsx': f'const A = lazy(() => import("{ui_standards.LAZY_ANIMATED_BACKGROUND}"))\n',
        }
        issues = ui_standards.shared_import_issues(sources, sources.__getitem__)
        self.assertEqual([issue.split(':')[0] for issue in issues],
                         ['apps/sites/scripts/check.mts', 'apps/sites/src/Page.tsx',
                          'apps/ui3/src/Other.tsx', 'apps/ui3/src/shop/discovery/components/LivePromo.test.tsx',
                          'library/shop/cart.ts', 'tooling/shop-parity/page.mts'])
        self.assertIn("from '@dcl/one-ui' instead", issues[0])

    def test_inspector_tooltip_import_is_limited_to_its_adapter_and_component(self):
        adapter = 'apps/creator-hub/packages/inspector/src/components/ui/Tooltip.ts'
        shop = 'apps/ui3/src/shop/detail/ItemDetail.tsx'
        tooltip = 'export { Tooltip } from "@dcl/one-ui/components/Tooltip/Tooltip"'
        button = 'export { Button } from "@dcl/one-ui/components/Button/Button"'
        sources = {adapter: tooltip, shop: tooltip}
        self.assertEqual([issue.split(':')[0] for issue in ui_standards.shared_import_issues(
            sources, sources.__getitem__)], [shop])
        self.assertTrue(ui_standards.shared_import_issues([adapter], {adapter: button}.__getitem__))

    def test_relative_shared_imports(self):
        found = ui_standards.relative_shared_imports('apps/sites/scripts/check.mts', (
            'import { SECTIONS } from "../../../library/ui/apps/sites/flow-map/exflowmapdata"\n'
            "import type { IconProps } from '../../../library/ui/foundations/icons/index.ts'\n"
            'export { Button } from "../../../library/ui/components/Button/Button"\n'
            'import "../../../library/ui/foundations/setup"\n'
            'let copy: typeof import("../../../library/ui/apps/sites/status/statusCopy.ts")\n'
            'const page = await import("../../../library/ui/components/NavBar/NavBar")\n'
            'const legacy = require("../../../library/ui/index.ts")\n'
            'vi.mock("../../../library/ui/components/AppChrome/chrome-nav", () => ({}))\n'
            'vi.doMock("../../../library/ui/svg/logo.svg?url")\n'
            'import { x } from "../../../library/ui"\n'))
        self.assertEqual(found, ['../../../library/ui/apps/sites/flow-map/exflowmapdata',
                                 '../../../library/ui/foundations/icons/index.ts',
                                 '../../../library/ui/components/Button/Button',
                                 '../../../library/ui/foundations/setup',
                                 '../../../library/ui/apps/sites/status/statusCopy.ts',
                                 '../../../library/ui/components/NavBar/NavBar',
                                 '../../../library/ui/index.ts',
                                 '../../../library/ui/components/AppChrome/chrome-nav',
                                 '../../../library/ui/svg/logo.svg?url',
                                 '../../../library/ui'])

    def test_relative_shared_imports_allow_stylesheets_tooling_fixtures_and_other_paths(self):
        self.assertEqual(ui_standards.relative_shared_imports('apps/ui3/src/web/pages/StInvite.browser.test.ts', (
            'import styles from "../../../../../library/ui/components/Button/Button.module.css"\n'
            'import css from "../../../../../library/ui/components/Button/Button.module.css?inline"\n'
            'import { openSession } from "../../../../../library/ui/figma/review/scripts/capture-browser.mjs"\n'
            '// import { NavBar } from "../../../../../library/ui/components/NavBar/NavBar"\n'
            "const fixture = 'import { x } from \"../../../../../library/ui/apps/shop/capabilities.ts\"'\n"
            'const root = new URL("../../../../../library/ui/", import.meta.url)\n'
            'import { flow } from "../../../../../library/web-core/flow-map"\n'
            'import { ui } from "../../../../../library/uikit/index"\n'
            'import { Page } from "./Page"\n'
            'import { NavBar } from "@dcl/one-ui"\n')), [])

    def test_relative_shared_imports_resolve_against_the_file(self):
        specifier = 'import { x } from "../../library/ui/index.ts"\n'
        self.assertEqual(ui_standards.relative_shared_imports('apps/sites/check.mts', specifier),
                         ['../../library/ui/index.ts'])
        self.assertEqual(ui_standards.relative_shared_imports('apps/sites/scripts/check.mts', specifier), [])

    def test_relative_shared_import_scope(self):
        relative = 'import { SECTIONS } from "../../../library/ui/apps/sites/flow-map/data"\n'
        pending = 'apps/sites/src/routes/routes/_index.tsx'
        sources = {
            'apps/sites/scripts/check.mts': relative,
            pending: 'import { x } from "' + '../' * pending.count('/') + 'library/ui/components/X"\n',
            'apps/sites/README.md': relative,
            'library/shop/scripts/x/check.mts': relative,
            'tooling/shop-parity/page.mts': relative,
            'apps/sites/scripts/clean.mts': 'import { SECTIONS } from "@web-core/flow-map"\n',
        }
        issues = ui_standards.shared_import_issues(sources, sources.__getitem__)
        self.assertEqual(sorted(issue.split(':')[0] for issue in issues),
                         sorted(['apps/sites/scripts/check.mts', pending]))
        self.assertIn("from '@dcl/one-ui' instead", issues[0])

    def test_shared_import_scope_empty(self):
        self.assertEqual(ui_standards.shared_import_issues(set(), lambda name: ''), [])

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


class TokensReportTest(unittest.TestCase):
    LENGTHS = ui_standards.token_lengths(
        '--dcl-space-8: 8px; --dcl-space-16: 16px; --dcl-radius-md: 12px; --dcl-font-size-body-md: 16px;')

    def test_token_lengths_read_the_space_and_radius_tokens_only(self):
        self.assertEqual(self.LENGTHS, {8.0, 12.0, 16.0})

    def test_stylesheet_report_counts_literals_token_uses_and_drift(self):
        css = ('/* no token for the hero: reference #0b0a0c */\n'
               '.hero { background: #0b0a0c url("red.svg"); color: rgba(255, 255, 255, 0.6); border-color: white;\n'
               '  padding: 16px 10px; border-radius: 12px; font-family: Inter, sans-serif; }\n'
               '.card { color: var(--dcl-text-primary); gap: var(--dcl-space-8); outline: 1px solid transparent;\n'
               '  white-space: nowrap; margin: 8px; content: "white"; }\n'
               '@media (min-width: 768px) { .hero { padding: 0 16px; } }\n')
        self.assertEqual(ui_standards.stylesheet_report(css, self.LENGTHS),
                         {'colours': 3, 'lengths': 4, 'fonts': 1, 'tokens': 2, 'drift': 1})

    def test_tokens_report_lists_each_stylesheet_and_the_totals(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'a').mkdir()
            (root / 'node_modules').mkdir()
            (root / 'a' / 'A.module.css').write_text('.a { color: #fff; }')
            (root / 'b.css').write_text('.b { gap: 8px; color: var(--dcl-text-primary); }')
            (root / 'node_modules' / 'x.css').write_text('.x { color: red; }')
            lines = ui_standards.tokens_report([str(root)], self.LENGTHS)
            self.assertEqual(len(lines), 4)
            self.assertIn('a/A.module.css', lines[1])
            self.assertTrue(lines[3].startswith('total (2 files)'))
            self.assertEqual(lines[3].split()[-5:], ['1', '1', '0', '1', '0'])
            result = subprocess.run([sys.executable, str(SCRIPT), '--tokens', str(root)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.splitlines()[-1].split()[-5:], ['1', '1', '0', '1', '0'])

    def test_token_floor_counts_tokens_and_primitives_outside_comments(self):
        primitives = ui_standards.primitive_names(':root { --ink-1: #fff; --panel: #000; /* --gone: 1px; */ }')
        self.assertEqual(primitives, {'--ink-1', '--panel'})
        text = '.a { color: var(--dcl-text-default); background: var(--panel); border-color: var(--own); }\n/* var(--ink-1) */'
        self.assertEqual(ui_standards.floor_counts(text, primitives), {'tokens': 1, 'primitives': 1})

    def test_token_floor_fails_on_fewer_tokens_or_more_primitives(self):
        floor = {'a.css': {'tokens': 3, 'primitives': 1}}
        self.assertEqual(ui_standards.floor_issues(floor, {'a.css': {'tokens': 4, 'primitives': 0}}), [])
        self.assertEqual(len(ui_standards.floor_issues(floor, {'a.css': {'tokens': 2, 'primitives': 2}})), 2)
        self.assertEqual(ui_standards.floor_issues({}, {'new.css': {'tokens': 0, 'primitives': 1}}),
                         ['new.css: 1 uses of atoms/primitives.css properties, the ceiling is 0'])

    def test_token_floor_fails_on_an_entry_whose_stylesheet_is_gone(self):
        floor = {'moved.css': {'tokens': 3, 'primitives': 0}, 'kept.css': {'tokens': 1, 'primitives': 0}}
        current = {'kept.css': {'tokens': 1, 'primitives': 0}, 'new.css': {'tokens': 0, 'primitives': 0}}
        self.assertEqual(ui_standards.floor_issues(floor, current),
                         ['moved.css: the floor records a stylesheet that is not there; run --write-token-floor'])

    def test_recorded_floor_leaves_out_a_stylesheet_with_no_use(self):
        current = {'a.css': {'tokens': 2, 'primitives': 0}, 'b.css': {'tokens': 0, 'primitives': 1},
                   'none.css': {'tokens': 0, 'primitives': 0}}
        recorded = ui_standards.recorded_floor(current)
        self.assertEqual(sorted(recorded), ['a.css', 'b.css'])
        self.assertEqual(ui_standards.floor_issues(recorded, current), [])

    def test_kit_ceiling_counts_components_and_skips_tests_stories_and_other_files(self):
        files = ['kit/Button.tsx', 'kit/menu/Menu.jsx', 'kit/Button.test.tsx', 'kit/Button.spec.tsx',
                 'kit/Button.stories.tsx', 'kit/Button.browser.test.tsx', 'kit/index.ts', 'kit/Button.module.css',
                 'kitchen/Sink.tsx', 'other/Other.tsx']
        self.assertEqual(ui_standards.kit_counts(files, ('kit', 'gone')), {'kit': 2, 'gone': 0})

    def test_kit_ceiling_file_must_map_kits_to_whole_numbers(self):
        self.assertEqual(ui_standards.read_kit_ceiling('{"kit": 2}'), {'kit': 2})
        for text in ('[1]', '"kit"', '{"kit": "2"}', '{"kit": 1.5}', '{"kit": true}', '{"kit": -1}', '{"kit": null}',
                     '{'):
            with self.assertRaises(ValueError, msg=text):
                ui_standards.read_kit_ceiling(text)

    def test_raised_ceilings_names_each_value_above_its_limit_and_lets_a_new_kit_pass(self):
        self.assertEqual(ui_standards.raised_ceilings({'kit': 3, 'other': 2}, {'kit': 3, 'other': 1, 'new': 5}), [])
        self.assertEqual(ui_standards.raised_ceilings({'kit': 3}, {'kit': 4}),
                         ['kit: 4 is above the recorded ceiling of 3; a ceiling is never raised'])

    def test_kit_ceiling_fails_when_a_kit_grows_and_passes_when_one_is_below_its_ceiling(self):
        ceiling = {'kit': 3, 'other': 1}
        self.assertEqual(ui_standards.kit_issues(ceiling, {'kit': 3, 'other': 1}), [])
        self.assertEqual(ui_standards.kit_issues(ceiling, {'kit': 4, 'other': 0}), [
            'kit: 4 component files, the ceiling is 3; add shared components to library/ui/components instead'])

    def test_kit_ceiling_fails_on_an_unrecorded_kit_and_on_an_entry_that_names_no_kit(self):
        self.assertEqual(ui_standards.kit_issues({}, {'new': 1}), [
            'new: 1 component files, the ceiling is 0; add shared components to library/ui/components instead'])
        self.assertEqual(ui_standards.kit_issues({}, {'empty': 0}), [])
        self.assertEqual(ui_standards.kit_issues({'removed': 2}, {}), [
            'removed: the ceiling records a kit that is not in KIT_PATHS; run --write-kit-ceiling'])


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

    def kit(self, *components, ceiling=None):
        """Commit a kit with these component files and, when given, its recorded ceiling."""
        kit = self.root / ui_standards.KIT_PATHS[0]
        kit.mkdir(parents=True, exist_ok=True)
        for name in components:
            (kit / name).write_text('')
        if ceiling is not None:
            self.write_ceiling({ui_standards.KIT_PATHS[0]: ceiling, ui_standards.KIT_PATHS[1]: 0})
        self.git('add', '-A')
        self.commit('kit')
        return kit

    def write_ceiling(self, ceiling):
        file = self.root / ui_standards.KIT_FILE
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(json.dumps(ceiling))

    def test_kit_ceiling_counts_tracked_files_only(self):
        kit = self.kit('Button.tsx', ceiling=1)
        (kit / 'Scratch.tsx').write_text('')
        self.assertEqual(ui_standards.kit_ceiling('HEAD'), [])
        self.git('add', '-N', str(kit / 'Scratch.tsx'))
        self.assertEqual(len(ui_standards.kit_ceiling('HEAD')), 1)

    def test_kit_ceiling_fails_on_a_ceiling_raised_by_hand_above_the_base(self):
        kit = self.kit('Button.tsx', ceiling=1)
        (kit / 'Added.tsx').write_text('')
        self.git('add', '-A')
        self.write_ceiling({ui_standards.KIT_PATHS[0]: 2, ui_standards.KIT_PATHS[1]: 0})
        self.assertEqual(ui_standards.kit_ceiling('HEAD'), [
            f'{ui_standards.KIT_PATHS[0]}: 2 is above the recorded ceiling of 1; a ceiling is never raised'])

    def test_kit_ceiling_writer_refuses_a_raise_even_after_the_file_is_deleted(self):
        kit = self.kit('Button.tsx', ceiling=1)
        (kit / 'Added.tsx').write_text('')
        self.git('add', '-A')
        file = self.root / ui_standards.KIT_FILE
        self.assertEqual(len(ui_standards.kit_ceiling('HEAD', write=True)), 1)
        file.unlink()
        self.assertEqual(len(ui_standards.kit_ceiling('HEAD', write=True)), 1)
        self.assertFalse(file.exists())

    def test_kit_ceiling_writer_lowers_the_ceiling_and_records_a_kit_the_base_lacks(self):
        kit = self.kit('Button.tsx', 'Modal.tsx', ceiling=2)
        (kit / 'Modal.tsx').unlink()
        self.assertEqual(ui_standards.kit_ceiling('HEAD'), [])
        self.assertEqual(ui_standards.kit_ceiling('HEAD', write=True), [])
        self.assertEqual(ui_standards.kit_ceiling('HEAD'), [])
        self.assertEqual(json.loads((self.root / ui_standards.KIT_FILE).read_text())[ui_standards.KIT_PATHS[0]], 1)

    def test_kit_ceiling_passes_when_the_base_records_none_and_fails_when_the_file_is_missing(self):
        self.kit('Button.tsx')
        with self.assertRaises(ValueError):
            ui_standards.kit_ceiling('HEAD')
        self.write_ceiling({ui_standards.KIT_PATHS[0]: 1, ui_standards.KIT_PATHS[1]: 0})
        self.assertEqual(ui_standards.kit_ceiling('HEAD'), [])

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

    def test_imported_files_pass_only_when_added(self):
        (self.root / 'tooling').mkdir()
        (self.root / ui_standards.IMPORTED).write_text('# imported\napps/sites/src/Old.tsx\napps/sites/src/Old.css\n')
        component = self.path / 'Old.tsx'
        component.write_text("import styled from 'styled-components'\n")
        (self.path / 'Old.css').write_text('.root {}\n')
        (self.path / 'New.tsx').write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/New.tsx: new CSS-in-JS imports are not allowed'])
        self.commit('import')
        component.write_text("import styled from 'styled-components'\nimport { css } from '@emotion/css'\n")
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Old.tsx: new CSS-in-JS imports are not allowed'])

    def test_import_list_cannot_grow_or_be_deleted_after_it_exists(self):
        (self.root / 'tooling').mkdir()
        listing = self.root / ui_standards.IMPORTED
        listing.write_text('apps/sites/src/Old.tsx\n')
        (self.path / 'Old.tsx').write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        self.commit('import')
        listing.write_text('apps/sites/src/Old.tsx\napps/sites/src/New.tsx\n')
        (self.path / 'New.tsx').write_text("import styled from 'styled-components'\n")
        self.assertEqual(ui_standards.check('HEAD'), [
            f'{ui_standards.IMPORTED}: paths can\'t be added to the import list',
            'apps/sites/src/New.tsx: new CSS-in-JS imports are not allowed',
        ])
        listing.unlink()
        self.assertEqual(ui_standards.check('HEAD')[0], f'{ui_standards.IMPORTED}: the import list can\'t be deleted; empty it instead')
        listing.write_text('')
        (self.path / 'New.tsx').unlink()
        self.assertEqual(ui_standards.check('HEAD'), [])

    def test_listed_path_deleted_then_re_added_is_checked(self):
        (self.root / 'tooling').mkdir()
        (self.root / ui_standards.IMPORTED).write_text('apps/sites/src/Old.tsx\n')
        component = self.path / 'Old.tsx'
        component.write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        self.commit('import')
        self.git('rm', '-q', 'apps/sites/src/Old.tsx')
        self.commit('delete')
        self.path.mkdir(parents=True, exist_ok=True)
        component.write_text("import styled from 'styled-components'\n")
        self.git('add', '.')
        self.assertEqual(ui_standards.check('HEAD'), ['apps/sites/src/Old.tsx: new CSS-in-JS imports are not allowed'])

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
