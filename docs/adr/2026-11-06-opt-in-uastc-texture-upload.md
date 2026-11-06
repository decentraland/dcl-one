# ADR: Opt-in UASTC KTX2 textures in scene uploads

- Status: Proposed
- Date: 2026-11-06

## Context

Most of a typical scene's payload is the PNG and JPEG textures inside its GLB and glTF models. A
client decodes each one and, in the Bevy Explorer, compresses it to BC7 on load. Basis Universal's
UASTC format ships GPU-ready blocks in a KTX2 container: the Bevy Explorer transcodes it to BC7
without a decode and block search ([vendored transcoder](2026-11-04-vendored-basisu.md)), and with
Zstandard supercompression and RDO the files are smaller than the sources.

Measured on Genesis Plaza (909 images, each resized to at most 1024 px, 10 cores), from 164 MB of
source images, with a native basisu encoder:

| Setting           | Time  | Output |
| ----------------- | ----- | ------ |
| UASTC L2 + RDO    | 308 s | 111 MB |
| UASTC L1 + RDO    | 173 s | 102 MB |
| UASTC L0 + RDO    |  90 s |  86 MB |

Level 1 stays within 0.5 dB PSNR of level 2 after transcoding to BC7, including the 5th percentile
and the minimum. Level 0 brings normal maps down to a 5th-percentile PSNR of 31.2 dB.

Native basisu builds produce different bytes from the WebAssembly build on about half of the
images, through floating-point code generation, and so would differ between deployers' machines.
The single-threaded WASI build gives byte-identical output run to run, at 1.19 times the native
encode time. basisu's own multithreading is slower than one encoder per texture across cores and
is not deterministic.

## Decision

`dcl-scene-cli start --optimize-textures` and `dcl-scene-cli deploy --optimize-textures` convert the
images that GLB and glTF models sample. The flag is off by default; without it, every payload is
byte-for-byte what it was.

- **Format.** UASTC LDR 4x4 in a Zstandard-supercompressed KTX2, encoded with
  `-uastc -ktx2 -uastc_level 1 -uastc_rdo_l 1.0`. Base colour and emissive slots (and the colour
  slots of the material extensions) are sRGB. Normal slots add `-normal_map -linear`. Every other
  slot adds `-linear`. An image used as colour and as data is encoded as sRGB and reported. An image
  used only in data slots is linear.
- **Preparation.** The source is decoded by content, not by name. Like the Explorer's own texture
  path, it is resized so the longer side is at most 1024 px and both sides are multiples of 4
  (Catmull-Rom). It is then written as a lossless PNG for the encoder. Images of at most
  1024 bytes of RGBA, or with a side of 2 px or less, are kept.
- **Encoder.** Basis Universal's official `basisu_st.wasm` at tag `v2_50`, pinned by sha256 in
  `apps/dcl-scene-cli/basisu-release.lock` beside its Apache-2.0 license and notice. It is embedded
  in the binary and run in-process by wasmtime over WASI preview 1, one instance per texture in its
  own scratch directory, on as many threads as cores. The module is compiled once per process. NaN
  canonicalization and deterministic relaxed SIMD are enabled, so the output does not depend on
  the CPU.
- **Models.** Each converted image becomes an external file beside the model,
  `<sha256 of the KTX2>.ktx2`, which the model names with no path segment and `mimeType`
  `image/ktx2`, so no client has to resolve `..`. Models in one folder share the file; the same
  texture in several folders is published at several paths and, being content-addressed, uploaded
  and stored once. Textures sampling it carry
  `KHR_texture_basisu` with no fallback `source`, and the extension is added to `extensionsUsed` and
  `extensionsRequired`. A GLB's binary chunk drops the image bytes and is repacked, with buffer-view
  references renumbered, Draco's included. If the GLB uses an extension that could address the
  chunk by offset, the chunk is kept as it was. All other fields and extensions are kept unchanged.
  An image stays as it was when its KTX2 is not smaller than the source, it cannot be decoded or
  encoded, or a texture samples it through another extension.
- **Originals.** Creator files are never written to or deleted. An original image file is left out
  of the published list only when every model that names it was rewritten without it, and its file
  name appears in no published `.js`, `.json`, `.crdt` or `.composite` file. That covers the bundle
  and its chunks, `main.crdt`, composites and `scene.json`. `--keep-source-textures` keeps every
  original.
- **Staging.** Rewritten models, KTX2 files, a `manifest.json` and a cache live in
  `.dcl-one/optimized-textures/`. The cache is keyed by the source's sha256, the encoder identity
  (module sha256 and arguments), the colour mode and the size cap, so an unchanged texture is never
  encoded again.
- **Preview equals upload.** The overlay (staged sources, added KTX2 files, left-out originals) is
  applied where the payload is decided: `deploy`'s file list and `payload_path`, and the preview's
  scene entity and `/content/contents`. The preview serves the bytes a deploy uploads, and a deploy
  started from the preview page optimizes with the preview's settings. A failed optimization stops
  a deploy rather than publishing the originals.
- **Scope.** Only textures that GLB and glTF models sample. Textures that scene code loads directly
  (SDK `Material` and UI textures) are not converted.

## Consequences

Scene payloads shrink, and the Bevy Explorer skips its decode and BC7 compression for these
textures. Content hashes are the same whichever machine deploys. The binary embeds the 8.4 MB
module (deflated) and links wasmtime with Cranelift. The first optimization of a large scene takes
minutes; later runs re-encode only changed textures.

Clients without KTX2 support show converted models untextured, because no fallback is published.
These include Unity's local raw-glTF preview and Bevy Explorer builds older than its
`KHR_texture_basisu` support. The flag stays opt-in until both gates pass:

- the production asset-bundle converter (abgen) ingests KTX2 for Unity clients;
- the shipped Bevy Explorer loads `KHR_texture_basisu` natively.

Updating the encoder means pinning a new module in `basisu-release.lock`. That changes the encoder
identity, so every texture is encoded again and gets a new content hash.
