import type { FigmaSource } from './sources/types.js'
import type { FigmaConfig } from './config.js'

type PageState = { id: string; name: string; ready: boolean | null }
type ConfiguredPage = FigmaConfig['pages'][string] & {
  key: string
  id: string | null
  name: string
}
type ResolvedPage = ConfiguredPage & {
  liveName?: string
  ready: boolean | null
  renamed: boolean
  legacy: boolean
}

function configuredPages(config: Pick<FigmaConfig, 'pages'>): ConfiguredPage[] {
  const ids = new Set<string>()
  return Object.entries(config.pages).map(([key, page]) => {
    const id = page.id === undefined ? (/^\d+:\d+$/.test(key) ? key : null) : page.id
    if (id !== null && ids.has(id)) throw new Error(`Ambiguous configured page ID: ${id}`)
    if (id !== null && /^\d+:\d+$/.test(key) && key !== id)
      throw new Error(`Configured page key ${key} does not match ID ${id}`)
    if (id !== null) ids.add(id)
    return { ...page, key, id, name: page.name ?? key }
  })
}

function resolvePageIdentities(config: FigmaConfig, live: PageState[]): ResolvedPage[] {
  return configuredPages(config).map((page) => {
    if (page.id === null) return { ...page, ready: null, legacy: true, renamed: false }
    const matches = live.filter((item) => item.id === page.id)
    if (matches.length === 0)
      throw new Error(`Missing configured Figma page: ${page.id} (${page.name})`)
    if (matches.length !== 1) throw new Error(`Ambiguous Figma page ID: ${page.id}`)
    const current = matches[0]!
    return {
      ...page,
      liveName: current.name,
      ready: current.ready,
      renamed: current.name !== page.name,
      legacy: false
    }
  })
}

function selectCapturePages(pages: ResolvedPage[], requested?: string): ResolvedPage[] {
  if (!requested) return pages.filter((page) => page.id !== null && page.ready === true)
  const byId = pages.find((page) => page.id === requested)
  const matches = byId ? [byId] : pages.filter((page) => page.liveName === requested)
  if (!matches.length) throw new Error(`Unknown Figma page: ${requested}`)
  if (matches.length !== 1) throw new Error(`Ambiguous Figma page: ${requested}`)
  if (matches[0]!.ready !== true) throw new Error(`Page is not ready for sync: ${requested}`)
  return matches
}

function mergeSyncState(
  previous: PageState[],
  live: PageState[],
  selectedIds: string[]
): PageState[] {
  const selected = new Set(selectedIds)
  const result = previous.map((page) =>
    selected.has(page.id) ? (live.find((item) => item.id === page.id) ?? page) : page
  )
  for (const page of live)
    if (selected.has(page.id) && !result.some((item) => item.id === page.id)) result.push(page)
  return result
}

async function loadPageSnapshot(source: FigmaSource, page: Pick<ConfiguredPage, 'id' | 'name'>) {
  const data =
    page.id && source.getComponentPageById
      ? await source.getComponentPageById(page.id, page.name)
      : await source.getComponentPage(page.name)
  if (data && page.id && data.page.id !== page.id)
    throw new Error(`Snapshot page identity changed: ${page.name}`)
  return data
}

export {
  configuredPages,
  resolvePageIdentities,
  selectCapturePages,
  mergeSyncState,
  loadPageSnapshot
}
export type { ConfiguredPage, ResolvedPage, PageState }
