# ADR: Vendor the Basis Universal transcoder inside the Explorer

- Status: Accepted
- Date: 2026-11-04

## Context

The Bevy Explorer transcodes UASTC KTX2 textures (`KHR_texture_basisu`, or plain `.ktx2` image
sources) to BC7 on load, on native and on the web. The transcoder is `basisu` 0.1.0, a pure-Rust
port of the Basis Universal transcoder: one implementation for both targets, with no C++ or
Emscripten build. The official C++ transcoder does not build for `wasm32-unknown-unknown`; using
it would mean native bindings plus a separate Emscripten module and a JavaScript bridge on the
web.

`basisu` 0.1.0 is its maintainer's first release, and it reads scene content, which is
untrusted. Review found that it sizes buffers from KTX2 header fields without checking them
against the data, and adds file offsets and lengths without overflow checks. A file of a few
hundred bytes could make the client attempt a multi-gigabyte allocation, and on wasm32, where
`usize` is 32 bits, wrapped sums pass bounds checks and the following slice panics. Either aborts
the engine. The upstream repository has no private vulnerability reporting.

## Decision

Vendor the published crate at `rendering/bevy-explorer/source/vendor/basisu/`. The Explorer is
its only consumer, so it lives in the Explorer's tree rather than `library/`. It keeps its
package identity and is listed in the workspace's `exclude`, so the Explorer's Clippy and Rustfmt
gates don't apply to third-party code. `[workspace.dependencies]` points at the path, and
`Cargo.lock` records it as a path package.

Import the exact crates.io package `basisu` 0.1.0, whose `.crate` SHA256 is
`36056503b9996959defdfcf058811637fdf68a2e35a48ee8e39c89a787fe9cdc` (the checksum `Cargo.lock`
recorded), built from `github.com/marcogomez/basisu` revision
`cca5a1c8f90147a5f2b74bcabbc6329d30c9d6f2`. Preserve the Apache-2.0 license and the package's
metadata. Beside the source, as for `library/vendor/rusty_enet`, keep `upstream.lock.json` (pin,
license digest, manifest and patch hashes, update instructions), the pristine and shipped SHA256
manifests, the patches and `verify.py`. `.gitattributes` disables newline conversion for the
subtree.

Fix the crate locally in one reviewed patch, `patches/0001-bound-untrusted-sizes.patch`, instead
of depending on an upstream release:

- reject KTX2 width or height above 16384, and more levels than the base level's mip chain;
- add every offset, length and image index from the header and supercompression global data in
  checked or u64 arithmetic;
- bound a Zstandard level's declared uncompressed length by its block data;
- compute output sizes with checked arithmetic, and allocate output and inflate buffers
  fallibly, so an oversized file is an error rather than a process abort;
- keep the key/value scan within its range.

A second patch, `patches/0002-bound-zstd-decoding.patch`, moves the crate's Zstandard decoder
from `ruzstd` 0.7 to the crates.io release 0.9.1, which checks a frame's declared window when the
decoder is created and stops a block's sequences at the block maximum. A level's frame may declare
a window no larger than the greater of its uncompressed length and 8 MiB, so a level's decode is
bounded by that window plus its length plus one block, and any output past its length fails it. `ruzstd` stays a registry dependency;
`cargo`'s `[patch]` can't change the vendored crate's version requirement, which is why the bump
is part of the patch.

A third patch, `patches/0003-remove-etc1s-transcoders.patch`, removes ETC1S (BasisLZ)
transcoding, which the Explorer never reaches: `image_processing` takes UASTC only. The per-format
ETC1S transcoders and the precomputed tables only they use, about 640,000 of the crate's 680,000
lines, are deleted, and `transcode_etc1s` returns `None` for every target, as it does for an
unsupported one. KTX2 and `.basis` containers holding ETC1S are still parsed. The deleted files are
listed under `omitted` in `upstream.lock.json` and keep their published digests in
`upstream-files.sha256`.

Each modified file carries a notice of modification, as Apache-2.0 section 4(b) requires.

`image_processing` keeps its own policy check in front of the crate: a single 2D UASTC image
(vkFormat undefined, stored plain or zstd-supercompressed) within 8192, the size whose RGBA
fallback stays within what the PNG path accepts.

## Verification

`python3 rendering/bevy-explorer/source/vendor/basisu/verify.py` checks every shipped byte against
the manifests, the license digest and the patch hashes, and replays the patches in reverse to the
pristine package bytes, offline. Omitted files must be absent; they are checked only against the
published package, not in the offline replay. The Explorer CI's Integration guards job runs it on every
Explorer change. `image_processing`'s tests cover oversized and inconsistent headers (dimensions,
level ranges past the file, an oversized uncompressed length, too many levels, truncation,
BasisLZ and BC7) and the glTF fallback when a KTX2 can't be transcoded.

Not verified: the published package against the upstream Git revision (the manifest anchors the
crates.io bytes only), and the block decoders beyond these tests. The UASTC path the Explorer
reaches has not been fuzzed or audited for panics in its block data.

## Consequences

The Explorer no longer depends on the upstream maintainer for fixes to the transcoder it ships;
updating the pin means re-importing a release, reviewing the patch against it and regenerating
the provenance. Reports of these issues upstream are optional. The patch changes no public API
except that output sizes that overflow `usize` and failed allocations now return
`Error::InvalidData`.
