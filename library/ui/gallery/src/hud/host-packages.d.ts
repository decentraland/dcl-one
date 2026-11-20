// Packages the HUD hosts install for the Explorer page's core, which library/ui does not. The signer that uses them
// (explorer-core/shell/sign.ts) reaches the gallery only on demand, in code it never runs: the build leaves them out
// and fails if a page would load one (HOST_PACKAGES in vite.config.ts). The hosts check the signer against the
// packages themselves.
declare module '@noble/curves/secp256k1.js'
declare module '@noble/hashes/sha3.js'
declare module '@noble/hashes/utils.js'
