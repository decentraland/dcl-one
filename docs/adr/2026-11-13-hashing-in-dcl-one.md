# ADR: The hashing package lives in `library/hashing`

- Status: Accepted
- Date: 2026-11-13

## Decision

`library/hashing` is the `@dcl/hashing` package: `hashV0`, `hashV1` and the ADR-32 and ADR-62 helpers listed in its README. These functions are available from the package root and produce the results of `@dcl/hashing` 1.2.0 from `decentraland/core-libs`. The package keeps the Apache-2.0 `LICENSE`.

`@noble/hashes` is the only dependency. The package encodes the CID, the dag-pb node and the UnixFS file header itself, so it needs no IPFS library. Its tests hold hashes that the npm packages and `ipfs-unixfs-importer` return, including trees of two and more levels, and seeded property tests compare it with a second implementation written for the tests. `structure.yml` runs them.

Input of 4096 bytes or more uses the SHA-256 of the platform through `@noble/hashes/webcrypto.js`, and `hashV1` hashes up to 16 chunks at the same time. Smaller input and a platform without WebCrypto use the JavaScript digest. `hashV0` on a stream longer than 16 MiB also uses the JavaScript digest, because WebCrypto has no incremental digest; this is the one case slower than the npm versions that use the digest of Node.

For input of the declared types the results are equal to `@dcl/hashing` 1.1.3, 1.2.0 and 3.0.4. Where those versions differ on other input, the package follows 1.2.0. One case is stricter than all of them: a stream part that is not a `Uint8Array` rejects. The package README lists the differences.

The package is managed with pnpm and keeps its `pnpm-lock.yaml` beside it. The build is CommonJS and loads `@noble/hashes`, which is ESM only, with `require`. This needs Node 22.12 or later, so `@dcl/sdk-commands` requires that version too.

`@dcl/sdk-commands` uses the package through `file:../../../../hashing`, the same link it has to `library/protocol`. `make install` in `library/js-sdk-toolchain` installs and builds the package first. `make prepare` pins `@dcl/hashing` in the packed manifests to `HASHING_VERSION`, with the rule that `PROTOCOL_VERSION` follows: the caller sets it, and `sdk.yml` packs with a `0.0.0-unpublished.<sha>` value. The package is `private` until it is published from this repository.

## Limits

The package does not provide the `./node`, `./ADR32` or `./ADR62` entry points, or the `sortKeys` and `compareStrings` exports of the published 1.2.0 package. Its compatibility scope is the root functions listed in the README.

A package installed from npm that depends on `@dcl/hashing` keeps the npm version. npm resolves a `file:` override relative to each dependent, so one override cannot point nested packages at this folder. This applies to `dcl-catalyst-client` and to the published `@dcl/sdk-commands` versions in the SDK lockfiles, in the Explorer bridge scene and in the UI editor scene.
