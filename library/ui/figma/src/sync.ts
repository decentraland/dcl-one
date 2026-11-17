import { mkdir } from 'node:fs/promises'
import path from 'node:path'
import type { GeneratedFile } from './generators/types.js'
import { atomicWrite, listFiles, readIfExists, resolveContained, toPosix } from './guard/fs-safe.js'

// `removed` lists files that exist in a generated directory but were not regenerated. It is a report:
// nothing is deleted here. Only a retirement removes output (guard/apply-retirements.ts).
type FileDiff = { added: string[]; changed: string[]; removed: string[] }

const OWNED_DIRECTORIES = ['tokens', 'icons', 'components']

async function ownedFiles(outDir: string): Promise<string[]> {
  const found: string[] = []
  for (const directory of OWNED_DIRECTORIES)
    for (const existing of await listFiles(path.join(outDir, directory)))
      found.push(toPosix(path.relative(outDir, existing)))
  return found
}

async function diffFiles(outDir: string, files: GeneratedFile[]): Promise<FileDiff> {
  const diff: FileDiff = { added: [], changed: [], removed: [] }
  const expected = new Set(files.map((file) => file.path))

  for (const file of files) {
    const current = await readIfExists(await resolveContained(outDir, file.path))
    if (current === undefined) diff.added.push(file.path)
    else if (current !== file.content) diff.changed.push(file.path)
  }

  for (const relative of await ownedFiles(outDir))
    if (!expected.has(relative)) diff.removed.push(relative)
  return diff
}

// Adds and updates generated files. It never deletes on its own: a stale file is reported in `removed`
// and stays. A missing generation over populated output is refused, since it would look like a
// request to remove everything.
async function writeFiles(outDir: string, files: GeneratedFile[]): Promise<FileDiff> {
  const diff = await diffFiles(outDir, files)
  if (files.length === 0 && diff.removed.length > 0)
    throw new Error('Refusing to sync an empty generation over existing output')
  const toWrite = new Set([...diff.added, ...diff.changed])

  for (const file of files.filter((candidate) => toWrite.has(candidate.path))) {
    const target = await resolveContained(outDir, file.path)
    await mkdir(path.dirname(target), { recursive: true })
    await atomicWrite(target, file.content)
  }
  return diff
}

const isEmptyDiff = (diff: FileDiff) =>
  diff.added.length + diff.changed.length + diff.removed.length === 0

export { diffFiles, isEmptyDiff, ownedFiles, writeFiles }
export type { FileDiff }
