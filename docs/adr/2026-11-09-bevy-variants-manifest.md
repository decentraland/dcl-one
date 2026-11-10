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

abgen publishes, per scene, wearable and emote entity, a manifest that maps each
original content CID to the paths of its GPU-ready variants. The Explorer reads
the manifest when present and loads a variant instead
of the original; without a manifest, without a usable variant or on any failure
it loads the deployment's own file. The service can be switched off at any time
and Decentraland keeps working.

abgen names a variant by its inputs (the original's CID, how the scene uses
it, the encoder's generation), so it can find an existing variant before doing
any work and never encodes the same file twice across scenes or redeploys. A
path's bytes are fixed for the life of its generation; a new encoder
generation, or a forced rebuild, moves variants to new paths and regenerates
the manifests that named the old ones. The Explorer needs no version of its
own: it fetches what the manifest names. Because a path's bytes never change, the
manifest carries no hash for them, and the Explorer trusts a variant by its path
from the configured service, rejecting only what fails to decode.

### Routes (served by abgen)

- `GET {base}/bevy/manifest/{entityId}.json`: `200` with the manifest, or
  `404` when there is none (not built yet, or nothing to build).
- `GET {base}/{variant path}`, where the path is `bevy/{prefix}/{name}`: the
  variant's bytes, served with `Cache-Control: immutable`. A path's bytes are
  fixed for the life of its generation.

### Manifest schema (`v: 3`)

```json
{
  "v": 3,
  "lanes": { "texture": "t2", "mesh": "m1" },
  "files": {
    "<original cid>": ["bevy/t2/<original cid>_srgb", "bevy/m1/<original cid>"]
  }
}
```

- Keys are original content CIDs (the entity's `content[].hash`); the path to
  CID mapping comes from the entity. The manifest is fetched at the entity's own
  URL and names no entity.
- An original has at most one variant per class, in the generator's order of
  preference; the Explorer takes the first one it supports.
- A path is relative to the base: `bevy`, a prefix of ASCII letters and digits
  other than `manifest`, and a name of ASCII letters, digits and `_`. The
  prefix's first letter is the variant's class: `t` a BC KTX2 texture, `m` a
  `glb-meshopt` model, `g` a GLB. A reader drops a path of any other shape or
  class.
- `lanes` records which generation each variant class was built from; it is for
  abgen, and a reader only shows it.
- A reader takes only `v: 3`. Earlier versions listed a CID, format and size for
  each variant; abgen keeps those in records beside each variant, which it does
  not serve.

Classes:

- `t`, a texture: KTX2 with a BC payload, whose header's vkFormat names BC7, BC1
  or BC5 UNORM, and zstd supercompression, full mip chain,
  longest side at most 1024. The Explorer applies sRGB or linear from how the
  texture is requested. Level 0 must be whole 4×4 blocks (width and height
  multiples of 4), because the GPU refuses anything else; a reader rejects such
  a file and loads the original, so the generator pads, resizes or emits no
  variant. Lower mips may be smaller than a block. BC1 is for fully opaque images, BC5 for images the
  entity's models use only as normal maps, BC7 otherwise. Images of two pixels
  or fewer a side get no variant. A creator UASTC KTX2 is transcoded block for
  block to BC7, one level, no resize or re-encode, so the variant is
  pixel-identical to the Explorer's own transcode; no variant when a level
  cannot be block-transcoded or the variant is larger than the original.
- `g`, a GLB: a GLB with Draco decoded, `KHR_texture_basisu`
  resolved and embedded images replaced by BC KTX2. External image URIs stay as
  they are; they resolve to their own original CID, which has its own texture
  variant.
- `m`, a `glb-meshopt` model: a GLB whose meshes carry
  `EXT_meshopt_compression` with `KHR_mesh_quantization`, meshes only; textures
  are left as the original has them. Both extensions are in
  `extensionsRequired`, so a reader without a meshopt decoder must not take it.

Excluded from variants: source maps, audio, video and scene code.

### Explorer rules

- Off unless a variants base URL is configured (launch option, web parameter or
  config field). Preview mode, local scenes (`b64-` ids) and entities served
  from a loopback or file content server never ask for a manifest; in preview
  mode that includes wearables and emotes, so a preview shows deployed files.
- One manifest request per entity per session: each scene when it loads, and
  each wearable and emote when its entity is fetched for an avatar. A `404` is
  remembered as "no manifest". A wearable or emote takes its content server from
  the catalyst it was fetched from, so it is eligible on any realm.
- An entity's files wait for its manifest for at most two seconds from the
  request. A file read after that loads its original at once, so a slow service
  delays a load by that much at worst; a manifest that arrives later still serves
  the files read after it.
- A content file reads as a variant only when its original CID has a variant of
  a class the Explorer supports for that file type and GPU: original CID, first
  supported variant, fetch from `{base}/{path}`, decode. Any failure (fetch, a
  KTX2 that does not unwrap, a model that does not load) falls back to the
  original, per file; a variant that failed is not tried again in the session and
  the asset is reloaded from the original. An avatar waits for that reload
  before it gives up on a body or wearable. Repeated transport failures turn the
  service off for the session.
- Variant bytes are cached under a digest of the URL they came from, never under
  the original's content hash, so a variant can never stand in for an original
  in the content cache. A texture variant is handed to the image loaders as the
  DDS it unwraps to, cached beside it.
- The supported classes are one table in the manifest client
  (`crates/ipfs/src/bevy_variants`): the class, the file types it may replace,
  what the GPU must support and what the bytes decode as. The table holds the
  BC texture class (BC1, BC5 or BC7, read from the KTX2 header) and, behind the
  `ipfs` crate's `meshopt-variants` feature, `glb-meshopt`: its bytes are handed
  as they are to the loader of the original `.glb`/`.gltf` path (the meshopt
  pre-pass glTF loader). The `decentra-bevy` crate, which
  compiles that loader's decoder in, enables the feature; a build without it
  keeps reading the original GLB. `glb` is not taken because the glTF loader
  rejects a file that requires `KHR_texture_basisu`.

## Consequences

- A cold visit downloads BC textures that need no transcode; the stored
  deployment and its CIDs are untouched, and the texture path of a deployment is
  unchanged for anyone not configured with a base URL.
- abgen is the only producer of variants, and the Explorer trusts the configured
  variants service for their content: with no hash in the manifest, a file that
  decodes is shown as served. A malformed file is dropped per file without
  affecting the scene. Only a deployment's own service may be configured on web
  (the launch gate refuses other hosts); a native launch option is the
  operator's choice.
- The manifest names classes the Explorer does not take yet; adding one is a
  row in the client's table and the decoder it needs, not a schema change.
- Measured savings and render-parity evidence belong in the task reports and the
  PR that enables a base URL, not in this record.
