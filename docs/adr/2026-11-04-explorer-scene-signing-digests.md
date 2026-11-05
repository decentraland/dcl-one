# Hash signed fetch bodies in the scene runtime

- Status: Accepted
- Date: 2026-11-04

Scene-runtime Rust computes SHA-256 over the snapshotted UTF-8 body before a signing
request crosses into the engine. This runs in the scene worker on web and the scene
sidecar on native. Hashing cost stays with the requesting scene. A body of any size
adds one 64-character digest to the signing message. Bodyless messages retain their
compact wire encoding.

The engine validates the digest's length and lowercase hexadecimal encoding before
using it with either the ordinary wallet or a scoped storage delegation. The engine
owns the identity, realm and delegation claims. The receiving service can verify
the body it received against the signed `hashPayload`.
