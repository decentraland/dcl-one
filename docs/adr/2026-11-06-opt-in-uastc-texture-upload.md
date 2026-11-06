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

`dcl-scene-cli` converts the images that GLB and glTF models sample. `start` converts them in the
background while the setting is on, the way a game engine imports assets. Whether the preview serves the converted
files and `deploy` uploads them is a per-project setting, off by default. With it off, every payload
is byte-for-byte what it was.

- **Setting.** `.dcl-one/settings.json` holds `optimizeTextures` and `keepSourceTextures`, both off
  when missing. `--optimize-textures` / `--no-optimize-textures` and `--keep-source-textures` /
  `--no-keep-source-textures` on `start` and `deploy` set and store them. Later runs and a deploy
  started from the preview page read them back.
- **Background import.** `start` runs one import thread per project, at startup and after every
  watch batch. A request during a run queues exactly one more run. The encoder uses all cores but
  one, and the preview never waits for it. A run whose inputs (the publishable files' sizes and
  modification times, and the setting) are unchanged does nothing. With the setting off a run
  encodes nothing and drops the overlay, so a project that never opted in spends no CPU on it. A
  deploy that turns the setting on queues the next run when it ends.
- **Swap.** A run builds a complete overlay and replaces the previous one in a single step. Until
  then the preview serves the previous overlay. Rewritten models are staged under the sha256 of
  their bytes, so a run never rewrites a file the overlay in use names, and staged models the new
  overlay no longer names are deleted after the swap. `start` sends no reload when a run ends: the
  next reload, from a save or from the client, serves the new files.
- **Status.** Each run reports on the live-reload socket with
  `decentraland.sdk.development.TextureOptimization`, case `texture_optimization = 3` of
  `WsSceneMessage`. The message carries the scene id, the state (`TOS_RUNNING`, `TOS_READY`,
  `TOS_FAILED`), the textures done, total and failed, the source and KTX2 bytes of the converted
  images, and whether the setting is on. A JSON text frame `TEXTURE_OPTIMIZATION` carries the same
  fields in camelCase, as `SCENE_UPDATE` does for `UpdateScene`. `RUNNING` goes out while textures
  are encoded, at most every 500 ms. `READY` or `FAILED` goes out once per run, and `READY` only
  when the overlay or the setting changed or `RUNNING` went out. A run that changes nothing sends
  nothing. A client that connects later receives the last status first. The client decides when to
  reload. The Bevy Explorer routes the message from its preview socket to the HUD (the
  `getTextureOptimizationStream` system API), marking the first status per scene within two
  seconds of the socket opening as the replay; the HUD shows one toast and offers **Reload**, which
  drops the live scene as a save's `UpdateScene` does, except for a replayed `READY`, which the
  freshly loaded scene already reflects.
- **Deploy.** With the setting on, `deploy` runs over the release payload after the build, encoding
  in the foreground, with a progress line, whatever the cache lacks. A warm cache costs nothing. It
  holds the process's import lock until the payload is read, so no background swap lands in
  between, then puts the preview's overlay back.

- **Format.** UASTC LDR 4x4 in a Zstandard-supercompressed KTX2, one level with no mipmaps (the
  Bevy Explorer keeps a single level for every texture, its own PNG path included, so mipmaps
  would add about a third to each file for nothing it reads; a client that samples KTX2 mipmaps
  revisits this), encoded with
  `-uastc -ktx2 -uastc_level 1 -uastc_rdo_l 1.0`. Base colour and emissive slots (and the colour
  slots of the material extensions) are sRGB. Normal slots add `-normal_map -linear`. Every other
  slot adds `-linear`. An image used as colour and as data stays as it was: one KTX2 serves every
  slot sampling it, and either transfer function would skew the other slots. An image used only in
  data slots is linear.
- **Preparation.** The source is decoded by content, not by name. Like the Explorer's own texture
  path, it is resized so the longer side is at most 1024 px and both sides are multiples of 4
  (Catmull-Rom). It is then written as a lossless PNG for the encoder. Images of at most
  1024 bytes of RGBA, or with a side of 2 px or less, are kept.
- **Encoder.** Basis Universal's official `basisu_st.wasm` at tag `v2_50`, pinned by sha256 in
  `apps/dcl-scene-cli/basisu-release.lock` beside its Apache-2.0 license and notice. It is embedded
  in the binary and run in-process by wasmtime over WASI preview 1, one instance per texture in its
  own scratch directory, on as many threads as cores (one fewer in the background). The module is compiled once per process. NaN
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
  chunk by offset, or its first buffer names a file (so the chunk is not buffer 0), the chunk is
  kept as it was. All other fields and extensions are kept unchanged.
  An image stays as it was when its KTX2 is not smaller than the source, it cannot be decoded or
  encoded, or a texture samples it through another extension.
- **Originals.** Creator files are never written to or deleted. An original image file is left out
  of the published list only when every model that names it was rewritten without it, and no
  published `.js`, `.json`, `.crdt` or `.composite` file names it. That covers the bundle and its
  chunks, `main.crdt`, composites and `scene.json`. A file is named by its file name anywhere, or by
  its bare stem closed by a single, double or back quote and opened by one or by a `/`, the shape
  of code that appends the extension itself. Image URIs a model gives that resolve to no scene file
  (a Windows path) are searched as well. Every file is read where the payload reads it: a deploy
  searches the files it uploads, release-only build output included, and the preview the files it
  serves. `deploy` warns about the originals it leaves out, since a texture path built at runtime
  would 404. It also lists the originals it treats differently from the preview's last import. `--keep-source-textures` keeps
  every original.
- **Staging.** Rewritten models, KTX2 files, the background import's `manifest.json`, the last
  deploy's `deploy-manifest.json` and a cache live in `.dcl-one/optimized-textures/`. The cache is keyed by the source's sha256, the encoder identity
  (module sha256 and arguments), the colour mode and the size cap, so an unchanged texture is never
  encoded again.
- **Preview equals upload.** The overlay (staged sources, added KTX2 files, left-out originals) is
  applied where the payload is decided: `deploy`'s file list and `payload_path`, and the preview's
  scene entity and `/content/contents`. Once an import finishes, the preview serves the bytes a
  deploy uploads. A texture that fails to encode keeps its original. A failed run stops a deploy
  rather than publishing a half-optimized scene.
- **Scope.** Only textures that GLB and glTF models sample. Textures that scene code loads directly
  (SDK `Material` and UI textures) are not converted.

## Consequences

Scene payloads shrink, and the Bevy Explorer skips its decode and BC7 compression for these
textures. Content hashes are the same whichever machine deploys. The binary embeds the 8.4 MB
module (deflated) and links wasmtime with Cranelift. The first import of a large scene takes
minutes of background work on all cores but one. Later runs re-encode only changed textures, and a
deploy after a preview finds them cached. Until a run finishes, a changed model or texture is
served as the previous import left it. The status message adds a case to `WsSceneMessage`; clients
that ignore unknown oneof cases are unaffected.

Clients without KTX2 support show converted models untextured, because no fallback is published.
These include Unity's local raw-glTF preview and Bevy Explorer builds older than its
`KHR_texture_basisu` support. The flag stays opt-in until both gates pass:

- the production asset-bundle converter (abgen) ingests KTX2 for Unity clients;
- the shipped Bevy Explorer loads `KHR_texture_basisu` natively.

These gates apply to the setting's default only.

Updating the encoder means pinning a new module in `basisu-release.lock`. That changes the encoder
identity, so every texture is encoded again and gets a new content hash.
