"""The nixpkgs updater rewrites exactly the flake.nix pin and reads the locked revision back."""
import importlib.util
import json
from pathlib import Path
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('nixpkgs_update', ROOT / 'tooling/nixpkgs-update.py')
update = importlib.util.module_from_spec(spec)
spec.loader.exec_module(update)

OLD = '7a6522c868711d06e3a520bbcdfafe9806f4337f'
NEW = 'f9a75f41be3a74eab55d3a7679d34ad6cc0da977'
FLAKE = textwrap.dedent(f'''
    inputs = {{
      nixpkgs.url = "github:NixOS/nixpkgs/{OLD}";
      rust-overlay = {{
        url = "github:oxalica/rust-overlay/42076d1c6a0fb405ed04f9a674e4a8f69de62ba3";
        inputs.nixpkgs.follows = "nixpkgs";
      }};
    }};
''')
LOCK = json.dumps({'nodes': {
    'root': {'inputs': {'nixpkgs': 'nixpkgs'}},
    'nixpkgs': {'locked': {'rev': NEW, 'lastModified': 1793876309}},
}})


class NixpkgsUpdateTest(unittest.TestCase):
    def test_reads_the_pin(self):
        self.assertEqual(update.current_pin(FLAKE), OLD)

    def test_rewrites_only_the_nixpkgs_pin(self):
        rewritten = update.replace_pin(FLAKE, NEW)
        self.assertEqual(rewritten, FLAKE.replace(OLD, NEW))
        self.assertIn('rust-overlay/42076d1c6a0fb405ed04f9a674e4a8f69de62ba3', rewritten)

    def test_rejects_a_flake_without_a_commit_pin(self):
        with self.assertRaises(ValueError):
            update.current_pin(FLAKE.replace(OLD, 'nixos-unstable'))

    def test_reads_the_locked_revision_and_date(self):
        self.assertEqual(update.locked_nixpkgs(LOCK), (NEW, '2026-11-05'))

    def test_repository_pin_matches_its_lock(self):
        pin = update.current_pin((ROOT / 'flake.nix').read_text(encoding='utf-8'))
        self.assertEqual(update.locked_nixpkgs((ROOT / 'flake.lock').read_text(encoding='utf-8'))[0], pin)


if __name__ == '__main__':
    unittest.main()
