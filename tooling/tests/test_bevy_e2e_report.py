import importlib.util
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location('bevy_e2e_report', Path(__file__).resolve().parents[1] / 'bevy-e2e-report.py')
report = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(report)

JUNIT = '''<?xml version="1.0" encoding="utf-8"?>
<testsuites>
  <testsuite name="shortcuts" tests="3" failures="1" skipped="1">
    <testcase name="KeyZ opens places" classname="shortcuts" time="1.2"/>
    <testcase name="KeyM opens map" classname="shortcuts" time="2.0">
      <failure type="AssertionError" message="map should be open after KeyM&#10;more">stack</failure>
    </testcase>
    <testcase name="X opens Events" classname="shortcuts" time="0">
      <skipped message="todo: the Bevy HUD binds no key to Events"/>
    </testcase>
  </testsuite>
  <testsuite name="camera" tests="1">
    <testcase name="takes a photo" classname="camera" time="0"><skipped message="no camera"/></testcase>
  </testsuite>
</testsuites>
'''


class ReportTests(unittest.TestCase):
    def test_counts_and_failures(self):
        counts, failures = report.summarize(JUNIT)
        self.assertEqual(dict(counts), {'passed': 1, 'failed': 1, 'todo': 1, 'skipped': 1})
        self.assertEqual(failures, [('shortcuts', 'KeyM opens map', 'map should be open after KeyM')])

    def test_section_with_a_failure(self):
        text = report.section(JUNIT, 'https://example.test/run/1')
        self.assertIn('InWorld-1%20failed-ff0000', text)
        self.assertIn('1 passed, 1 failed, 1 recorded gaps (todo), 1 skipped, of 4 scenarios', text)
        self.assertIn('- shortcuts: KeyM opens map — map should be open after KeyM', text)

    def test_section_all_passed(self):
        passed = JUNIT.replace('<failure type="AssertionError" message="map should be open after KeyM&#10;more">stack</failure>', '')
        text = report.section(passed, 'https://example.test/run/2')
        self.assertIn('InWorld-2%20passed-3fb950', text)
        self.assertNotIn('Failed:', text)

    def test_missing_report(self):
        text = report.missing('https://example.test/run/3', 'Visual')
        self.assertIn('Visual-No%20report-ff0000', text)


if __name__ == '__main__':
    unittest.main()
