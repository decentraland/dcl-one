// Read-only supplement for an existing capture. Use exact variant IDs from review-manifest.
// Save the raw use_figma response and pass it to ingest-text-metrics.ts.
const VARIANT_IDS = []
const out = []
for (const id of VARIANT_IDS) {
  const root = await figma.getNodeByIdAsync(id)
  if (!root) throw Error('Missing source ' + id)
  const entries = []
  function walk(node, path) {
    if (node.visible === false) return
    if (node.type === 'TEXT') entries.push({
      path, text: node.characters, w: Math.round(node.width * 100) / 100, h: Math.round(node.height * 100) / 100,
      textAlignHorizontal: node.textAlignHorizontal, textAlignVertical: node.textAlignVertical,
      lineHeight: node.lineHeight === figma.mixed ? undefined : node.lineHeight,
      paragraphSpacing: node.paragraphSpacing,
      segments: node.getStyledTextSegments(['lineHeight']).map(segment => ({ text: segment.characters, lineHeight: segment.lineHeight }))
    })
    if ('children' in node) {
      let index = 0
      for (const child of node.children) {
        if (child.visible === false) continue
        walk(child, [...path, index++])
      }
    }
  }
  walk(root, [])
  out.push({ id, entries })
}
return out
