// Packages the HUD hosts install for the Explorer page's core, which library/ui does not. The signer that uses them
// (explorer-core/shell/sign.ts) reaches this type check only through a type import and is never bundled here; the
// hosts check it against the packages themselves.
declare module '@noble/curves/secp256k1.js'
declare module '@noble/hashes/sha3.js'
declare module '@noble/hashes/utils.js'
