import { promises as fsPromises } from 'fs'
import { dirname, join, resolve, sep } from 'path'
import { readZip } from './zip'
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
  // we should remove this package and use the native "fetch" when Node
  // releases it as stable: https://nodejs.org/docs/latest-v18.x/api/globals.html#fetch
  const data = await (await components.fetch.fetch(url)).arrayBuffer()
  await components.fs.writeFile(dest, Buffer.from(data))
  return dest
}

/**
 * Extracts a .zip file
 * @param url Path of the zip file
 * @param dest Path to where to extract the zip file
 */
export async function extract(path: string, dest: string): Promise<{ destPath: string; topLevelFolders: string[] }> {
  const destPath = resolve(dest)
  // All names are checked before the first write, so a bad entry leaves no file behind. Links are skipped.
  const entries = readZip(await fsPromises.readFile(resolve(path)))
    .map((entry) => ({ ...entry, parts: entryParts(entry.name) }))
    .filter((entry) => entry.parts.length > 0 && (entry.mode & TYPE_MASK) !== TYPE_LINK)
  await fsPromises.mkdir(destPath, { recursive: true })
  const root = await fsPromises.realpath(destPath)
  for (const entry of entries) {
    const target = join(root, ...entry.parts)
    const folder = entry.name.endsWith('/') || (entry.mode & TYPE_MASK) === TYPE_FOLDER
    await makeFolder(root, folder ? target : dirname(target), entry.name)
    if (folder) continue
    if ((await fsPromises.lstat(target).catch(() => undefined))?.isFile() === false) {
      throw new Error(`zip entry would replace something that is not a file: ${entry.name}`)
    }
    await fsPromises.writeFile(target, entry.data, { mode: entry.mode & 0o777 || 0o666 })
  }
  return { destPath, topLevelFolders: Array.from(new Set(entries.map((entry) => entry.parts[0]))) }
}

const TYPE_MASK = 0o170000
const TYPE_LINK = 0o120000
const TYPE_FOLDER = 0o040000

/**
 * The path parts of a zip entry name, which must be relative and `/`-separated. Windows drops the
 * dots and spaces at the end of a name, so a part of only dots and spaces is `.` or `..` there.
 */
function entryParts(name: string): string[] {
  if (name === '' || /[\\\0]/.test(name) || name.startsWith('/') || /^[a-zA-Z]:/.test(name)) {
    throw new Error(`invalid zip entry name: ${name}`)
  }
  const parts = name.split('/').filter((part) => part !== '' && part !== '.')
  if (parts.some((part) => /^[. ]+$/.test(part))) throw new Error(`zip entry escapes the destination: ${name}`)
  return parts
}

/** Only the nearest folder that exists can be a link: extract makes the folders below it, and no link. */
async function makeFolder(root: string, folder: string, name: string) {
  let existing = folder
  while (!(await fsPromises.lstat(existing).catch(() => undefined))) existing = dirname(existing)
  const real = await fsPromises.realpath(existing)
  if (real !== root && !real.startsWith(root + sep)) {
    throw new Error(`zip entry goes through a link out of the destination: ${name}`)
  }
  await fsPromises.mkdir(folder, { recursive: true })
}

/**
 * Reads a file and parses it's JSON content
 * @param path The path to the subject json file
 */
export async function readJson<T>(components: Pick<CliComponents, 'fs'>, path: string): Promise<T> {
  const content = await components.fs.readFile(path, 'utf-8')
  return JSON.parse(content) as T
}
