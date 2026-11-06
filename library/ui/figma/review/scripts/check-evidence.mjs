import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { pendingCommit } from './capture-retry.mjs'
import { staleCases } from './capture-scope.mjs'

// Fails unless the committed visual evidence was captured from the current source: no interrupted capture, the
// report signed with the current signature, no case kept from an earlier capture, and every catalogued variant
// recorded with its image. The signature is passed in because computing it reads the whole Review source.
async function checkEvidence({ root, signature }) {
  if (await pendingCommit(root))
    throw new Error(
      'A capture commit was interrupted. Run npm run review:capture to complete it, then rebuild.'
    )
  const report = JSON.parse(
    await readFile(path.join(root, 'public/dcl-ui-review/fidelity.json'), 'utf8')
  )
  if (report.signature !== signature)
    throw new Error(
      'Visual evidence is stale. Build the review, run npm run review:capture, then rebuild.'
    )
  // A partial capture (REVIEW_CAPTURE_PAGE_IDS) keeps the evidence of the pages it left out, signed by the
  // capture it came from.
  const stale = staleCases(report, signature)
  if (stale.length)
    throw new Error(
      `Visual evidence is stale for ${stale.length} case(s) kept from an earlier capture (${stale.slice(0, 5).join(', ')}${stale.length > 5 ? ', ...' : ''}). Run npm run review:capture for every page, then rebuild.`
    )
  const catalog = JSON.parse(await readFile(path.join(root, 'src/review/catalog.json'), 'utf8'))
  for (const component of catalog.components)
    for (const [index, variant] of component.variants.entries()) {
      const item = report.cases[`${component.name}-${index}`]
      if (!item || item.sourceId !== variant.id)
        throw new Error('Visual evidence is incomplete: ' + component.name)
      await readFile(path.join(root, 'public', item.image))
    }
}
export { checkEvidence }
