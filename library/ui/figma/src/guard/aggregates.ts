import { componentOwner, componentOwnerById } from './component-identity.js'
import { exportStatements } from './exports.js'
import { AGGREGATES, SNAPSHOT, type Ownership } from './ownership.js'

// A shared aggregate holds one entry per component or page. The guard compares entries one by one so an
// automated run may add or update the entries of the pages it captured while every other entry stays
// byte-identical. `rest` is everything that is not an entry (headers, foundations).
type Entry = { owner: string | null; value: string }
type ParsedAggregate = { rest: string; entries: Map<string, Entry>; order: string[] }

const json = (value: unknown) => JSON.stringify(value)

function fromList<T>(
  items: T[],
  keyOf: (item: T) => string,
  ownerOfItem: (item: T) => string | null,
  prefix: string,
  into: Map<string, Entry>,
  order: string[]
): void {
  for (const item of items) {
    const key = `${prefix}:${keyOf(item)}`
    if (into.has(key)) throw new Error(`duplicate entry ${key}`)
    into.set(key, { owner: ownerOfItem(item), value: json(item) })
    order.push(key)
  }
}

type PageEntry = { id: string }
type ComponentEntry = { name: string; page: string; id?: string; figmaId?: string }
type InventoryEntry = { pageId: string; componentIds?: string[] }
const byId2 = (item: InventoryEntry) => item.pageId
const idOwner2 = (item: InventoryEntry) => item.pageId

// Returns undefined for a file that is not an entry-level aggregate (it is then compared as a whole).
function parseAggregate(
  aggregate: string,
  text: string,
  ownership: Ownership
): ParsedAggregate | undefined {
  const entries = new Map<string, Entry>()
  const order: string[] = []
  const byId = (item: PageEntry) => item.id
  const idOwner = (item: PageEntry) => item.id
  const componentIdOwner = (item: ComponentEntry) => {
    const id = item.figmaId ?? item.id
    return id ? (componentOwnerById(ownership, id)?.pageId ?? null) : null
  }

  if (aggregate === 'library-index') {
    let rest = text
    for (const statement of exportStatements(text)) {
      if (!statement.from.startsWith('./components/')) continue
      const file = statement.from.slice('./components/'.length)
      const id = ownership.fileComponentId.get(file)
      for (const symbol of statement.symbols) {
        if (entries.has(symbol.key)) throw new Error(`duplicate entry ${symbol.key}`)
        entries.set(symbol.key, {
          owner: id ? (componentOwnerById(ownership, id)?.pageId ?? null) : null,
          value: `${statement.from}#${symbol.raw}`
        })
        order.push(symbol.key)
      }
      rest = rest.replace(statement.text, '')
    }
    return {
      rest: rest
        .split('\n')
        .filter((line) => line.trim() && !/^\/\/ (BEGIN|END) PROMOTED COMPONENTS$/.test(line))
        .join('\n'),
      entries,
      order
    }
  }
  if (aggregate === 'library-promotion') {
    const data = JSON.parse(text) as { components?: string[]; files?: Record<string, string> }
    const { components = [], ...rest } = data
    delete rest.files
    fromList(
      components,
      (name) => name,
      (name) => componentOwner(ownership, name)?.pageId ?? null,
      'component',
      entries,
      order
    )
    return { rest: json(rest), entries, order }
  }
  if (aggregate === 'catalog') {
    const data = JSON.parse(text) as {
      components?: ComponentEntry[]
      pages?: PageEntry[]
    } & Record<string, unknown>
    const { components = [], pages = [], ...rest } = data
    fromList(components, (c) => c.name, componentIdOwner, 'component', entries, order)
    fromList(pages, byId, idOwner, 'page', entries, order)
    return { rest: json(rest), entries, order }
  }

  if (aggregate === 'sync-state' || aggregate === 'pages') {
    const data = JSON.parse(text) as { pages?: PageEntry[] } & Record<string, unknown>
    const { pages = [], ...rest } = data
    fromList(pages, byId, idOwner, 'page', entries, order)
    return { rest: json(rest), entries, order }
  }

  if (aggregate === 'live-source') {
    const data = JSON.parse(text) as { pages?: Record<string, unknown> } & Record<string, unknown>
    const { pages = {}, ...rest } = data
    for (const [id, value] of Object.entries(pages)) {
      entries.set(`page:${id}`, { owner: id, value: json(value) })
      order.push(`page:${id}`)
    }
    return { rest: json(rest), entries, order }
  }

  if (aggregate === 'snapshot') {
    const data = JSON.parse(text) as {
      page: { id: string }
      components?: { id: string; base?: { id: string }; others?: { id: string }[] }[]
    }
    const { components = [], ...rest } = data
    fromList(
      components,
      (component) => component.id,
      () => data.page.id,
      'component',
      entries,
      order
    )
    for (const component of components)
      for (const variant of [component.base, ...(component.others ?? [])])
        if (variant)
          fromList(
            [variant],
            (item) => item.id,
            () => data.page.id,
            'variant',
            entries,
            order
          )
    return { rest: json(rest), entries, order }
  }
  if (aggregate === 'promotion-manifest') {
    const data = JSON.parse(text) as {
      components?: { name: string }[]
      excluded?: { name: string }[]
      files?: Record<string, string>
    }
    const { components = [], excluded = [], ...metadata } = data
    const rest = { ...metadata }
    delete rest.files
    fromList(
      components,
      (item) => item.name,
      (item) => componentOwner(ownership, item.name)?.pageId ?? null,
      'component',
      entries,
      order
    )
    fromList(
      excluded,
      (item) => item.name,
      (item) => componentOwner(ownership, item.name)?.pageId ?? null,
      'excluded',
      entries,
      order
    )
    return { rest: json(rest), entries, order }
  }
  if (aggregate === 'inventory') {
    const data = JSON.parse(text) as {
      review?: InventoryEntry[]
      promotion?: InventoryEntry[]
    } & Record<string, unknown>
    const { review = [], promotion = [], ...rest } = data
    fromList(review, byId2, idOwner2, 'review', entries, order)
    fromList(promotion, byId2, idOwner2, 'promotion', entries, order)
    for (const [scope, items] of [
      ['review', review],
      ['promotion', promotion]
    ] as const)
      for (const page of items)
        for (const id of page.componentIds ?? []) {
          const key = `${scope}:${page.pageId}:component:${id}`
          entries.set(key, { owner: page.pageId, value: id })
          order.push(key)
        }
    return { rest: json(rest), entries, order }
  }

  if (aggregate === 'components-index') {
    const lines = text.split('\n')
    const rest: string[] = []
    for (const line of lines) {
      const match = /^export \* from '\.\/(.+)'$/.exec(line)
      if (!match) {
        rest.push(line)
        continue
      }
      const id = ownership.fileComponentId.get(match[1]!)
      const key = `export:${match[1]}`
      if (entries.has(key)) throw new Error(`duplicate entry ${key}`)
      entries.set(key, {
        owner: id ? (componentOwnerById(ownership, id)?.pageId ?? null) : null,
        value: line
      })
      order.push(key)
    }
    return { rest: rest.join('\n'), entries, order }
  }

  if (aggregate === 'components-manifest') {
    const start = text.indexOf('= [')
    if (start < 0) return undefined
    const items = JSON.parse(text.slice(start + 2)) as ComponentEntry[]
    fromList(items, (c) => c.name, componentIdOwner, 'component', entries, order)
    return { rest: text.slice(0, start + 2), entries, order }
  }

  return undefined
}

type RetirementTarget = { pageId: string; componentId?: string }

// Whether an aggregate entry belongs to a retired page or component. Page listings (`page:`) are never
// the retired thing: the page still exists in Figma.
function entryBelongsTo(
  key: string,
  entry: Entry,
  target: RetirementTarget,
  ownership: Ownership
): boolean {
  if (key.startsWith('page:')) return false
  if (entry.owner !== target.pageId) return false
  if (target.componentId && key.endsWith(`:component:${target.componentId}`)) return true
  if (target.componentId && key === `component:${target.componentId}`) return true
  if (target.componentId && key.startsWith('variant:'))
    return ownership.variantRoot.get(key.slice('variant:'.length)) === target.componentId
  const name =
    target.componentId === undefined ? undefined : ownership.componentById.get(target.componentId)
  if (
    target.componentId !== undefined &&
    (name === undefined || componentOwner(ownership, name)?.componentId !== target.componentId)
  )
    return false
  if (name === undefined) return entry.owner === target.pageId
  if (key.startsWith('export:'))
    return ownership.fileComponentId.get(key.slice('export:'.length)) === target.componentId
  const component = key.replace(/^(component|excluded|review|promotion):/, '').replace(/-\d+$/, '')
  return component === name
}

function aggregateKind(file: string): string | undefined {
  return (
    AGGREGATES[file] ??
    (file.startsWith(`${SNAPSHOT}/components/`) && file.endsWith('.json') ? 'snapshot' : undefined)
  )
}

export { entryBelongsTo, parseAggregate, aggregateKind }
export type { Entry, ParsedAggregate, RetirementTarget }
