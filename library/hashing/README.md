# `@dcl/hashing`

The Decentraland content hashes, as a package in this repository ([ADR](../../docs/adr/2026-11-13-hashing-in-dcl-one.md)).

- `hashV1(content)`: the hash the content servers compute. It is the CIDv1 of the bytes as a UnixFS file with raw leaves, 262144-byte chunks and up to 174 children per node. Content of one chunk or less gives a `bafkrei…` hash; larger content gives a `bafybei…` hash.
- `hashV0(content)`: the old `Qm…` hash, the sha2-256 digest as a multihash. Use it only for entities deployed with that hash.
- `prepareADR32Data`, `calculateMultipleHashesADR32`, `calculateMultipleHashesADR32LegacyQmHash`: the [ADR-32](https://adr.decentraland.org/adr/ADR-32) hash of several files.
- `keccak256Hash(metadata, keys)`: the [ADR-62](https://adr.decentraland.org/adr/ADR-62) metadata hash.

`content` is a `Uint8Array` or an async iterable of `Uint8Array`, for example a Node read stream. A stream is hashed as it arrives, and it can use its buffer again after each part.

The functions listed above are available from the package root and produce the results of `@dcl/hashing` 1.2.0 from `decentraland/core-libs`. The package does not provide its `./node`, `./ADR32` or `./ADR62` entry points, or its `sortKeys` and `compareStrings` exports. `@noble/hashes` is the only dependency; the package encodes the CID, the dag-pb node and the UnixFS header itself.

```sh
pnpm install --frozen-lockfile --ignore-scripts
pnpm test    # runs the TypeScript sources with Node
pnpm build   # writes the CommonJS build and its types to dist/
```

The build loads `@noble/hashes`, which is ESM only, with `require`. This needs Node 22.12 or later.

## Speed

Input of 4096 bytes or more goes to the SHA-256 of the platform (WebCrypto, through `@noble/hashes/webcrypto.js`), which runs off the main thread. `hashV1` has up to 16 chunk digests in progress at the same time and adds them to the tree in chunk order. Smaller input, the tree nodes, views of shared memory and a platform without WebCrypto use the JavaScript digest of `@noble/hashes`.

A stream part that holds a whole chunk is hashed without a copy. The other bytes are copied into one chunk buffer. `hashV0` copies a stream of up to 16 MiB and hashes it in one call; it hashes a longer stream as it arrives, with the JavaScript digest.

Measured with Node 24 on 64 MB of content, `hashV1` is about 3 times faster than `@dcl/hashing` 1.1.3 on a buffer and 2 times faster on a stream of 64 KiB parts. It is more than 10 times faster than 1.2.0 on both. On files of 300 bytes it is about 7 times faster than 1.1.3. `hashV0` is slower than 1.1.3, which uses the digest of Node: about 1.5 times on a buffer and about 6 times on a stream longer than 16 MiB.

## Results and the npm packages

The results are equal to those of `@dcl/hashing` 1.1.3, 1.2.0 and 3.0.4 for a `Uint8Array`, for a stream of `Uint8Array` and for ADR-32 and ADR-62 input of the declared types. Other input has these differences:

- A stream part that is not a `Uint8Array` (a string, an array of numbers, an `ArrayBuffer`, another typed array) rejects with a `TypeError`. The npm versions give such a stream a hash, and not the same hash in each version.
- `keccak256Hash` reads only the own fields of the metadata, as 1.2.0 does. 1.1.3 and 3.0.4 also read inherited fields. The result differs only for the key `__proto__` and for metadata with a prototype of its own.
- The ADR-32 functions do not check that `file` and `hash` are strings, and they do not change the array they receive, as 1.2.0. 1.1.3 and 3.0.4 throw on a `null` field and sort the array in place.

The functions hash bytes. The same text in UTF-8 and in UTF-16, or with composed and decomposed characters, has different hashes. ADR-32 sorts by hash and then by path, by UTF-16 code unit. It does not normalize a path: case, separators and Unicode form stay as given. The metadata is written as `JSON.stringify` writes it, so `undefined` metadata is left out and `null` metadata is written.

## Tests

`test/hashing.test.ts` holds hashes that the npm packages and `ipfs-unixfs-importer` return, with the edge cases above. `test/properties.test.ts` runs seeded cases against a second implementation of the digest, the CID text and the tree, written for the tests. The cases cover stream partitions, streams that reuse their buffer, shared memory, chunk and tree boundaries, ADR-32 order, unchanged input, the order of the chunk digests and stream errors. It runs 1000 cases by default. To repeat a run or to make it longer:

```sh
HASH_FUZZ_SEED=42 HASH_FUZZ_ROUNDS=10000 pnpm test
```
