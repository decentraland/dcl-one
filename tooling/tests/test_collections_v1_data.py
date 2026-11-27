"""Offline checks of tooling/collections-v1-data.py: what it reads of the TypeScript sources and
what it refuses of an RPC endpoint. Run with `python3 -m unittest tooling/tests/test_collections_v1_data.py`."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "collections_v1_data", Path(__file__).resolve().parents[1] / "collections-v1-data.py"
)
DATA = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DATA)


def abi(text):
    """`text` ABI-encoded as a function's single `string` return value."""
    raw = text.encode()
    padded = raw + b"\0" * (-len(raw) % 32)
    return "0x" + (32).to_bytes(32, "big").hex() + len(raw).to_bytes(32, "big").hex() + padded.hex()


class WearablesTest(unittest.TestCase):
    def test_only_the_export_arrays_live_entries_are_read(self):
        source = """import { Wearable } from './Wearable'

// new Wearable('hat', 'Commented', 'a line comment', 'hat', 'rare', ['BaseMale']),
export let sample: Wearable[] = [
  /* new Wearable('hat', 'Blocked', 'a block comment', 'hat', 'rare', ['BaseMale']), */
  new Wearable('hat', 'Live Hat', "it's // not a comment", 'hat', 'epic', ['BaseFemale', 'BaseMale']),
]

export let other = [new Wearable('outside', 'Outside', '', 'hat', 'rare', [])]
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.ts"
            path.write_text(source)
            self.assertEqual(
                DATA.wearables(path, "sample"),
                [{
                    "id": "hat",
                    "name": "Live Hat",
                    "description": "it's // not a comment",
                    "category": "hat",
                    "rarity": "epic",
                    "bodyShapes": ["BaseFemale", "BaseMale"],
                }],
            )


class SymbolsTest(unittest.TestCase):
    ADDRESSES = ["0x" + "a1" * 20, "0x" + "b2" * 20]

    def test_symbols_are_read_at_the_heads_block(self):
        calls = []

        def post(_rpc, payload):
            calls.append(payload)
            if isinstance(payload, dict):
                return {"jsonrpc": "2.0", "id": 0, "result": "0x10"}
            return [{"jsonrpc": "2.0", "id": c["id"], "result": abi(f"DCL{c['id']}")} for c in reversed(payload)]

        with mock.patch.object(DATA, "rpc_post", post):
            block, symbols = DATA.eth_call_symbols("http://rpc.invalid", self.ADDRESSES)
        self.assertEqual(block, 16)
        self.assertEqual(symbols, {self.ADDRESSES[0]: "DCL0", self.ADDRESSES[1]: "DCL1"})
        self.assertTrue(all(c["params"][1] == "0x10" for c in calls[1]))

    def test_a_refused_batch_names_the_endpoints_error(self):
        def post(_rpc, payload):
            if isinstance(payload, dict):
                return {"jsonrpc": "2.0", "id": 0, "result": "0x10"}
            return {"jsonrpc": "2.0", "id": None, "error": {"code": -32005, "message": "rate limited"}}

        with mock.patch.object(DATA, "rpc_post", post), self.assertRaises(SystemExit) as refused:
            DATA.eth_call_symbols("http://rpc.invalid", self.ADDRESSES)
        self.assertIn("rate limited", str(refused.exception))

    def test_a_result_that_is_not_a_string_is_refused(self):
        # Empty, too short, a length past the data, not hex, an empty string.
        for result in ["0x", "0x1234", abi("DCL")[:-64], "nothex", abi("")]:
            with self.assertRaises(SystemExit, msg=result):
                DATA.abi_string(result, "symbol()")
        self.assertEqual(DATA.abi_string(abi("DCLHLWN2019"), "symbol()"), "DCLHLWN2019")


if __name__ == "__main__":
    unittest.main()
