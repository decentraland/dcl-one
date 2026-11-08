"""stage-dist.py copies a complete runtime set and refuses an incomplete or mixed one."""
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
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

    def test_macos_set_names(self):
        self.write_set(("uuav-helper", "libavcodec.63.dylib", "libavformat.63.dylib",
                        "libavutil.61.dylib", "libswresample.7.dylib", "libswscale.10.dylib",
                        "LICENSE-ffmpeg.txt"))
        with tempfile.TemporaryDirectory() as out:
            self.assertEqual(self.module.stage(self.source.name, out), [])

    def test_refuses_two_majors_of_one_library(self):
        self.write_set(COMPLETE + ("libavcodec.so.62",))
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(self.source.name, out)
            self.assertEqual(len(problems), 1)
            self.assertIn("more than one avcodec library: libavcodec.so.62, libavcodec.so.63", problems[0])
            self.assertEqual(os.listdir(out), [])

    def test_refuses_a_set_of_two_platforms(self):
        self.write_set(COMPLETE + ("avcodec-63.dll",))
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(self.source.name, out)
            self.assertEqual(len(problems), 1)
            self.assertIn("libraries of more than one platform", problems[0])
            self.assertEqual(os.listdir(out), [])

    def test_refuses_a_name_that_only_contains_a_library_name(self):
        names = tuple(n for n in COMPLETE if n != "libavcodec.so.63")
        for stray in ("libavcodec.so", "libavcodec.so.63.11.100", "avcodec.txt", "libavcodec.63.so"):
            with self.subTest(stray=stray), tempfile.TemporaryDirectory() as source:
                for name in names + (stray,):
                    (Path(source) / name).write_bytes(b"x")
                with tempfile.TemporaryDirectory() as out:
                    problems = self.module.stage(source, out)
                    self.assertEqual(len(problems), 1)
                    self.assertIn("no avcodec library", problems[0])

    def test_refuses_the_helper_of_another_platform(self):
        self.write_set(tuple(n for n in COMPLETE if n != "uuav-helper") + ("uuav-helper.exe",))
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(self.source.name, out)
            self.assertEqual(len(problems), 2)
            self.assertIn("no uuav-helper", problems[0])
            self.assertIn("uuav-helper.exe belongs to another platform", problems[1])

    def test_digests_are_appended_in_sha256sum_format(self):
        self.write_set()
        with tempfile.TemporaryDirectory() as out:
            summary = Path(out) / "summary.md"
            summary.write_text("### set\n")
            subprocess.run([sys.executable, str(SCRIPT), self.source.name, os.path.join(out, "set"),
                            "--digests", str(summary)], check=True, capture_output=True)
            lines = summary.read_text().splitlines()
            self.assertEqual(lines[:2] + lines[-1:], ["### set", "```", "```"])
            expected = [f"{hashlib.sha256((self.dist / n).read_bytes()).hexdigest()}  {n}"
                        for n in sorted(COMPLETE)]
            self.assertEqual(lines[2:-1], expected)

    def test_missing_set_is_reported(self):
        with tempfile.TemporaryDirectory() as out:
            problems = self.module.stage(os.path.join(self.source.name, "none"), out)
            self.assertEqual(len(problems), 1)
            self.assertIn("no runtime set", problems[0])


if __name__ == "__main__":
    unittest.main()
