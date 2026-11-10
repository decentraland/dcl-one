import {
  componentRegistryEntries,
  componentRegistryEntryName
} from '../../component-generation.mjs'
import { componentOwnerById, componentIdOfFile } from './component-identity.js'
import path from 'node:path'
import { exportStatements, publicExportSet, removePublicExports } from './exports.js'
import { atomicWrite, listFiles, readIfExists, resolveContained, toPosix } from './fs-safe.js'
import type { Ownership } from './ownership.js'

const LIBRARY_GENERATED = 'components/'
const LIBRARY_INDEX = 'index.ts'
const LIBRARY_PROMOTION = 'components/promotion.json'
type AcceptedLimit = { id: string; text: string }
type LibraryComponentEntry = {
  name: string
  visibility: 'public' | 'internal'
  limits?: AcceptedLimit[]
}
function libraryComponentEntries(value: unknown): LibraryComponentEntry[] {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Invalid library component registry')
  const data = value as { components?: unknown; internal?: unknown }
  if (
    (data.components !== undefined && !Array.isArray(data.components)) ||
    (data.internal !== undefined && !Array.isArray(data.internal))
  )
    throw new Error('Invalid library component registry name list')
  // Names and accepted limits are validated once, by the same reader the installer uses.
  let parsed: { components: string[]; internal: string[]; limits: Record<string, AcceptedLimit[]> }
  try {
    // Other keys of the registry document are the guard's concern (aggregates keeps them), not entries.
    parsed = componentRegistryEntries({
      components: data.components ?? [],
      ...(data.internal === undefined ? {} : { internal: data.internal })
    })
  } catch (error) {
    throw new Error('Invalid library component registry: ' + (error as Error).message, {
      cause: error
    })
  }
  const entries: LibraryComponentEntry[] = []
  const names = new Set<string>()
  for (const [list, visibility] of [
    [parsed.components, 'public'],
    [parsed.internal, 'internal']
  ] as const) {
    for (const name of list) {
      if (typeof name !== 'string' || !/^[A-Za-z_$][\w$]*$/.test(name) || names.has(name))
        throw new Error('Invalid or duplicate library component registry name')
      names.add(name)
      const limits = Object.hasOwn(parsed.limits, name) ? parsed.limits[name] : undefined
      entries.push(limits ? { name, visibility, limits } : { name, visibility })
    }
  }
  return entries
}
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
  const promoted =
    promotion === undefined
      ? []
      : libraryComponentEntries(JSON.parse(promotion)).map((entry) => entry.name)
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
    return libraryComponentEntries(JSON.parse(text)).some(
      (entry) => entry.name === key.replace(/^component:/, '')
    )
  return false
}
async function removeLibraryEntries(root: string, owned: string[]): Promise<string[]> {
  const promotion = await readIfExists(await resolveContained(root, LIBRARY_PROMOTION))
  if (promotion !== undefined) libraryComponentEntries(JSON.parse(promotion))
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
      libraryComponentEntries(data)
      // Entries are names or { name, limits }; match by the registry's own entry name, never the raw value.
      const kept = (entries: unknown[]) =>
        entries.filter((entry) => !keys.includes(`component:${componentRegistryEntryName(entry)}`))
      next =
        JSON.stringify(
          {
            ...data,
            components: kept(data.components ?? []),
            ...(data.internal === undefined ? {} : { internal: kept(data.internal) })
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
  libraryComponentEntries,
  LIBRARY_GENERATED,
  LIBRARY_INDEX,
  LIBRARY_PROMOTION,
  libraryOwned,
  libraryEntryPresent,
  removeLibraryEntries,
  retiredNames
}
