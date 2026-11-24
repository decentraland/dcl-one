"""Write the Genesis City occupancy snapshots the explorer embeds.

Reads the `WorldManifest.json` Decentraland's places pipeline publishes for each environment (the
file unity-explorer's `WorldManifestProvider` downloads) and writes
`crates/scene_runner/src/genesis_occupancy/<environment>.occupancy`:

    b"OCC1", then min x, min y, width and height as little-endian i32, then one byte per parcel,
    rows from min y upwards, x ascending: 0 unlisted, 1 empty, 2 occupied (roads included).

Run from anywhere with Python 3.10 or later and no other dependency:

    python crates/assets/landscape_tools/genesis_occupancy.py
"""

import json
import struct
import urllib.request
from pathlib import Path

MANIFESTS = {
    "org": "https://places-dcf8abb.s3.amazonaws.com/WorldManifest.json",
    "zone": "https://places-e22845c.s3.us-east-1.amazonaws.com/WorldManifest.json",
}
OUT_DIR = Path(__file__).resolve().parents[2] / "scene_runner" / "src" / "genesis_occupancy"
EMPTY = 1
OCCUPIED = 2


def parcels(manifest: dict, key: str) -> list[tuple[int, int]]:
    out = []
    for pointer in manifest.get(key) or []:
        x, y = pointer.split(",")
        out.append((int(x), int(y)))
    return out


def snapshot(manifest: dict) -> bytes:
    empty = parcels(manifest, "empty")
    occupied = parcels(manifest, "occupied") + parcels(manifest, "roads")
    if not occupied:
        raise SystemExit("the manifest lists no occupied parcel")
    every = empty + occupied
    min_x = min(x for x, _ in every)
    min_y = min(y for _, y in every)
    width = max(x for x, _ in every) - min_x + 1
    height = max(y for _, y in every) - min_y + 1
    cells = bytearray(width * height)
    # occupied wins over a parcel also listed empty
    for value, listed in ((EMPTY, empty), (OCCUPIED, occupied)):
        for x, y in listed:
            cells[(y - min_y) * width + (x - min_x)] = value
    return b"OCC1" + struct.pack("<4i", min_x, min_y, width, height) + bytes(cells)


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for environment, url in MANIFESTS.items():
        with urllib.request.urlopen(url, timeout=60) as response:
            manifest = json.load(response)
        data = snapshot(manifest)
        path = OUT_DIR / f"{environment}.occupancy"
        path.write_bytes(data)
        print(f"{path.name}: {len(data)} bytes from {url}")


if __name__ == "__main__":
    main()
