"""The module layer check (services/catalyst/scripts/check-module-dag.sh) on small fixture trees."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "services/catalyst/scripts/check-module-dag.sh"
BASH = shutil.which("bash")

LAYERS = """[layers]
comms = 8
ops = 16
comms_e2e = 17

[allow]
"""


def has_ripgrep():
    if BASH is None:
        return False
    found = subprocess.run([BASH, "-c", "command -v rg"], capture_output=True, check=False)
    return found.returncode == 0


@unittest.skipUnless(has_ripgrep(), "the check needs bash and ripgrep")
class ModuleLayerTests(unittest.TestCase):
    def check(self, sources):
        """Runs the check on a tree whose crates have the given `src/lib.rs` texts."""
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            (workspace / "docs").mkdir()
            (workspace / "docs/module-layers.toml").write_text(LAYERS)
            (workspace / "library/plain").mkdir(parents=True)
            (workspace / "library/plain/Cargo.toml").write_text("[dependencies]\n")
            catalyst = workspace / "services/catalyst"
            for crate, text in sources.items():
                (catalyst / crate / "src").mkdir(parents=True)
                (catalyst / crate / "Cargo.toml").write_text(f'[package]\nname = "{crate}"\n')
                (catalyst / crate / "src/lib.rs").write_text(text)
            return subprocess.run(
                [BASH, SCRIPT.as_posix(), catalyst.as_posix()],
                capture_output=True, text=True, check=False,
            )

    def test_a_tool_on_the_top_layer_may_use_a_module_below(self):
        result = self.check({
            "comms": "pub fn serve() {}\n",
            "ops": "pub fn run() { catalyst_comms::serve(); }\n",
            "comms-e2e": "pub fn bot() { catalyst_comms::serve(); }\n",
        })
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("check-module-dag: ok", result.stdout)

    def test_a_module_that_uses_the_tool_is_refused(self):
        result = self.check({
            "comms": "pub fn serve() {}\n",
            "ops": "pub fn run() { catalyst_comms_e2e::bot(); }\n",
            "comms-e2e": "pub fn bot() { catalyst_comms::serve(); }\n",
        })
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("new back-edge ops -> comms_e2e", result.stdout)


if __name__ == "__main__":
    unittest.main()
