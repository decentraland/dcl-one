# ADR: Bevy variants manifest

- Status: Proposed
- Date: 2026-11-09

## Context

A scene's content files are what its creator deployed: PNG and JPEG textures,
GLBs with their images embedded, sometimes a creator-made UASTC KTX2. The
Explorer turns each texture into a GPU format after download (BC7 on desktop)
and caches the result, which costs time on every cold visit, and GLBs travel
uncompressed. The asset-bundle generator (abgen) already reads every entity and
can produce GPU-ready files ahead of time.

The alternative of one pack per entity was measured on Genesis Plaza and
rejected: every byte saved came from BC7 textures, meshopt meshes and brotli,
which all work per file; a pack adds nothing to size, loses per-file caching
(a redeploy invalidates the whole pack) and had cold builds of several minutes.

## Decision

abgen publishes, per scene entity, a manifest that maps each original content
CID to GPU-ready variants, each with the path it is served at and the CID of its
bytes. The Explorer reads the manifest when present and loads a variant instead
of the original; without a manifest, without a usable variant or on any failure
it loads the deployment's own file. The service can be switched off at any time
and Decentraland keeps working.

abgen names a variant by its inputs (the original's CID, how the scene uses
it, the encoder's generation), so it can find an existing variant before doing
any work and never encodes the same file twice across scenes or redeploys. A
path's bytes are fixed for the life of its generation; a new encoder
generation, or a forced rebuild, moves variants to new paths and regenerates
the manifests that named the old ones. The Explorer needs no version of its
own: it fetches what the manifest names and checks the bytes against the CID.

### Routes (served by abgen)

- `GET {base}/bevy/manifest/{entityId}.json`: `200` with the manifest, or
  `404` when there is none (not built yet, or nothing to build).
- `GET {base}/{variant path}`, where the path is `bevy/{prefix}/{name}`: the
  variant's bytes, served with `Cache-Control: immutable`.

A variant CID is a CIDv1 raw-leaf sha256 (`bafkrei…`), the scheme the catalyst
uses for content, so the Explorer verifies the bytes with its existing
content-hash check.

### Manifest schema (`v: 2`)

```json
{
  "v": 2,
  "entity": "<entity id>",
  "generator": "abgen <version>",
  "lanes": { "texture": "t1", "mesh": "m1" },
  "files": {
    "<original cid>": [
      {
        "path": "bevy/t1/<original cid>_srgb",
        "cid": "<variant cid>",
        "kind": "texture" | "model",
        "format": "ktx2-bc7" | "ktx2-bc1" | "ktx2-bc5" | "glb" | "glb-meshopt",
        "width": 1024, "height": 1024, "mips": 11,
        "size": 123456
      }
    ]
  }
}
```

- Keys are original content CIDs (the entity's `content[].hash`); the path to
  CID mapping comes from the entity.
- Several variants per original are allowed, in the generator's order of
  preference; the Explorer takes the first one it supports.
- `path` is relative to the base: `bevy`, a prefix of ASCII letters and digits
  other than `manifest`, and a name of ASCII letters, digits and `_`. A reader
  drops a variant whose path has any other shape, and one whose `cid` is not a
  raw-leaf CID.
- `lanes` records which generation each variant class was built from; it is for
  abgen, and a reader ignores it.
- `width`, `height`, `mips` and `size` are informative; a reader must not rely
  on them.
- A reader takes only `v: 2`. Version 1 named variants by CID alone under
  `{base}/bevy/contents/{cid}`; no published service serves it.

Formats:

- `ktx2-bc7`, `ktx2-bc1`, `ktx2-bc5` (`kind: texture`): KTX2 with a BC payload
  (vkFormat BC7, BC1 or BC5 UNORM) and zstd supercompression, full mip chain,
  longest side at most 1024. The Explorer applies sRGB or linear from how the
  texture is requested. BC1 is for fully opaque images, BC5 for images the
  entity's models use only as normal maps, BC7 otherwise. Images of two pixels
  or fewer a side get no variant. A creator UASTC KTX2 is transcoded block for
  block to BC7, one level, no resize or re-encode, so the variant is
  pixel-identical to the Explorer's own transcode; no variant when a level
  cannot be block-transcoded or the variant is larger than the original.
- `glb` (`kind: model`): a GLB with Draco decoded, `KHR_texture_basisu`
  resolved and embedded images replaced by BC KTX2. External image URIs stay as
  they are; they resolve to their own original CID, which has its own texture
  variant.
- `glb-meshopt` (`kind: model`): a GLB whose meshes carry
  `EXT_meshopt_compression` with `KHR_mesh_quantization`, meshes only; textures
  are left as the original has them. Both extensions are in
  `extensionsRequired`, so a reader without a meshopt decoder must not take it.

Excluded from variants: source maps, audio, video and scene code.

### Explorer rules

- Off unless a variants base URL is configured (launch option, web parameter or
  config field). Preview mode, local scenes (`b64-` ids) and scenes served from
  a loopback or file content server never ask for a manifest.
- One manifest request per scene entity per session; a `404` is remembered as
  "no manifest".
- A content file reads as a variant only when its original CID has a variant in
  a format the Explorer supports for that file type and GPU: original CID, first
  supported variant, fetch from `{base}/{path}`, verify the bytes against the
  variant CID, decode. Any failure (fetch, hash mismatch, decode)
  falls back to the original, per file; a variant that failed to decode is not
  tried again in the session and the asset is reloaded from the original.
  Repeated transport failures turn the service off for the session.
- Variant bytes are cached under the variant CID only, never under the
  original's or the path. Where a cache is keyed by url (web), bytes read from
  it are checked against the CID again. A texture variant is handed to the image
  loaders as the DDS it unwraps to, cached under a name derived from the variant
  CID.
- The supported formats are one table in the manifest client
  (`crates/ipfs/src/bevy_variants`): the schema's `kind`/`format` pair, the file
  types it may replace, what the GPU must support and what the bytes decode as.
  Today the table holds the three BC texture formats. `glb` is not taken
  because the glTF loader rejects a file that requires `KHR_texture_basisu`;
  `glb-meshopt` joins the table only behind the feature that compiles the glTF
  loader's meshopt decoder in, so a build without the decoder keeps reading the
  original GLB.

## Consequences

- A cold visit downloads BC textures that need no transcode; the stored
  deployment and its CIDs are untouched, and the texture path of a deployment is
  unchanged for anyone not configured with a base URL.
- abgen is the only producer of variants; every variant is verifiable from the
  bytes alone, so a wrong or tampered file is dropped per file without affecting
  the scene.
- The manifest carries formats the Explorer does not take yet; adding one is a
  row in the client's table and the decoder it needs, not a schema change.
- Measured savings and render-parity evidence belong in the task reports and the
  PR that enables a base URL, not in this record.
