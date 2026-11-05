# ADR: Explorer Zstandard decoder source

- Status: Accepted
- Date: 2026-11-04

## Decision

Keep the Basis Universal transcoder's Zstandard dependency in
`rendering/bevy-explorer/source/vendor/ruzstd`, beside `vendor/basisu`.
The concrete build requirement is a pure Rust decoder with caller-controlled
output limits on both native and web Explorer builds.

The imported crate retains its license, release and revision pin, pristine
file manifest, replayable patches and shipped-file checksums. `verify.py`
checks those bytes and reverses the patches to verify the original import.
The existing Explorer ownership area covers this path.

Basis Universal depends directly on this local crate. Other dependencies'
Zstandard versions remain independently pinned. Both vendored crates are
excluded from the Explorer workspace; integration remains in its
`image_processing` crate. Updates require provenance verification and the
texture and decoder boundary tests.
