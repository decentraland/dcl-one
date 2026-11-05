#!/usr/bin/env node
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { collectIds, getId, parseSvg, referencedIds, serialize, type SvgNode } from './svg-tree.js'
import { stripFileKey, stripFileKeyText } from './strip-file-key.js'
import { stripSvgImageMetadata } from './strip-image-metadata.js'

type IconRow = {
  path: string
  componentName: string
  nodeId: string
  x: number
  y: number
  width: number
  height: number
}

type FrameRows = { rootId: string; section: string; rows: IconRow[] }

type SplitIcon = { row: IconRow; svg: string }

type IconRecord = {
  name: string
  nodeId: string
  section: string
  category: string
  group: string
  variant: string | null
  width: number
  height: number
  file: string
}

function parseRows(text: string): FrameRows[] {
  const frames: FrameRows[] = []
  for (const line of text.split('\n')) {
    if (!line.trim()) continue
    if (line.startsWith('## ')) {
      const [rootId, section] = line.slice(3).split('|')
      frames.push({ rootId: rootId!, section: section!, rows: [] })
      continue
    }
    const [rowPath, nodeId, x, y, width, height] = line.split('|')
    const lastSegment = rowPath!.split('/').pop()!
    frames[frames.length - 1]!.rows.push({
      path: rowPath!,
      componentName: lastSegment.includes('=') ? lastSegment : rowPath!,
      nodeId: nodeId!,
      x: Number(x),
      y: Number(y),
      width: Number(width),
      height: Number(height)
    })
  }
  return frames
}

const stripDedupSuffix = (id: string) => id.replace(/_\d+$/, '')

function findComponents(root: SvgNode, rows: IconRow[]): Map<IconRow, SvgNode> {
  const found = new Map<IconRow, SvgNode>()
  let pointer = 0

  const visit = (node: SvgNode) => {
    const row = rows[pointer]
    const id = getId(node)
    if (row && id !== undefined && stripDedupSuffix(id) === row.componentName) {
      found.set(row, node)
      pointer += 1
      return
    }
    node.children.forEach(visit)
  }
  visit(root)
  return found
}

function splitFrame(svgSource: string, rows: IconRow[]): { icons: SplitIcon[]; missing: string[] } {
  const root = parseSvg(svgSource)
  const ids = collectIds(root)
  const components = findComponents(root, rows)
  const icons: SplitIcon[] = []

  for (const row of rows) {
    const node = components.get(row)
    if (!node) continue
    const body = serialize(node)
    const defs: string[] = []
    const seen = new Set<string>()
    const queue = referencedIds(body)
    while (queue.length) {
      const id = queue.pop()!
      const def = ids.get(id)
      if (seen.has(id) || !def) continue
      seen.add(id)
      const markup = serialize(def)
      defs.push(markup)
      queue.push(...referencedIds(markup))
    }
    const box = `${row.x} ${row.y} ${row.width} ${row.height}`
    const defsBlock = defs.length ? `<defs>\n${defs.join('\n')}\n</defs>\n` : ''
    icons.push({
      row,
      svg: `<svg width="${row.width}" height="${row.height}" viewBox="${box}" fill="none" xmlns="http://www.w3.org/2000/svg">\n${defsBlock}${body}\n</svg>\n`
    })
  }

  const missing = rows.filter((row) => !components.has(row)).map((row) => row.path)
  return { icons, missing }
}

function toRecord(row: IconRow, section: string): IconRecord {
  const isVariant = row.componentName !== row.path
  const segments = row.path.split('/')
  const variant = isVariant ? segments[segments.length - 1]! : null
  const name = isVariant ? segments.slice(0, -1).join('/') : row.path
  return {
    name,
    nodeId: row.nodeId,
    section,
    category: segments[0]!,
    group: segments.length > 1 ? segments.slice(0, -1).join('/') : segments[0]!,
    variant,
    width: row.width,
    height: row.height,
    file: `${row.path.replace(/\//g, '__').replace(/=/g, '-')}.svg`
  }
}

async function main(snapshotDir: string, rawDir: string): Promise<void> {
  const frames = parseRows(await readFile(path.join(snapshotDir, 'icons-rows.txt'), 'utf8'))
  const outDir = path.join(snapshotDir, 'icons')
  await rm(outDir, { recursive: true, force: true })
  await mkdir(outDir, { recursive: true })

  const records: IconRecord[] = []
  const problems: string[] = []
  const rawFiles = new Set(await readdir(rawDir))

  for (const frame of frames) {
    const rawName = `${frame.rootId.replace(':', '_')}.svg`
    if (!rawFiles.has(rawName)) {
      problems.push(`missing raw export ${rawName}`)
      continue
    }
    const { icons, missing } = splitFrame(
      await readFile(path.join(rawDir, rawName), 'utf8'),
      frame.rows
    )
    problems.push(...missing.map((name) => `not found in export: ${name}`))
    for (const icon of icons) {
      const record = toRecord(icon.row, frame.section)
      // Icons are imported rasters too when an export embeds one: only rendering chunks are kept.
      await writeFile(
        path.join(outDir, record.file),
        stripSvgImageMetadata(stripFileKeyText(icon.svg))
      )
      records.push(record)
    }
  }

  await writeFile(
    path.join(snapshotDir, 'icons.json'),
    `${JSON.stringify(stripFileKey({ icons: records }), null, 1)}\n`
  )
  process.stdout.write(`${records.length} icons written, ${problems.length} problems\n`)
  for (const problem of problems) process.stdout.write(`  ${problem}\n`)
}

const invokedDirectly = process.argv[1]?.endsWith('split-icon-frames.ts')
if (invokedDirectly) {
  const [snapshotDir = 'figma-snapshot', rawDir] = process.argv.slice(2)
  if (!rawDir) {
    process.stderr.write('Usage: split-icon-frames <snapshotDir> <rawExportsDir>\n')
    process.exit(2)
  }
  main(snapshotDir, rawDir).catch((error: unknown) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
    process.exit(1)
  })
}

export { parseRows, splitFrame, toRecord }
export type { FrameRows, IconRecord, IconRow }
