"""Write a minimal unit-cube GLB (24 vertices: POSITION f32, NORMAL f32, TEXCOORD_0 f32;
36 u16 indices; one node named "cube" so gltfpack -kn keeps it) to argv[1]."""
import json
import struct
import sys

faces = [
    # (normal, [4 corner positions], uvs)
    ((0, 0, 1), [(-0.5, -0.5, 0.5), (0.5, -0.5, 0.5), (0.5, 0.5, 0.5), (-0.5, 0.5, 0.5)]),
    ((0, 0, -1), [(0.5, -0.5, -0.5), (-0.5, -0.5, -0.5), (-0.5, 0.5, -0.5), (0.5, 0.5, -0.5)]),
    ((1, 0, 0), [(0.5, -0.5, 0.5), (0.5, -0.5, -0.5), (0.5, 0.5, -0.5), (0.5, 0.5, 0.5)]),
    ((-1, 0, 0), [(-0.5, -0.5, -0.5), (-0.5, -0.5, 0.5), (-0.5, 0.5, 0.5), (-0.5, 0.5, -0.5)]),
    ((0, 1, 0), [(-0.5, 0.5, 0.5), (0.5, 0.5, 0.5), (0.5, 0.5, -0.5), (-0.5, 0.5, -0.5)]),
    ((0, -1, 0), [(-0.5, -0.5, -0.5), (0.5, -0.5, -0.5), (0.5, -0.5, 0.5), (-0.5, -0.5, 0.5)]),
]
uvs = [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)]

positions, normals, texcoords, indices = [], [], [], []
for n, corners in faces:
    base = len(positions)
    for i, p in enumerate(corners):
        # a small non-uniform wobble so no two vertices share a coordinate pattern
        positions.append((p[0] * 1.0, p[1] * 1.25, p[2] * 0.75))
        normals.append(n)
        texcoords.append(uvs[i])
    indices += [base, base + 1, base + 2, base, base + 2, base + 3]

bin_ = bytearray()
def put(fmt, rows):
    off = len(bin_)
    for r in rows:
        bin_.extend(struct.pack("<" + fmt * len(r), *r))
    while len(bin_) % 4:
        bin_.append(0)
    return off, len(bin_) - off - (len(bin_) - off) % 4

pos_off, _ = put("f", positions)
pos_len = 24 * 12
nrm_off, _ = put("f", normals)
nrm_len = 24 * 12
uv_off, _ = put("f", texcoords)
uv_len = 24 * 8
idx_off, _ = put("H", [(i,) for i in indices])
idx_len = 36 * 2

mins = [min(p[c] for p in positions) for c in range(3)]
maxs = [max(p[c] for p in positions) for c in range(3)]

doc = {
    "asset": {"version": "2.0", "generator": "dcl-one make_cube.py"},
    "scene": 0,
    "scenes": [{"nodes": [0]}],
    "nodes": [{"name": "cube", "mesh": 0}],
    "meshes": [{"name": "cube", "primitives": [{
        "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
        "indices": 3,
        "material": 0,
    }]}],
    "materials": [{"name": "grey", "pbrMetallicRoughness": {
        "baseColorFactor": [0.6, 0.6, 0.6, 1.0],
        "baseColorTexture": {"index": 0}}}],
    # an external texture reference keeps TEXCOORD_0 alive through gltfpack (-tr keeps the path)
    "textures": [{"source": 0}],
    "images": [{"uri": "cube.png"}],
    "buffers": [{"byteLength": len(bin_)}],
    "bufferViews": [
        {"buffer": 0, "byteOffset": pos_off, "byteLength": pos_len, "target": 34962},
        {"buffer": 0, "byteOffset": nrm_off, "byteLength": nrm_len, "target": 34962},
        {"buffer": 0, "byteOffset": uv_off, "byteLength": uv_len, "target": 34962},
        {"buffer": 0, "byteOffset": idx_off, "byteLength": idx_len, "target": 34963},
    ],
    "accessors": [
        {"bufferView": 0, "componentType": 5126, "count": 24, "type": "VEC3", "min": mins, "max": maxs},
        {"bufferView": 1, "componentType": 5126, "count": 24, "type": "VEC3"},
        {"bufferView": 2, "componentType": 5126, "count": 24, "type": "VEC2"},
        {"bufferView": 3, "componentType": 5123, "count": 36, "type": "SCALAR"},
    ],
}

js = json.dumps(doc, separators=(",", ":")).encode()
while len(js) % 4:
    js += b" "
total = 12 + 8 + len(js) + 8 + len(bin_)
out = bytearray()
out += b"glTF" + struct.pack("<II", 2, total)
out += struct.pack("<I", len(js)) + b"JSON" + js
out += struct.pack("<I", len(bin_)) + b"BIN\0" + bytes(bin_)
with open(sys.argv[1], "wb") as f:
    f.write(out)
print(len(out), "bytes")
