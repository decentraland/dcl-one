# EXT_meshopt_compression fixtures

Used by the tests in `src/meshopt_ext.rs`.

- `cube.glb`: a 24-vertex unit cube (f32 `POSITION`, `NORMAL`, `TEXCOORD_0`; 36 u16
  indices; one material with an external `cube.png` reference so gltfpack keeps the
  texcoords). Written by `make_cube.py` with no dependencies beyond the Python standard
  library: `python3 make_cube.py cube.glb`.
- `cube.meshopt.glb`: the same cube through gltfpack 1.0 (meshoptimizer), which is the
  encoder the pre-pass has to decode in production:

  ```
  gltfpack -c -vp 16 -vt 15 -vn 8 -kn -km -ke -tr -i cube.glb -o cube.meshopt.glb
  ```

  Positions are u16 with the dequantisation on the node transform, normals are oct-encoded
  i8, texcoords are u16 behind a `KHR_texture_transform` scale, and all four buffer views
  carry `EXT_meshopt_compression` streams (three `ATTRIBUTES`, one `TRIANGLES`).

Regenerate both when the generator changes; the test compares the decoded geometry against
`cube.glb`, so the two must come from the same run.
