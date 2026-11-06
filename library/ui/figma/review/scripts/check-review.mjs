import path from 'node:path'
import { checkEvidence } from './check-evidence.mjs'
import { reviewSignature } from './review-signature.mjs'
const root = path.resolve(import.meta.dirname, '..')
await checkEvidence({ root, signature: await reviewSignature(root) })
globalThis.console.log(
  'Visual evidence matches the current source. Pixel differences still require review.'
)
