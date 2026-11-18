// "Download item": every stored file of an item, zipped the way the importer accepts it back.
import JSZip from 'jszip'
import { fetchItemContents } from '~/lib/builder'
import { THUMBNAIL_PATH, getModelFileNameFromSubfolder, isImageFile, isModelFile } from './itemFiles'
import { BodyShapeType, IMAGE_PATH, VIDEO_PATH, type Item } from './items'
import { downloadBlob } from './navigation'

const MALE_FOLDER = `${BodyShapeType.MALE}/`
const FEMALE_FOLDER = `${BodyShapeType.FEMALE}/`
const ITEM_FILES = [THUMBNAIL_PATH, IMAGE_PATH, VIDEO_PATH]

/** The name the importer gives a root-level file: it flattens the models and images of a zip without body shape folders. */
function getImportedName(path: string): string {
  return (isModelFile(path) || isImageFile(path) ? getModelFileNameFromSubfolder(path) : path).toLowerCase()
}

const stripFolder = (path: string): string | null => {
  const folder = [MALE_FOLDER, FEMALE_FOLDER].find(prefix => path.startsWith(prefix))
  return folder ? path.slice(folder.length) : null
}

/** Folds `male/` and `female/` into their parent when both hold the same files, or returns null when they don't. */
function foldBodyShapes(files: Map<string, string>): Map<string, string> | null {
  const shapes = [MALE_FOLDER, FEMALE_FOLDER].map(
    folder =>
      new Map([...files].filter(([path]) => path.startsWith(folder)).map(([path, hash]) => [stripFolder(path)!, hash]))
  )
  const [male, female] = shapes
  if (male.size === 0 || male.size !== female.size) return null
  for (const [path, hash] of male) if (female.get(path) !== hash || files.has(path)) return null
  return new Map([...[...files].filter(([path]) => stripFolder(path) === null), ...male])
}

/**
 * Maps each stored path to its path in the zip: when both body shapes hold the same files, they fold into the root
 * instead of shipping twice. `contents` is the item's path → hash map.
 */
function getItemZipPaths(contents: Record<string, string>): Record<string, string> {
  const stored = Object.keys(contents)
  const root = stored.filter(path => stripFolder(path) === null)
  let files = new Map(Object.entries(contents))
  let depth = 0
  // Some stored emotes nest the body shape folders twice, so the fold repeats until it reaches the files.
  for (let folded = foldBodyShapes(files); folded; folded = foldBodyShapes(files)) {
    files = folded
    depth++
  }
  // Names are compared the way a case-insensitive file system and the importer see them, so no folded file lands
  // on the item's own files or on another file.
  const names = [...files.keys()].filter(path => !root.includes(path)).map(getImportedName)
  const taken = new Set([...ITEM_FILES, ...root.map(path => path.toLowerCase())])
  if (new Set(names).size !== names.length || names.some(name => taken.has(name))) depth = 0
  return Object.fromEntries(
    stored.map(path => {
      let zipPath = path
      for (let level = 0; level < depth; level++) zipPath = stripFolder(zipPath) ?? zipPath
      return [path, zipPath]
    })
  )
}

export async function buildItemZip(item: Pick<Item, 'contents'>, files: Record<string, Blob>): Promise<Blob> {
  const zipPaths = getItemZipPaths(item.contents)
  const zip = new JSZip()
  for (const [path, blob] of Object.entries(files)) zip.file(zipPaths[path] ?? path, blob)
  return zip.generateAsync({ type: 'blob' })
}

/** "My Hat!" → "my-hat.zip"; falls back to the id when the name has no usable characters. */
export function getItemZipName(item: Item): string {
  const slug = item.name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return `${slug || item.id}.zip`
}

/** Downloads a saved item's files as the zip the importer accepts back. */
export async function downloadItem(item: Item): Promise<void> {
  downloadBlob(await buildItemZip(item, await fetchItemContents(item)), getItemZipName(item))
}
