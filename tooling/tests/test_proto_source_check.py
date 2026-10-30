"""Only library/protocol/proto and the listed legacy copies may hold decentraland protos."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('proto_source_check', Path(__file__).parents[1] / 'proto-source-check.py')
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)

DECENTRALAND = 'syntax = "proto3";\npackage decentraland.sdk.components;\nmessage PBThing {}\n'
OWN = 'syntax = "proto3";\npackage catalyst.comms.bus.v1;\nmessage RoomEvent {}\n'


def write(root, rel, text):
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


class ProtoSourceTest(unittest.TestCase):
    def test_source_allowlist_and_own_packages_pass_while_a_new_copy_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, 'library/protocol/proto/decentraland/sdk/components/thing.proto', DECENTRALAND)
            write(root, 'library/protocol/public/sdk-components.proto', 'syntax = "proto3";\npackage decentraland.sdk.components;\nimport public "decentraland/sdk/components/thing.proto";\n')
            write(root, 'library/dcl-scene-authoring/proto/decentraland/sdk/components/thing.proto', DECENTRALAND)
            write(root, 'services/catalyst/comms/src/protocol/bus/room_events.proto', OWN)
            write(root, 'apps/x/node_modules/@dcl/protocol/proto/decentraland/thing.proto', DECENTRALAND)
            write(root, 'library/protocol/proto/google/protobuf/descriptor.proto', 'syntax = "proto2";\npackage google.protobuf;\n')
            self.assertEqual(check.offenders(root), [])
            write(root, 'services/new-copy/decentraland/sdk/components/thing.proto', '﻿' + DECENTRALAND)
            write(root, 'rendering/bevy-explorer/source/crates/dcl_component/src/proto/decentraland/x.proto', DECENTRALAND)
            self.assertEqual(check.offenders(root), [
                'rendering/bevy-explorer/source/crates/dcl_component/src/proto/decentraland/x.proto',
                'services/new-copy/decentraland/sdk/components/thing.proto'])

    def test_the_repository_passes(self):
        self.assertEqual(check.offenders(check.ROOT), [])


if __name__ == '__main__':
    unittest.main()
