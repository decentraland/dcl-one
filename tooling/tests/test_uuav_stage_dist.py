"""stage-dist.py copies a complete runtime set and refuses an incomplete one."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "uuav" / "stage-dist.py"
COMPLETE = ("uuav-helper", "libavcodec.so.63", "libavformat.so.63", "libavutil.so.61",
            "libswresample.so.7", "libswscale.so.10", "LICENSE-ffmpeg.txt")


def load():
    spec = importlib.util.spec_from_file_location("stage_dist", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class StageDistTests(unittest.TestCase):
    def setUp(self):
        self.module = load()
        self.source = tempfile.TemporaryDirectory()
        self.dist = Path(self.source.name)

    def tearDown(self):
        self.source.cleanup()

    def write_set(self, names=COMPLETE):
        for name in names:
            (self.dist / name).write_bytes(b"\x7fELF" + name.encode())
            if name.startswith("uuav-helper"):
                (self.dist / name).chmod(0o755)

    def test_stages_every_file_and_keeps_the_helper_executable(self):
        self.write_set()
        with tempfile.TemporaryDirectory() as out:
            self.assertEqual(self.module.stage(self.source.name, out), [])
            self.assertEqual(sorted(os.listdir(out)), sorted(COMPLETE))
            self.assertTrue(os.access(os.path.join(out, "uuav-helper"), os.X_OK))

    def test_windows_set_names(self):
        self.write_set(("uuav-helper.exe", "avcodec-63.dll", "avformat-63.dll", "avutil-61.dll",
                        "swresample-7.dll", "swscale-10.dll", "LICENSE-ffmpeg.txt"))
        with tempfile.TemporaryDirectory() as out:
            self.assertEqual(self.module.stage(self.source.name, out), [])

    def test_refuses_an_incomplete_set(self):
        self.write_set(("uuav-helper", "libavcodec.so.63"))
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(self.source.name, out)
            self.assertEqual(len(problems), 5)
            self.assertIn("no avformat library", problems[0])
            self.assertIn("no license text", problems[-1])
            self.assertEqual(os.listdir(out), [])

    def test_missing_set_is_reported(self):
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(os.path.join(self.source.name, "none"), out)
            self.assertEqual(len(problems), 1)
            self.assertIn("no runtime set", problems[0])


if __name__ == "__main__":
    unittest.main()
