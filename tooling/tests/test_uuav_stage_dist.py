"""stage-dist.py copies a complete runtime set and refuses LFS pointers."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "uuav" / "stage-dist.py"


def load():
    spec = importlib.util.spec_from_file_location("stage_dist", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class StageDistTests(unittest.TestCase):
    def setUp(self):
        self.module = load()
        self.repo = tempfile.TemporaryDirectory()
        self.dist = Path(self.repo.name) / "library" / "uuav" / "dist" / "linux-x86_64"
        self.dist.mkdir(parents=True)

    def tearDown(self):
        self.repo.cleanup()

    def test_stages_every_file_and_keeps_the_helper_executable(self):
        helper = self.dist / "uuav-helper"
        helper.write_bytes(b"\x7fELF")
        helper.chmod(0o755)
        (self.dist / "libavcodec.so.62").write_bytes(b"\x7fELF")
        (self.dist / "uuav-linux-x86_64.sha256").write_text("ignored report\n")
        with tempfile.TemporaryDirectory() as out:
            self.assertEqual(self.module.stage(self.repo.name, "linux-x86_64", out), [])
            staged = sorted(os.listdir(out))
            self.assertEqual(staged, ["libavcodec.so.62", "uuav-helper"])
            self.assertTrue(os.access(os.path.join(out, "uuav-helper"), os.X_OK))

    def test_refuses_an_lfs_pointer(self):
        (self.dist / "uuav-helper").write_text(
            "version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n")
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(self.repo.name, "linux-x86_64", out)
            self.assertEqual(len(problems), 1)
            self.assertIn("LFS pointer", problems[0])
            self.assertEqual(os.listdir(out), [])

    def test_missing_target_is_reported(self):
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(self.repo.name, "windows-x86_64", out)
            self.assertEqual(len(problems), 1)
            self.assertIn("no runtime set", problems[0])


if __name__ == "__main__":
    unittest.main()
