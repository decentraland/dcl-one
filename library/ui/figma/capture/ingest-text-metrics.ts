import { readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { z } from 'zod'
import { expandVariants } from '../src/capture/expand-variants.js'
import { rawComponentPageSchema, type RawNode } from '../src/ir/raw-components.js'
import { stripFileKey } from '../src/capture/strip-file-key.js'

const lineHeight = z.object({ unit: z.string(), value: z.number().optional() })
const records = z.array(z.object({
  id: z.string(),
  entries: z.array(z.object({
    path: z.array(z.number().int().nonnegative()), text: z.string(), w: z.number(), h: z.number(),
    textAlignHorizontal: z.string(), textAlignVertical: z.string(), lineHeight: lineHeight.optional(),
    paragraphSpacing: z.number(), segments: z.array(z.object({ text: z.string(), lineHeight }))
  }))
}))

// Supplement a verified capture with raw, read-only Plugin API text metrics.
const byVariant = new Map<string, z.infer<typeof records>[number]>()
for (const file of process.argv.slice(2)) {
  const response = JSON.parse(await readFile(file, 'utf8'))
  if (response.isError) throw new Error(`Figma read failed: ${file}`)
  const content = response.content.filter((entry: { type: string }) => entry.type === 'text')
  if (content.length !== 1) throw new Error(`Unexpected Figma response: ${file}`)
  for (const record of records.parse(JSON.parse(content[0].text))) {
    if (byVariant.has(record.id)) throw new Error(`Duplicate source variant: ${record.id}`)
    byVariant.set(record.id, record)
  }
}
const directory = path.resolve('figma-snapshot/components')
const config = JSON.parse(await readFile('figma.config.json', 'utf8')) as { selection: { exceptions: { pageId: string; componentId: string }[] } }
const syncState = JSON.parse(await readFile(path.resolve('figma-snapshot/sync-state.json'), 'utf8')) as { pages: { id: string; name: string; ready: boolean | null }[] }
const updates: { file: string; content: string }[] = []
let applied = 0
const matched = new Set<string>()
for (const file of await readdir(directory)) {
  if (!file.endsWith('.json')) continue
  const page = rawComponentPageSchema.parse(JSON.parse(await readFile(path.join(directory, file), 'utf8')))
  if (!syncState.pages.some(current => current.id === page.page.id && current.name === page.page.name)) continue
  let changed = false
  for (const component of page.components) {
    const marker = syncState.pages.find(current => current.id === page.page.id)!
    if (marker.ready !== true && !config.selection.exceptions.some(exception => exception.pageId === page.page.id && exception.componentId === component.id)) continue
    const variants = expandVariants(component)
    if (!variants.some(variant => byVariant.has(variant.id))) continue
    for (const variant of variants) {
      const record = byVariant.get(variant.id)
      if (!record) continue
      matched.add(record.id)
      for (const entry of record.entries) {
        let node: RawNode | undefined = variant.tree
        for (const index of entry.path) node = node?.children?.[index]
        // Exported artwork subtrees have no retained text to style.
        if (!node) continue
        if (node.type !== 'TEXT' || node.text !== entry.text || Math.abs(node.w - entry.w) > 0.02 || Math.abs(node.h - entry.h) > 0.02)
          throw new Error(`Capture drift at ${record.id} [${entry.path}]. Recapture the component.`)
        node.textAlignHorizontal = entry.textAlignHorizontal
        node.textAlignVertical = entry.textAlignVertical
        node.paragraphSpacing = entry.paragraphSpacing
        if (entry.lineHeight) node.lineHeight = entry.lineHeight
        if (node.textRuns) {
          let offset = 0
          for (const run of node.textRuns) {
            const start = offset
            offset += run.text.length
            let cursor = 0
            const overlapping = entry.segments.filter(segment => {
              const segmentStart = cursor
              cursor += segment.text.length
              return segmentStart < offset && cursor > start
            })
            if (overlapping.length && overlapping.every(segment => JSON.stringify(segment.lineHeight) === JSON.stringify(overlapping[0]!.lineHeight)))
              run.lineHeight = overlapping[0]!.lineHeight
            else throw new Error(`Line-height runs need a full recapture: ${record.id}`)
          }
        }
        applied++
      }
      changed = true
    }
    component.base.tree = variants[0]!.tree
    component.others = variants.slice(1).map(variant => ({ id: variant.id, name: variant.name, props: variant.props, patches: [{ at: [], replace: variant.tree }] }))
  }
  if (changed) updates.push({ file: path.join(directory, file), content: JSON.stringify(stripFileKey(rawComponentPageSchema.parse(page)), null, 2) + '\n' })
}
if (matched.size !== byVariant.size) throw new Error('Some source variants have no saved capture')
// Validate every input before writing any snapshot.
for (const update of updates) await writeFile(update.file, update.content)
console.log(JSON.stringify({ variants: matched.size, textNodes: applied, pages: updates.length }))
