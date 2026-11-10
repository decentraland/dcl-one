import { componentOwnerById } from './component-identity.js'
import { componentIdOfFile } from './ownership.js'
import path from 'node:path'
import { exportStatements, publicExportSet, removePublicExports } from './exports.js'
import { atomicWrite, listFiles, readIfExists, resolveContained, toPosix } from './fs-safe.js'
import type { Ownership } from './ownership.js'

const LIBRARY_GENERATED = 'components/'
const LIBRARY_INDEX = 'index.ts'
const LIBRARY_PROMOTION = 'components/promotion.json'
type Target = { pageId: string; componentId?: string }
function ownsComponent(ownership: Ownership, target: Target, id: string): boolean {
  const owner = componentOwnerById(ownership, id)
  return (
    owner?.pageId === target.pageId &&
    (target.componentId === undefined || owner.componentId === target.componentId)
  )
}
function retiredNames(ownership: Ownership, target: Target): string[] {
  return [...ownership.componentIdByName]
    .filter(([, id]) => ownsComponent(ownership, target, id))
    .map(([name]) => name)
}
async function libraryOwned(root: string, ownership: Ownership, target: Target): Promise<string[]> {
  const names = retiredNames(ownership, target)
  const bases = [...ownership.fileComponentId]
    .filter(([, id]) => ownsComponent(ownership, target, id))
    .map(([base]) => base)
  const owned: string[] = []
  for (const file of await listFiles(path.join(root, LIBRARY_GENERATED))) {
    const relative = toPosix(path.relative(root, file))
    const id = componentIdOfFile(ownership, relative)
    if (id && ownsComponent(ownership, target, id)) owned.push(relative)
  }
  const index = (await readIfExists(await resolveContained(root, LIBRARY_INDEX))) ?? ''
  for (const statement of exportStatements(index))
    if (bases.some((base) => statement.from === `./components/${base}`))
      owned.push(...statement.symbols.map((symbol) => `${LIBRARY_INDEX}#${symbol.key}`))
  const promotion = await readIfExists(await resolveContained(root, LIBRARY_PROMOTION))
  const promoted = promotion === undefined ? [] : (JSON.parse(promotion).components as string[])
  for (const name of names)
    if (promoted?.includes(name)) owned.push(`${LIBRARY_PROMOTION}#component:${name}`)
  return owned.sort()
}
async function libraryEntryPresent(root: string, artifact: string): Promise<boolean> {
  const [file, key] = artifact.split('#') as [string, string]
  const text = await readIfExists(await resolveContained(root, file))
  if (text === undefined) return false
  if (file === LIBRARY_INDEX) return publicExportSet(text).has(key)
  if (file === LIBRARY_PROMOTION)
    return ((JSON.parse(text).components as string[]) ?? []).includes(
      key.replace(/^component:/, '')
    )
  return false
}
async function removeLibraryEntries(root: string, owned: string[]): Promise<string[]> {
  const removed: string[] = []
  for (const file of [LIBRARY_INDEX, LIBRARY_PROMOTION]) {
    const keys = owned
      .filter((artifact) => artifact.startsWith(`${file}#`))
      .map((artifact) => artifact.slice(file.length + 1))
    if (!keys.length) continue
    const target = await resolveContained(root, file)
    const text = await readIfExists(target)
    if (text === undefined) continue
    let next: string
    if (file === LIBRARY_INDEX) next = removePublicExports(text, new Set(keys))
    else {
      const data = JSON.parse(text)
      next =
        JSON.stringify(
          {
            ...data,
            components: (data.components ?? []).filter(
              (name: string) => !keys.includes(`component:${name}`)
            )
          },
          null,
          2
        ) + '\n'
    }
    if (next !== text) {
      await atomicWrite(target, next)
      removed.push(...keys.map((key) => `${file}#${key}`))
    }
  }
  return removed
}
export {
  LIBRARY_GENERATED,
  LIBRARY_INDEX,
  LIBRARY_PROMOTION,
  libraryOwned,
  libraryEntryPresent,
  removeLibraryEntries,
  retiredNames
}
