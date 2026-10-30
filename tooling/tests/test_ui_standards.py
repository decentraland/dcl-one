import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import tooling.ui_standards as ui_standards

from tooling.ui_standards import is_ui_source, violations_in_added_lines, is_component_stylesheet


class UiStandardsTest(unittest.TestCase):
    def test_scope_includes_web_consumers(self):
        self.assertTrue(is_ui_source('apps/sites/src/Button.tsx'))
        self.assertTrue(is_ui_source('library/ui/components/Button.tsx'))
        self.assertTrue(is_ui_source('rendering/bevy-explorer/source/react-web/src/Button.tsx'))

    def test_scope_excludes_unrelated_code(self):
        self.assertFalse(is_ui_source('services/catalyst/src/main.rs'))
        self.assertFalse(is_ui_source('rendering/bevy-explorer/source/src/file.ts'))

    def test_forbids_added_css_in_js_import(self):
        self.assertTrue(violations_in_added_lines("import styled from 'styled-components'"))
        self.assertTrue(violations_in_added_lines("import { styled } from '@mui/material'"))
        self.assertFalse(violations_in_added_lines("import styles from './Button.module.css'"))

    def test_component_css_requires_matching_component(self):
        self.assertTrue(is_component_stylesheet('apps/sites/src/Button.css', {'apps/sites/src/Button.tsx'}))
        self.assertTrue(is_component_stylesheet('apps/sites/src/button.css', {'apps/sites/src/Button.tsx'}))
        self.assertFalse(is_component_stylesheet('apps/sites/src/Button.module.css', {'apps/sites/src/Button.tsx'}))
        self.assertFalse(is_component_stylesheet('apps/sites/src/global.css', {'apps/sites/src/Button.tsx'}))


class UiStandardsIntegrationTest(unittest.TestCase):
    def test_untracked_component_styles_fail_and_module_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
            subprocess.run(['git', '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                            '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-qm', 'initial'], cwd=root, check=True)
            component = root / 'apps/sites/src/Button.tsx'
            component.parent.mkdir(parents=True)
            component.write_text('export const Button = () => null\n')
            stylesheet = component.with_suffix('.css')
            stylesheet.write_text('.root {}\n')
            with patch.object(ui_standards, 'ROOT', root):
                self.assertTrue(ui_standards.check())
                stylesheet.rename(component.with_name('Button.module.css'))
                self.assertEqual(ui_standards.check(), [])


if __name__ == '__main__':
    unittest.main()
