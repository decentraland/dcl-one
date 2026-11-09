import { promises as fsPromises } from 'fs'
import { dirname, join, relative, resolve, sep } from 'path'
import i18next from 'i18next'
import { CliError } from './error'
import { readZip, ZipEntry } from './zip'
import { IFileSystemComponent } from '../components/fs'
import { IFetchComponent } from '../components/fetch'
import { CliComponents } from '../components'

/**
 * Check's if directory is empty
 * @param dir Directory to check for emptyness
 */
export async function isDirectoryEmpty(components: { fs: IFileSystemComponent }, dir: string): Promise<boolean> {
  const files = await components.fs.readdir(dir)
  return !files.length
}

/**
 * Download a file
 * @param url URL of the file
 * @param dest Path to where to save the file
 */
export async function download(
  components: { fs: IFileSystemComponent; fetch: IFetchComponent },
  url: string,
  dest: string
): Promise<string> {
  const response = await components.fetch.fetch(url)
  if (!response.ok) {
    // The body is never read: releasing it returns the connection instead of holding it until collection.
    await response.body?.cancel().catch(() => undefined)
    throw new CliError(
      'INIT_TEMPLATE_DOWNLOAD_FAILED',
      i18next.t('errors.init.template_download_failed', { url, status: response.status })
    )
  }
  await components.fs.writeFile(dest, Buffer.from(await response.arrayBuffer()))
  return dest
}

/**
 * Extracts a .zip file
 * @param path Path of the zip file
 * @param dest Path to where to extract the zip file
 */
export async function extract(path: string, dest: string): Promise<{ destPath: string; topLevelFolders: string[] }> {
  const zipPath = resolve(path)
  const destPath = resolve(dest)
  const invalid = (error: Error) =>
    new CliError(
      'INIT_INVALID_TEMPLATE_ZIP',
      i18next.t('errors.init.invalid_template_zip', { path: zipPath, reason: error.message })
    )
  const escapes = (entry: string) =>
    new CliError('INIT_INVALID_TEMPLATE_ZIP', i18next.t('errors.init.template_zip_escapes', { path: zipPath, entry }))
  let entries: ZipEntry[]
  try {
    entries = readZip(await fsPromises.readFile(zipPath))
  } catch (error: any) {
    throw invalid(error)
  }
  // Every entry is checked before the first write, so a bad archive leaves the destination untouched
  // and only a filesystem error can interrupt the writes: its name stays inside the destination and
  // is the only one landing there, its bytes decode and match their checksum, and nothing already in
  // the destination redirects the write (the archive cannot create a symbolic link, so a link can
  // only predate it). Entries are decoded again when written, keeping one in memory at a time.
  const targets = entries.map((entry) => {
    const target = resolve(destPath, entry.name)
    if (!target.startsWith(destPath + sep) && !(target === destPath && entry.name.endsWith('/'))) {
      throw escapes(entry.name)
    }
    return target
  })
  const seen = new Set<string>()
  for (const [i, entry] of entries.entries()) {
    if (targets[i] !== destPath && seen.has(targets[i])) throw invalid(new Error(`${entry.name} is listed twice`))
    seen.add(targets[i])
    if (!entry.name.endsWith('/')) {
      try {
        entry.read()
      } catch (error: any) {
        throw invalid(error)
      }
    }
  }
  for (const [i, entry] of entries.entries()) {
    if (targets[i] !== destPath && !(await landsInside(destPath, targets[i]))) throw escapes(entry.name)
  }
  const topLevelFolders = new Set<string>()
  for (const [i, entry] of entries.entries()) {
    const target = targets[i]
    if (target !== destPath) topLevelFolders.add(relative(destPath, target).split(sep)[0])
    if (entry.name.endsWith('/')) {
      await fsPromises.mkdir(target, { recursive: true })
      continue
    }
    await fsPromises.mkdir(dirname(target), { recursive: true })
    await fsPromises.writeFile(target, entry.read(), entry.mode ? { mode: entry.mode } : undefined)
  }
  return { destPath, topLevelFolders: Array.from(topLevelFolders) }
}

/**
 * Whether writing `target` (a path under `dest`) stays under `dest` on disk: the target is no
 * symbolic link, and its deepest ancestor that exists resolves to where it should under the
 * resolved destination. The directories missing below that ancestor are ours to create.
 */
async function landsInside(dest: string, target: string): Promise<boolean> {
  const existing = async (path: string) =>
    fsPromises.lstat(path).then(
      (stat) => stat,
      () => undefined
    )
  if ((await existing(target))?.isSymbolicLink()) return false
  let ancestor = dirname(target)
  while (!(await existing(ancestor))) {
    if (ancestor === dest) return true
    ancestor = dirname(ancestor)
  }
  const realDest = await fsPromises.realpath(dest)
  return (await fsPromises.realpath(ancestor)) === join(realDest, relative(dest, ancestor))
}

/**
 * Reads a file and parses it's JSON content
 * @param path The path to the subject json file
 */
export async function readJson<T>(components: Pick<CliComponents, 'fs'>, path: string): Promise<T> {
  const content = await components.fs.readFile(path, 'utf-8')
  return JSON.parse(content) as T
}
