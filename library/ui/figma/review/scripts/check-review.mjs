import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { reviewSignature } from './review-signature.mjs'
import { pendingCommit } from './capture-retry.mjs'
const root = path.resolve(import.meta.dirname, '..')
if (await pendingCommit(root))
  throw new Error(
    'A capture commit was interrupted. Run npm run review:capture to complete it, then rebuild.'
  )
const report = JSON.parse(
  await readFile(path.join(root, 'public/dcl-ui-review/fidelity.json'), 'utf8')
)
if (report.signature !== (await reviewSignature(root)))
  throw new Error(
    'Visual evidence is stale. Build the review, run npm run review:capture, then rebuild.'
  )
const catalog = JSON.parse(await readFile(path.join(root, 'src/review/catalog.json'), 'utf8'))
for (const component of catalog.components)
  for (const [index, variant] of component.variants.entries()) {
    const item = report.cases[`${component.name}-${index}`]
    if (!item || item.sourceId !== variant.id)
      throw new Error('Visual evidence is incomplete: ' + component.name)
    await readFile(path.join(root, 'public', item.image))
  }
globalThis.console.log(
  'Visual evidence matches the current source. Pixel differences still require review.'
)
