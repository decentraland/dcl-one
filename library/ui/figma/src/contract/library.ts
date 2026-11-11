import { existsSync } from 'node:fs'
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { z } from 'zod'
import { readFdsPin } from '../sources/fds-source.js'

const packageSchema = z.object({
  name: z.string(),
  version: z.string(),
  main: z.string().optional(),
  exports: z.unknown().optional()
})

type LibraryInfo = {
  rootDir: string
  name: string
  version: string
  entryFile?: string
}

function findPackageRoot(start: string): string | undefined {
  let current = path.resolve(start)
  for (;;) {
    if (existsSync(path.join(current, 'package.json'))) return current
    const parent = path.dirname(current)
    if (parent === current) return undefined
    current = parent
  }
}

function entryOf(pkg: z.infer<typeof packageSchema>): string | undefined {
  const exported = pkg.exports
  if (typeof exported === 'string') return exported
  if (exported && typeof exported === 'object' && '.' in exported) {
    const root = (exported as Record<string, unknown>)['.']
    if (typeof root === 'string') return root
  }
  return pkg.main
}

// Provenance comes from the package that owns the mapping files, never from the consumer's own package.
// Undefined outside any package; `{ invalid }` when the owning package.json cannot be read or names no name and
// version, so the caller answers with a coded diagnostic instead of a raw error.
async function loadLibrary(
  mappingRoot: string
): Promise<LibraryInfo | { invalid: string } | undefined> {
  const rootDir = findPackageRoot(mappingRoot)
  if (!rootDir) return undefined
  const file = path.join(rootDir, 'package.json')
  let parsed: unknown
  try {
    parsed = JSON.parse(await readFile(file, 'utf8'))
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code
    return { invalid: `${file} cannot be read (${code ?? 'not JSON'})` }
  }
  const result = packageSchema.safeParse(parsed)
  if (!result.success) return { invalid: `${file} names no package name and version` }
  const pkg = result.data
  const entry = entryOf(pkg)
  return {
    rootDir,
    name: pkg.name,
    version: pkg.version,
    entryFile: entry ? path.resolve(rootDir, entry) : undefined
  }
}

async function findMappingFiles(mappingRoot: string): Promise<string[]> {
  const found: string[] = []
  const walk = async (dir: string) => {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      if (entry.name === 'node_modules') continue
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) await walk(full)
      else if (entry.name.endsWith('.figma.ts')) found.push(full)
    }
  }
  await walk(mappingRoot)
  return found.sort()
}

// The capture revision the library's tracked lock pins, read with the same pin reader fetch-source uses and
// without touching the capture itself. A library with no lock has none; a lock that exists but does not pin
// a capture is a broken checkout and throws.
async function readCaptureRevision(rootDir: string): Promise<string | undefined> {
  let text: string
  try {
    text = await readFile(path.join(rootDir, 'upstream.lock.json'), 'utf8')
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined
    throw error
  }
  return readFdsPin(JSON.parse(text)).snapshotRevision
}

export { findMappingFiles, loadLibrary, readCaptureRevision }
export type { LibraryInfo }
