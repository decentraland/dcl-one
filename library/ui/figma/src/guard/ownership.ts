import {
  componentOwner,
  componentOwnerById,
  componentIdOfFile,
  type ComponentOwner
} from './component-identity.js'
import { assetUsers } from './asset-users.js'
import { CACHED_LEDGERS, LEDGER, RETIREMENTS } from './consumers.js'
import { exportStatements } from './exports.js'
import { configuredPages } from '../page-identity.js'
import type { FigmaConfig } from '../config.js'
import path from 'node:path'
import { listFiles, readIfExists, toPosix } from './fs-safe.js'
import { LIBRARY_GENERATED, LIBRARY_INDEX, LIBRARY_PROMOTION } from './library.js'

// Component artifacts keep stable page and component IDs. Shared containers are merged by entry,
// and shared assets retain all source consumers, including foreign code.
type Owner =
  | { pageId: string; componentId?: string }
  | { aggregate: string; users?: ComponentOwner[]; foreignUsers?: string[] }

const GENERATED = 'figma/review/src/generated'
const REVIEW = 'figma/review/src/review'
const PUBLIC = 'figma/review/public/dcl-ui-review'
const SNAPSHOT = 'figma/figma-snapshot'
const CONFIG = 'figma/figma.config.json'

// Roots (relative to the repository) the guard watches. Paths in reports are relative to the same root.
const GUARDED_DIRECTORIES = [
  GENERATED,
  'figma/review/src/components',
  // Nothing here is generated or published any more: any file under it is unowned and rejected.
  PUBLIC,
  SNAPSHOT
] as const
const GUARDED_FILES = [
  LIBRARY_PROMOTION,
  // The committed ledgers sit outside the snapshot folder.
  LEDGER,
  RETIREMENTS,
  `${REVIEW}/catalog.json`,
  `${REVIEW}/promotion.json`,
  CONFIG
] as const

const AGGREGATES: Record<string, string> = {
  'index.ts': 'library-index',
  'components/promotion.json': 'library-promotion',
  [`${GENERATED}/components/index.ts`]: 'components-index',
  [`${GENERATED}/components/manifest.ts`]: 'components-manifest',
  [`${REVIEW}/catalog.json`]: 'catalog',
  [`${REVIEW}/promotion.json`]: 'promotion-manifest',
  [`${SNAPSHOT}/sync-state.json`]: 'sync-state',
  [`${SNAPSHOT}/live-source.json`]: 'live-source',
  [`${SNAPSHOT}/pages.json`]: 'pages',
  [`${SNAPSHOT}/generation-inventory.json`]: 'inventory',
  [RETIREMENTS]: 'retirements',
  // Validated as a whole by checkLedger.
  [LEDGER]: 'orphan-ledger',
  // fetch-source's copies of the two ledgers in the cache, pinned by its digest table.
  [CACHED_LEDGERS[0]!]: 'orphan-ledger',
  [CACHED_LEDGERS[1]!]: 'retirements',
  // Written by `pnpm figma fetch-source` next to the cache it builds: the revision and digest table it came from.
  [`${SNAPSHOT}/.fds-source.json`]: 'fds-source',
  [CONFIG]: 'config'
}

// Inputs that feed every page (tokens, icons, styles). They change only when their own page is captured.
const FOUNDATION_FILES = new Set([
  `${SNAPSHOT}/variables.json`,
  `${SNAPSHOT}/text-styles.json`,
  `${SNAPSHOT}/paint-styles.json`,
  `${SNAPSHOT}/icons.json`,
  `${SNAPSHOT}/icons-rows.txt`
])
const FOUNDATION_PREFIXES = [
  `${SNAPSHOT}/icons/`,
  `${GENERATED}/tokens/`,
  `${GENERATED}/icons/`,
  'components/tokens/',
  'components/icons/'
]

type Ownership = {
  pageNames: Map<string, string> // page ID -> report metadata only; never an ownership claim
  fileComponentId: Map<string, string> // generated module -> component ID recorded in the manifest
  componentById: Map<string, string> // component ID -> raw/public label metadata; never an identity claim
  componentPageById: Map<string, string>
  variantRoot: Map<string, string>
  artifactIdentity: Map<string, string>
  componentIdByName: Map<string, string>
  assetUsers: Map<string, string[]> // asset path -> every source consumer path
  // Variant ID -> every saved top-level component that lists it, as serialized VariantParent. Unlike
  // variantRoot, this keeps every claim, so a caller can tell a unique parent from an ambiguous one.
  variantParents: Map<string, string[]>
  // Places where one tree claims two different owners for the same key.
  conflicts: string[]
}

const emptyOwnership = (): Ownership => ({
  pageNames: new Map(),
  fileComponentId: new Map(),
  componentById: new Map(),
  componentPageById: new Map(),
  variantRoot: new Map(),
  artifactIdentity: new Map(),
  componentIdByName: new Map(),
  assetUsers: new Map(),
  variantParents: new Map(),
  conflicts: []
})

type OwnershipMap = Exclude<
  keyof Ownership,
  'conflicts' | 'assetUsers' | 'variantParents' | 'pageNames'
>

// First claim wins; a different second claim inside the same tree is recorded, not applied silently.
function claim(ownership: Ownership, map: OwnershipMap, key: string, value: string): void {
  const existing = ownership[map].get(key)
  if (existing === undefined) ownership[map].set(key, value)
  else if (existing !== value)
    ownership.conflicts.push(`${map} "${key}" claimed as ${existing} and ${value}`)
}

// A file that does not parse yields no ownership. The entry-level check then reports that same file as
// unreadable, so a broken aggregate fails the run instead of weakening it silently.
function parseJson(text: string | undefined): unknown {
  if (text === undefined) return undefined
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

async function readJson(file: string): Promise<unknown> {
  return parseJson(await readIfExists(file))
}

type NamedPage = { id: string; name: string }
type SnapshotVariant = { id?: string }
type VariantParent = {
  pageId: string
  componentId: string
  kind: string
  truncated: boolean
  external: boolean
}
type SnapshotPage = {
  page?: NamedPage
  components?: {
    id?: string
    name: string
    kind?: string
    truncated?: boolean
    external?: boolean
    base?: SnapshotVariant
    others?: SnapshotVariant[]
  }[]
}

async function buildOwnership(root: string): Promise<Ownership> {
  const ownership = emptyOwnership()
  const config = (await readJson(path.join(root, CONFIG))) as Pick<FigmaConfig, 'pages'> | undefined
  for (const page of config ? configuredPages(config) : [])
    if (page.id) ownership.pageNames.set(page.id, page.name)
  const savedPages: { relative: string; saved: SnapshotPage & { page: NamedPage } }[] = []
  const snapshot = path.join(root, SNAPSHOT, 'components')
  for (const file of await listFiles(snapshot)) {
    const relative = toPosix(path.relative(snapshot, file))
    if (relative.includes('/') || !relative.endsWith('.json')) continue
    const saved = (await readJson(file)) as SnapshotPage | undefined
    if (typeof saved?.page?.id !== 'string' || !saved.page.id.trim()) {
      ownership.conflicts.push(`Invalid snapshot page identity: ${SNAPSHOT}/components/${relative}`)
      continue
    }
    savedPages.push({ relative, saved: { ...saved, page: saved.page } })
    ownership.artifactIdentity.set(
      `${SNAPSHOT}/components/${relative}`,
      JSON.stringify({ pageId: saved.page.id })
    )
    if (!ownership.pageNames.has(saved.page.id))
      ownership.pageNames.set(saved.page.id, saved.page.name ?? saved.page.id)
    for (const component of saved.components ?? [])
      if (component.id && !component.external)
        claim(ownership, 'componentPageById', component.id, saved.page.id)
  }

  const manifestText = await readIfExists(path.join(root, GENERATED, 'components/manifest.ts'))
  if (manifestText !== undefined) {
    const start = manifestText.indexOf('= [')
    const entries = ((start < 0 ? [] : parseJson(manifestText.slice(start + 2))) ?? []) as {
      name: string
      page: string
      file: string
      figmaId?: string
      id?: string
    }[]
    for (const entry of entries) {
      const id = entry.figmaId ?? entry.id
      if (id) {
        claim(ownership, 'fileComponentId', entry.file, id)
        claim(ownership, 'componentById', id, entry.name)
        claim(ownership, 'componentIdByName', entry.name, id)
      }
    }
  }

  const catalog = (await readJson(path.join(root, REVIEW, 'catalog.json'))) as
    | {
        components?: { name: string; page: string; id?: string; variants?: { id: string }[] }[]
      }
    | undefined
  for (const component of catalog?.components ?? []) {
    if (component.id) {
      claim(ownership, 'componentById', component.id, component.name)
      claim(ownership, 'componentIdByName', component.name, component.id)
    }
    for (const variant of component.variants ?? []) {
      if (component.id) claim(ownership, 'variantRoot', variant.id, component.id)
    }
  }

  // Raw component IDs remain available for retirement even when they were never generated.
  for (const { saved } of savedPages) {
    for (const component of saved.components ?? []) {
      if (component.id && !ownership.componentById.has(component.id))
        ownership.componentById.set(component.id, component.name)
    }
  }
  for (const { relative, saved } of savedPages)
    for (const component of saved.components ?? []) {
      if (!component.id) continue
      const identity = JSON.stringify({ pageId: saved.page.id, componentId: component.id })
      ownership.artifactIdentity.set(
        `${SNAPSHOT}/components/${relative}#component:${component.id}`,
        identity
      )
      const parent: VariantParent = {
        pageId: saved.page.id,
        componentId: component.id,
        kind: component.kind ?? 'unknown',
        truncated: component.truncated === true,
        external: component.external === true
      }
      for (const variant of [component.base, ...(component.others ?? [])])
        if (variant?.id)
          ownership.variantParents.set(variant.id, [
            ...(ownership.variantParents.get(variant.id) ?? []),
            JSON.stringify(parent)
          ])
      for (const variant of [component.base, ...(component.others ?? [])])
        if (variant?.id) {
          if (!ownership.variantRoot.has(variant.id))
            ownership.variantRoot.set(variant.id, component.id)
          ownership.artifactIdentity.set(
            `${SNAPSHOT}/components/${relative}#variant:${variant.id}`,
            identity
          )
        }
    }
  for (const statement of exportStatements(
    (await readIfExists(path.join(root, LIBRARY_INDEX))) ?? ''
  )) {
    const prefix = './components/'
    if (!statement.from.startsWith(prefix)) continue
    const id = ownership.fileComponentId.get(statement.from.slice(prefix.length))
    const owner = id ? componentOwnerById(ownership, id) : null
    if (owner)
      for (const symbol of statement.symbols)
        ownership.artifactIdentity.set(`${LIBRARY_INDEX}#${symbol.key}`, JSON.stringify(owner))
  }
  for (const [directory, source] of [
    [`${GENERATED}/components/assets`, 'figma/review/src'],
    [`${LIBRARY_GENERATED}assets`, 'components']
  ])
    for (const [asset, users] of await assetUsers(root, directory!, source!))
      ownership.assetUsers.set(asset, users)
  return ownership
}

// The base tree is the trusted source of ownership. The staged tree may only add keys the base does not
// have; if it tries to move an existing component or artifact to another owner, that is reported
// as a remap and the base answer stays, so a run cannot make a frozen artifact look like its own.
type Remap = { map: OwnershipMap; key: string; from: string; to: string }
const describeRemap = ({ map, key, from, to }: Remap) =>
  `${map} "${key}" moved from ${from} to ${to}`

function mergeOwnership(
  base: Ownership,
  staged: Ownership
): { ownership: Ownership; remaps: Remap[] } {
  const ownership = emptyOwnership()
  const remaps: Remap[] = []
  ownership.conflicts.push(
    ...base.conflicts.map((c) => `base: ${c}`),
    ...staged.conflicts.map((c) => `staged: ${c}`)
  )
  for (const key of Object.keys(ownership).filter(
    (name) =>
      name !== 'conflicts' &&
      name !== 'assetUsers' &&
      name !== 'variantParents' &&
      name !== 'pageNames'
  ) as OwnershipMap[]) {
    for (const [name, value] of base[key]) ownership[key].set(name, value)
    for (const [name, value] of staged[key]) {
      const trusted = ownership[key].get(name)
      if (trusted === undefined) ownership[key].set(name, value)
      // A raw label may gain a generated public name without changing its component/page IDs.
      else if (trusted !== value && key !== 'componentById')
        remaps.push({ map: key, key: name, from: trusted, to: value })
    }
  }
  // Display labels may change or collide without changing any identity.
  for (const source of [base, staged])
    for (const [id, name] of source.pageNames)
      if (!ownership.pageNames.has(id)) ownership.pageNames.set(id, name)
  for (const source of [base, staged])
    for (const [variant, parents] of source.variantParents)
      ownership.variantParents.set(variant, [
        ...new Set([...(ownership.variantParents.get(variant) ?? []), ...parents])
      ])
  for (const source of [base, staged])
    for (const [asset, users] of source.assetUsers)
      ownership.assetUsers.set(asset, [
        ...new Set([...(ownership.assetUsers.get(asset) ?? []), ...users])
      ])
  return { ownership, remaps }
}

const startsWithAny = (value: string, prefixes: readonly string[]) =>
  prefixes.some((prefix) => value.startsWith(prefix))

// Pure: the answer comes from the ownership map and the path alone.
function ownerOf(ownership: Ownership, file: string): Owner | null {
  const identity = ownership.artifactIdentity.get(file)
  if (identity) return JSON.parse(identity) as Owner
  const [aggregateFile, key] = file.split('#')
  if (key && aggregateFile) {
    const kind = AGGREGATES[aggregateFile]
    if (
      ['catalog', 'components-manifest', 'promotion-manifest', 'library-promotion'].includes(
        kind ?? ''
      )
    )
      return componentOwner(ownership, key.replace(/^(component|excluded):/, ''))
    if (kind === 'components-index') {
      const id = ownership.fileComponentId.get(key.replace(/^export:/, ''))
      return id ? componentOwnerById(ownership, id) : null
    }
  }
  if (file.startsWith('figma/review/src/components/')) return { aggregate: 'runtime' }
  const aggregate = AGGREGATES[file]
  if (aggregate) return { aggregate }
  if (FOUNDATION_FILES.has(file) || startsWithAny(file, FOUNDATION_PREFIXES))
    return { aggregate: 'foundations' }
  if (
    file.startsWith(`${GENERATED}/components/assets/`) ||
    file.startsWith(`${LIBRARY_GENERATED}assets/`)
  ) {
    const users = new Map<string, ComponentOwner>()
    const foreignUsers: string[] = []
    for (const source of ownership.assetUsers.get(file) ?? []) {
      const id = componentIdOfFile(ownership, source)
      const user = id ? componentOwnerById(ownership, id) : null
      if (user) users.set(JSON.stringify(user), user)
      else foreignUsers.push(source)
    }
    return { aggregate: 'assets', users: [...users.values()], foreignUsers }
  }

  const id = componentIdOfFile(ownership, file)
  if (id) return componentOwnerById(ownership, id)

  return null
}

// Public labels are metadata for selectors; only the recorded module/component ID establishes ownership.
function componentNameOfFile(ownership: Ownership, file: string): string | undefined {
  const id = componentIdOfFile(ownership, file)
  return id ? ownership.componentById.get(id) : undefined
}

export {
  AGGREGATES,
  CONFIG,
  GENERATED,
  GUARDED_DIRECTORIES,
  GUARDED_FILES,
  REVIEW,
  SNAPSHOT,
  buildOwnership,
  describeRemap,
  emptyOwnership,
  mergeOwnership,
  ownerOf,
  componentOwner,
  componentOwnerById,
  componentNameOfFile,
  componentIdOfFile
}
export type { Owner, Ownership, ComponentOwner, Remap, VariantParent }
