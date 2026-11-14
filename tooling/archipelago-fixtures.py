#!/usr/bin/env python3
"""Check frozen corpora offline; regenerate only with explicit authenticated inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
STATS = "services/catalyst/comms/src/stats/"
WIRE = "services/catalyst/comms/src/archipelago/ws/"
RECOVERY = "services/catalyst/tests/fixtures/"
UPSTREAM = "64dc515987175c679e4009fe557ea60fdf50bc6c"
# Updates require reviewing source identity and the complete behavioral corpus.
PINS = {
    STATS + "source-runtime-lock.json": "4b2f09f741e020cb1d50a03f974f023c209d749a3212aeb76bd17ab2b8ecc1e5",
    STATS + "source-fixtures.json": "26e1ab30bb4576f5092b33130bb641820de9835edac2dad81657a773d5820311",
    STATS + "generate-source-fixtures.cjs": "039e91b5b2d3ff4b1822cb6b310f08bf030152d8651c8350f502f843f4c1b734",
    WIRE + "legacy-fixtures.json": "dbbca5b4e4ed135c50d95b5703cc9e96f1cea18b55460ddea7b2c35927c63edc",
    WIRE + "generate-legacy-fixtures.cjs": "1647e14b76d3a8e8461a69ccab896288d915099124a178c99725a630065236c8",
    RECOVERY + "pulse_retained_recovery.json": "dd21fa52314a26dfcaf7602ed010327f44a8735eb70692bf924ef09fc53e45f9",
    RECOVERY + "pulse_selected_recovery.json": "3bdbe9bc35547e34fbce9dfe41db48e54afa258a038ed1ff81051c40bff9633f",
}
NODE_WINDOWS_X64_SHA256 = "3331e1ffe19874215472217c5e94f5a0c6d8e18c4ac7111d3937aa0ad5e9b4a5"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def lf_bytes(path):
    return path.read_bytes().replace(b"\r\n", b"\n")


def verify_digest(path, expected, *, normalize=False):
    data = lf_bytes(path) if normalize else path.read_bytes()
    require(hashlib.sha256(data).hexdigest() == expected, f"fixture integrity mismatch: {path}")
    return data


def member_path(root, member):
    relative = PurePosixPath(member)
    require(member and not relative.is_absolute() and ".." not in relative.parts
            and "\\" not in member and ":" not in member, f"unsafe fixture member: {member}")
    result = root.joinpath(*relative.parts).resolve()
    require(result.is_relative_to(root.resolve()), f"fixture member escapes root: {member}")
    return result


def check_selected_recovery(document):
    for group, count in [("intent_vectors", 19), ("reply_vectors", 12),
                         ("identity_boundary_vectors", 15), ("encoded_reply_boundaries", 2)]:
        cases = document[group]
        require(len(cases) == count and len({case["id"] for case in cases}) == count,
                f"lost/duplicate selected recovery cases: {group}")
        for case in cases:
            require(len(bytes.fromhex(case["wire_hex"])) == case["length_bytes"],
                    f"incomplete selected recovery wire bytes: {case['id']}")
    identities = document["identity_boundary_vectors"]
    expected = {"1", "9007199254740991", "9007199254740993", "9223372036854775807", "18446744073709551615"}
    for origin in (1, 2, 3):
        require({case["identity_decimal"] for case in identities if case["origin"] == origin} == expected,
                f"lost exact selected identity boundaries: {origin}")
    for case, size in zip(document["encoded_reply_boundaries"], (65536, 65537), strict=True):
        require(case["length_bytes"] == case["expected_size_bytes"] == size
                and case["construction"]["pending_count"] == 1,
                "selected recovery cap must retain the complete pending reply")


def check(root=ROOT):
    data = {name: verify_digest(root / name, digest, normalize=True) for name, digest in PINS.items()}
    check_selected_recovery(json.loads(data[RECOVERY + "pulse_selected_recovery.json"]))
    lock = json.loads(data[STATS + "source-runtime-lock.json"])
    handlers = json.loads(data[STATS + "source-fixtures.json"])
    decoder = json.loads(data[WIRE + "legacy-fixtures.json"])
    require(lock["revision"] == UPSTREAM and lock["node"] == "v24.15.0", "wrong fixture source/runtime pin")
    require(len(lock["sources"]) == 11 and len(lock["packages"]) == 21
            and sum(len(package["files"]) for package in lock["packages"].values()) == 292,
            "lost source/runtime manifest entries")
    provenance = handlers["provenance"]
    require(provenance["revision"] == UPSTREAM and provenance["node"] == lock["node"]
            and provenance["sources"] == provenance["sourcesLf"] == lock["sources"],
            "handler source/runtime provenance mismatch")
    names = [case["name"] for case in handlers["cases"]]
    require(names == ["cold", "populated", "disconnect_reinsert_nonfinite_stale", "future_clock_no_expiry", "empty_topology"],
            "lost/duplicate handler corpus cases")
    require(sum(len(case["reads"]) for case in handlers["cases"]) == 85 and len(handlers["wire"]) == 8,
            "lost handler reads/wire corpus")
    require(decoder["source"] == "decentraland/archipelago-workers@" + UPSTREAM
            and len(decoder["cases"]) == 566, "lost decoder cases/source identity")
    protocol = lock["packages"]["@dcl/protocol@1.0.0-35089025179.commit-5810768"]["files"]
    reader = lock["packages"]["protobufjs@7.2.4"]["files"]
    for member, digest in decoder["pins"].items():
        if member.startswith("@dcl/protocol/"):
            expected = protocol[member.removeprefix("@dcl/protocol/")]
        else:
            require(member.startswith("protobufjs/"), "unknown decoder dependency pin")
            expected = reader[member.removeprefix("protobufjs/")]
        require(digest == expected, f"decoder/runtime pin mismatch: {member}")
    print("Archipelago fixture integrity: 5 handler states, 85 reads, 8 wire packets, 566 decoder cases; 21 runtime roots/292 members", flush=True)
    print("Pulse selected recovery integrity: 19 intents, 12 replies, 15 exact identities, complete 65536/65537-byte pending replies", flush=True)
    return lock


# Both generators run through the same pre-execution dependency guard, including
# the decoder's transitive dependencies. Unknown package members fail closed.
PRELOAD = r"""
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');
const Module = require('node:module');
const lock = JSON.parse(fs.readFileSync(process.env.ARCHIPELAGO_FIXTURE_LOCK));
const generator = path.resolve(process.argv[1]);
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
for (const extension of ['.js', '.json', '.node', '.cjs']) {
  const original = Module._extensions[extension] || Module._extensions['.js'];
  Module._extensions[extension] = (module, filename) => {
    if (path.resolve(filename) === generator) return original(module, filename);
    let folder = path.dirname(filename), pin;
    while (true) {
      const manifest = path.join(folder, 'package.json');
      if (fs.existsSync(manifest)) {
        const bytes = fs.readFileSync(manifest), identity = JSON.parse(bytes);
        pin = lock.packages[`${identity.name}@${identity.version}`];
        if (pin) {
          if (digest(bytes) !== pin.files['package.json']) throw new Error('Unpinned package manifest: ' + filename);
          break;
        }
      }
      const parent = path.dirname(folder);
      if (parent === folder) throw new Error('Unpinned dependency: ' + filename);
      folder = parent;
    }
    const member = path.relative(folder, filename).replaceAll('\\', '/');
    if (digest(fs.readFileSync(filename)) !== pin.files[member]) throw new Error('Unpinned dependency bytes: ' + filename);
    return original(module, filename);
  };
}
"""


def regenerate(root, upstream, node, node_path, output_dir):
    lock = check(root)
    require(sys.platform == "win32", "authenticated regeneration currently supports official Node24.15.0 Windows x64 only")
    verify_digest(node, NODE_WINDOWS_X64_SHA256)
    require(node_path and all(entry.strip() and Path(entry).is_dir() for entry in node_path.split(os.pathsep)),
            "regeneration requires explicit existing NODE_PATH directories")
    # Validate all declared application bytes before starting any runtime.
    for member, digest in lock["sources"].items():
        verify_digest(member_path(upstream, member), digest, normalize=True)
    output_dir.mkdir(parents=True, exist_ok=True)
    environ = dict(os.environ, NODE_PATH=node_path, NODE_DISABLE_COMPILE_CACHE="1",
                   ARCHIPELAGO_FIXTURE_LOCK=str(root / (STATS + "source-runtime-lock.json")))
    environ.pop("NODE_OPTIONS", None)
    with tempfile.TemporaryDirectory(prefix="archipelago-fixtures-", dir=output_dir) as temporary:
        preload = Path(temporary) / "verify-dependencies.cjs"
        preload.write_text(PRELOAD, encoding="utf-8")
        for generator, arguments, corpus in [
            (STATS + "generate-source-fixtures.cjs", [str(upstream)], STATS + "source-fixtures.json"),
            (WIRE + "generate-legacy-fixtures.cjs", [], WIRE + "legacy-fixtures.json"),
        ]:
            generated = Path(temporary) / Path(corpus).name
            command = [str(node), "--require", str(preload), str(root / generator), *arguments, str(generated)]
            result = subprocess.run(command, cwd=root, env=environ, text=True,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60)
            print(result.stdout, flush=True)
            require(result.returncode == 0, f"fixture generator failed ({result.returncode}): {generator}")
            verify_digest(generated, PINS[corpus], normalize=True)
            require(lf_bytes(generated) == lf_bytes(root / corpus), f"regenerated corpus differs: {corpus}")
            print(f"authenticated regeneration matches: {corpus}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="mode", required=True)
    commands.add_parser("check", help="offline corpus and provenance integrity")
    generation = commands.add_parser("regenerate", help="explicit authenticated offline oracle; never downloads or rewrites expectations")
    generation.add_argument("--upstream", type=Path, required=True)
    generation.add_argument("--node", type=Path, required=True)
    generation.add_argument("--node-path", required=True)
    generation.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.mode == "check":
        check()
    else:
        regenerate(ROOT, args.upstream.resolve(), args.node.resolve(), args.node_path, args.output_dir.resolve())


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, json.JSONDecodeError, subprocess.TimeoutExpired) as error:
        sys.exit(f"Archipelago fixture guard: {error}")
