import { existsSync } from 'node:fs'
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { z } from 'zod'

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
async function loadLibrary(mappingRoot: string): Promise<LibraryInfo | undefined> {
  const rootDir = findPackageRoot(mappingRoot)
  if (!rootDir) return undefined
  const pkg = packageSchema.parse(
    JSON.parse(await readFile(path.join(rootDir, 'package.json'), 'utf8'))
  )
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

export { findMappingFiles, loadLibrary }
export type { LibraryInfo }
