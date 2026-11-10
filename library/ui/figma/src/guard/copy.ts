import type { Stats } from 'node:fs'
import { cp, lstat, readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import { atomicWrite, isAtomicTemporary, isMissing, readBytesIfExists } from './fs-safe.js'

const excluded = new Set(['node_modules', 'dist', 'review-dist', '.git', '.figma-cache'])
type Entry = Pick<Stats, 'isSymbolicLink' | 'isDirectory'>

async function validateTree(file: string, known?: Entry): Promise<void> {
  const info =
    known ??
    (await lstat(file).catch((error) => {
      if (isMissing(error)) return undefined
      throw error
    }))
  if (!info) return
  if (info.isSymbolicLink()) throw new Error(`Managed artifacts may not be symlinks: ${file}`)
  if (!info.isDirectory()) return
  for (const entry of await readdir(file, { withFileTypes: true }))
    if (!excluded.has(entry.name) && !isAtomicTemporary(entry.name))
      await validateTree(path.join(file, entry.name), entry)
}

// Staging must not retain links that a later write could follow back into the published tree.
async function copyManagedTree(from: string, to: string): Promise<void> {
  await Promise.all([validateTree(from), validateTree(to)])
  await cp(from, to, {
    recursive: true,
    filter: async (source, destination) => {
      if (excluded.has(path.basename(source)) || isAtomicTemporary(source)) return false
      const info = await lstat(source)
      if (info.isDirectory()) return true
      const content = await readFile(source)
      const previous = await readBytesIfExists(destination)
      if (previous === undefined || !previous.equals(content))
        await atomicWrite(destination, content, info.mode & 0o777)
      return false
    }
  })
}
export { copyManagedTree }
