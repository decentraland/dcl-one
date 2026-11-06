import { buildOwnership, ownerOf } from '../guard/ownership.js'

// The approved input hashes identify the pages whose reviewed artifacts changed since promotion.
async function promotionScope(
  root: string,
  previous: Record<string, string> | undefined,
  current: Record<string, string>
) {
  const ownership = await buildOwnership(root)
  const pages = new Set<string>()
  let foundationsCaptured = false
  for (const [file, hash] of Object.entries(current)) {
    if (previous?.[file] === hash) continue
    const owner = ownerOf(ownership, file)
    if (owner && 'pageId' in owner) pages.add(owner.pageId)
    if (owner && 'aggregate' in owner && owner.aggregate === 'foundations')
      foundationsCaptured = true
  }
  return { capturedPageIds: [...pages], foundationsCaptured }
}
export { promotionScope }
