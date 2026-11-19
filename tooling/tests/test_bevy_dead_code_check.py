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
    use bevy::{math::{Vec4, IVec2}, render::render_resource::ShaderType};

    /// see shader.wgsl
    #[derive(Clone, Copy, ShaderType)]
    #[repr(C)]
    pub(super) struct Uniform {
        // a field comment
        pub color: Vec4,
        pub origin: IVec2,
    }
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
            'spaced': '# [allow(dead_code)]\nfn f() {}\n',
            'spaced_inner': '#! [allow(dead_code)]\n',
            'block_comment_in_list': '#[allow(/* x */ dead_code)]\nfn f() {}\n',
            'line_comment_in_list': '#[allow(\n    // kept for later\n    dead_code,\n)]\nfn f() {}\n',
            'warn': '#[warn(dead_code)]\nfn f() {}\n',
            'shader_reason_outer': '#[expect(dead_code, reason = "the ShaderType derive emits a never-called check fn per field")]\nfn f() {}\n',
            'shader_reason_with_fn': SHADER_TYPE.replace('    use bevy', '    fn unused() {}\n    use bevy'),
            'shader_reason_with_impl': SHADER_TYPE.replace('    use bevy', '    impl Default for S { fn default() -> Self { S } }\n    use bevy'),
            'shader_reason_without_derive': SHADER_TYPE.replace('    use bevy', '    struct S {\n        a: u32,\n    }\n    use bevy'),
            'shader_reason_at_crate_root': '#![expect(dead_code, reason = "the ShaderType derive emits a never-called check fn per field")]\n#[derive(ShaderType)]\nstruct S {}\n',
        }
        for name, text in cases.items():
            with self.subTest(name):
                self.assertEqual(len(self.offenders({f'crates/x/src/{name}.rs': text})), 1)

    def test_reports_the_attribute_line(self):
        self.assertEqual(self.offenders({'src/main.rs': 'use a;\n\n#[allow(\n    dead_code\n)]\nfn f() {}\n'}),
                         [('src/main.rs', 3)])

    def test_vendor_local_harness_and_target_are_not_scanned(self):
        self.assertEqual(self.offenders({
            'vendor/basisu/src/lib.rs': '#[allow(dead_code)]\nfn f() {}\n',
            'local-harness/benchmark/src/main.rs': '#[allow(dead_code)]\nfn f() {}\n',
            'crates/x/target/debug/build/out.rs': '#[allow(dead_code)]\nfn f() {}\n',
        }), [])

    def test_comments_and_strings_do_not_count(self):
        self.assertEqual(self.offenders({'crates/x/src/lib.rs': (
            '// #[allow(dead_code)]\n/* #[allow(dead_code)] /* nested */ */\n'
            'const S: &str = "#[allow(dead_code)]";\nconst Q: char = \'"\';\nfn f<\'a>(s: &\'a str) {}\n')}), [])

    def test_missing_sources_raise(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(FileNotFoundError):
                check.offenders(Path(tmp))

    def test_build_script_is_scanned(self):
        self.assertEqual(self.offenders({'build.rs': '#[allow(dead_code)]\nfn f() {}\n'}), [('build.rs', 1)])


if __name__ == '__main__':
    unittest.main()
