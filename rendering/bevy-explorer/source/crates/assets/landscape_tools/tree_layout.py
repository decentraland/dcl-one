"""Write the tree layout the explorer embeds from unity-explorer's two baked layouts.

unity-explorer's `StreamingAssets/GenesisTrees.bin` and `StreamingAssets/WorldsTrees.bin` place
the same instances in the same parcels and differ only in their scales, and each instance's
horizontal and vertical scales are equal. Both are little-endian: `i32 min_x, min_z, max_x,
max_z` (max exclusive), one `i32` start index per parcel in z-major order, `i32 count`, then
`count` 6-byte instances `{prototype, pos_x, pos_z, rot_y, scale_xz, scale_y}`.

This writes `src/assets/landscape/layout/landscape.trees`, which holds each instance once with
both realms' scales:

    b"TRL1", then i32 min_x, min_z, max_x, max_z (max exclusive), then one u8 instance count per
    parcel in z-major order, then the instances parcel by parcel, 6 bytes each:
    {prototype, pos_x, pos_z, rot_y, genesis_scale, worlds_scale}.

It stops if the two inputs differ in anything but their scales. Run from this crate's folder
with Python 3.10 or later and no other dependency:

    python landscape_tools/tree_layout.py GenesisTrees.bin WorldsTrees.bin
"""

import struct
import sys
from pathlib import Path

MAGIC = b"TRL1"
OUT = Path(__file__).resolve().parents[1] / "src/assets/landscape/layout/landscape.trees"


def read(path: Path) -> tuple[bytes, list[int], list[bytes]]:
    data = path.read_bytes()
    min_x, min_z, max_x, max_z = struct.unpack_from("<4i", data)
    cells = (max_x - min_x) * (max_z - min_z)
    starts = list(struct.unpack_from(f"<{cells}i", data, 16))
    at = 16 + cells * 4
    (count,) = struct.unpack_from("<i", data, at)
    at += 4
    if len(data) != at + count * 6:
        raise SystemExit(f"{path}: {len(data)} bytes, expected {at + count * 6}")
    ends = starts[1:] + [count]
    counts = [end - start for start, end in zip(starts, ends)]
    if any(n < 0 or n > 255 for n in counts):
        raise SystemExit(f"{path}: a parcel holds more than 255 instances or a start decreases")
    records = [data[at + i * 6 : at + i * 6 + 6] for i in range(count)]
    return data[:16], counts, records


def merge(genesis: Path, worlds: Path) -> bytes:
    header, counts, genesis_records = read(genesis)
    worlds_header, worlds_counts, worlds_records = read(worlds)
    if (header, counts) != (worlds_header, worlds_counts):
        raise SystemExit("the layouts' bounds or per-parcel counts differ")
    out = bytearray(MAGIC + header + bytes(counts))
    for g, w in zip(genesis_records, worlds_records):
        if g[:4] != w[:4]:
            raise SystemExit("the layouts differ in more than their scales")
        if g[4] != g[5] or w[4] != w[5]:
            raise SystemExit("an instance's horizontal and vertical scales differ")
        out += g[:4] + bytes((g[4], w[4]))
    return bytes(out)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: tree_layout.py GenesisTrees.bin WorldsTrees.bin")
    data = merge(Path(sys.argv[1]), Path(sys.argv[2]))
    OUT.write_bytes(data)
    print(f"{OUT.name}: {len(data)} bytes")


if __name__ == "__main__":
    main()
