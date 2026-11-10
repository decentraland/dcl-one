import { constants } from 'node:fs'
import { randomUUID } from 'node:crypto'
import {
  access,
  lstat,
  mkdir,
  open,
  readdir,
  readFile,
  realpath,
  rename,
  rm
} from 'node:fs/promises'
import path from 'node:path'

// Only a missing path counts as "absent". Any other failure (permissions, a file where a directory
// should be, I/O errors) must stop the run: treating it as absence is how output gets deleted.
const ATOMIC_TEMP_PREFIX = '.figma-tmp-'
const isAtomicTemporary = (file: string) =>
  /^\.figma-tmp-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
    path.basename(file)
  )

const isMissing = (error: unknown): boolean => (error as NodeJS.ErrnoException)?.code === 'ENOENT'

async function readIfExists(file: string): Promise<string | undefined> {
  try {
    return await readFile(file, 'utf8')
  } catch (error) {
    if (isMissing(error)) return undefined
    throw error
  }
}

async function readBytesIfExists(file: string): Promise<Buffer | undefined> {
  try {
    return await readFile(file)
  } catch (error) {
    if (isMissing(error)) return undefined
    throw error
  }
}

async function listFiles(directory: string): Promise<string[]> {
  let entries
  try {
    entries = await readdir(directory, { withFileTypes: true })
  } catch (error) {
    if (isMissing(error)) return []
    throw error
  }
  const nested = await Promise.all(
    entries
      .filter((entry) => !isAtomicTemporary(entry.name))
      .map(async (entry) => {
        const full = path.join(directory, entry.name)
        return entry.isDirectory() ? listFiles(full) : [full]
      })
  )
  return nested.flat()
}

// A failed or interrupted write leaves the prior artifact intact. Temporary files are never inputs.
async function atomicWrite(file: string, content: string | Buffer, mode?: number): Promise<void> {
  const existing = await lstat(file).catch((error) => {
    if (isMissing(error)) return undefined
    throw error
  })
  if (existing?.isSymbolicLink()) throw new Error(`Path goes through a symlink: ${file}`)
  if (existing) await access(file, constants.W_OK)
  await mkdir(path.dirname(file), { recursive: true })
  const temporary = path.join(path.dirname(file), `${ATOMIC_TEMP_PREFIX}${randomUUID()}`)
  const handle = await open(temporary, 'wx', mode ?? (existing ? existing.mode & 0o777 : 0o644))
  let closed = false
  try {
    await handle.writeFile(content)
    await handle.close()
    closed = true
    await rename(temporary, file)
  } finally {
    try {
      if (!closed) await handle.close()
    } finally {
      await rm(temporary, { force: true })
    }
  }
}

// A generated path must stay inside its root: no absolute paths, no `..`, no symlink on the way out.
async function resolveContained(root: string, relative: string): Promise<string> {
  if (
    !relative ||
    relative.includes('\0') ||
    path.isAbsolute(relative) ||
    /^[a-zA-Z]:/.test(relative)
  )
    throw new Error(`Path is not contained: ${JSON.stringify(relative)}`)
  const segments = relative.split(/[\\/]/)
  if (segments.some((segment) => segment === '..'))
    throw new Error(`Path escapes its root: ${relative}`)

  const realRoot = await realpath(root).catch((error) => {
    if (isMissing(error)) return path.resolve(root)
    throw error
  })
  const target = path.join(root, ...segments)

  // Walk the existing part of the path and reject any symlink, whose target could leave the root.
  let current = root
  for (const segment of segments) {
    current = path.join(current, segment)
    let info
    try {
      info = await lstat(current)
    } catch (error) {
      if (isMissing(error)) break
      throw error
    }
    if (info.isSymbolicLink()) throw new Error(`Path goes through a symlink: ${relative}`)
  }
  const resolved = await realpath(path.dirname(target)).catch((error) => {
    if (isMissing(error)) return undefined
    throw error
  })
  if (resolved !== undefined && resolved !== realRoot && !resolved.startsWith(realRoot + path.sep))
    throw new Error(`Path escapes its root: ${relative}`)
  return target
}

// The real path of something that may not exist yet: resolve the nearest existing ancestor, then append the rest.
async function canonicalPath(target: string): Promise<string> {
  const missing: string[] = []
  let current = path.resolve(target)
  for (;;) {
    try {
      return path.join(await realpath(current), ...missing.reverse())
    } catch (error) {
      if (!isMissing(error)) throw error
      const parent = path.dirname(current)
      if (parent === current) return path.resolve(target)
      missing.push(path.basename(current))
      current = parent
    }
  }
}

async function validateOutputPath(
  workspace: string,
  target: string,
  managedRelative?: string
): Promise<string> {
  const root = await canonicalPath(workspace)
  const output = await canonicalPath(target)
  if (output === root || !output.startsWith(root + path.sep))
    throw new Error(`outDir must stay inside ${root} and cannot equal the workspace root`)
  if (managedRelative && output !== (await canonicalPath(path.join(root, managedRelative))))
    throw new Error('An automated sync may only write the generated folder of this repository')
  return output
}

const toPosix = (value: string): string => value.split(path.sep).join('/')

export {
  atomicWrite,
  isAtomicTemporary,
  canonicalPath,
  validateOutputPath,
  isMissing,
  listFiles,
  readBytesIfExists,
  readIfExists,
  resolveContained,
  toPosix
}
