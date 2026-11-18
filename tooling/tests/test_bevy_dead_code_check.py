"""Only the ShaderType derive's expect may suppress dead code in the Explorer's own sources."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('bevy_dead_code_check', Path(__file__).parents[1] / 'bevy-dead-code-check.py')
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)

SHADER_TYPE = '''mod decl {
    #![expect(
        dead_code,
        reason = "the ShaderType derive emits a never-called check fn per field"
    )]
    use bevy::render::render_resource::ShaderType;
}
'''


def write(root, rel, text):
    path = root / check.SOURCE / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


class DeadCodeCheckTest(unittest.TestCase):
    def offenders(self, files):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel, text in files.items():
                write(root, rel, text)
            return [(rel.split(check.SOURCE + '/')[1], line) for rel, line, _ in check.offenders(root)]

    def test_shader_type_expect_and_other_lints_pass(self):
        self.assertEqual(self.offenders({
            'crates/visuals/src/stars.rs': SHADER_TYPE,
            'crates/nft/src/lib.rs': '#[allow(unused_imports)]\nuse a::b;\n#[allow(clippy::too_many_arguments)]\nfn f() {}\n',
            'src/lib.rs': '#[cfg(feature = "html")]\nfn g() {}\n',
        }), [])

    def test_each_dead_code_suppression_fails(self):
        cases = {
            'allow': '#[allow(dead_code)]\nfn f() {}\n',
            'expect_unused': 'trait T {\n    #[expect(unused)]\n    fn f(&self);\n}\n',
            'list': '#[allow(clippy::all, dead_code)]\nfn f() {}\n',
            'warnings': '#![allow(warnings)]\n',
            'cfg_attr': '#[cfg_attr(not(feature = "html"), expect(unused))]\nfn f() {}\n',
            'split': '#[allow(\n    dead_code,\n)]\nfn f() {}\n',
            'bare_expect': '#[expect(dead_code)]\nstruct S;\n',
            'other_reason': '#[expect(dead_code, reason = "kept for later")]\nstruct S;\n',
            'shader_reason_on_unused': '#[expect(unused, reason = "the ShaderType derive emits a never-called check fn per field")]\nstruct S;\n',
        }
        for name, text in cases.items():
            with self.subTest(name):
                self.assertEqual(len(self.offenders({f'crates/x/src/{name}.rs': text})), 1)

    def test_reports_the_attribute_line(self):
        self.assertEqual(self.offenders({'local-harness/src/main.rs': 'use a;\n\n#[allow(\n    dead_code\n)]\nfn f() {}\n'}),
                         [('local-harness/src/main.rs', 3)])

    def test_vendor_and_target_are_not_scanned(self):
        self.assertEqual(self.offenders({
            'vendor/basisu/src/lib.rs': '#[allow(dead_code)]\nfn f() {}\n',
            'crates/x/target/debug/build/out.rs': '#[allow(dead_code)]\nfn f() {}\n',
        }), [])

    def test_build_script_is_scanned(self):
        self.assertEqual(self.offenders({'build.rs': '#[allow(dead_code)]\nfn f() {}\n'}), [('build.rs', 1)])


if __name__ == '__main__':
    unittest.main()
