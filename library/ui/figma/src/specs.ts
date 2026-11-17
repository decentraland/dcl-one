import { sourceRetirements } from './retirement-selection.js'
import { isRetired } from './guard/retirements.js'
import { configuredPages, loadPageSnapshot } from './page-identity.js'
import type { InventoryScope } from './generation-inventory.js'
import type { FigmaConfig } from './config.js'
import { generateIcons } from './generators/icons.js'
import type { ComponentSpec } from './ir/components.js'
import type { TextStyle, TokenCollection } from './ir/schemas.js'
import {
  buildComponentSpecsWithCollisions,
  type ComponentPageInput,
  type NameCollision
} from './mappers/component-spec.js'
import type { FigmaSource } from './sources/types.js'

type SpecDependencies = {
  collections: TokenCollection[]
  textStyles: TextStyle[]
  icons: Map<string, string>
}

type LoadedSpecs = {
  specs: ComponentSpec[]
  skippedPages: string[]
  nameCollisions: NameCollision[]
}

async function loadComponentSpecs(
  source: FigmaSource,
  config: FigmaConfig,
  dependencies: SpecDependencies,
  scope: InventoryScope = 'review'
): Promise<LoadedSpecs> {
  const pages: ComponentPageInput[] = []
  const skippedPages: string[] = []
  const inventory = await source.getGenerationInventory?.()
  // Single-file by construction: the snapshot, its inventory and retirements carry no file key (the capture
  // covers one file), so there is no file-identity check to make here.
  const retirements = await sourceRetirements(source)
  if (config.selection?.readyOnly && !inventory)
    throw new Error('Generation inventory required for ready-only generation')
  const configured = configuredPages(config)
  for (const entry of inventory?.[scope] ?? []) {
    if (!configured.some((page) => page.id === entry.pageId && page.kind === 'components'))
      throw new Error(`Inventory page is not configured: ${entry.pageId}`)
  }
  for (const page of configured) {
    if (page.kind !== 'components' || (page.id && isRetired(retirements, page.id))) continue
    const entry = inventory?.[scope].find((item) => item.pageId === page.id)
    if (inventory && !entry) {
      skippedPages.push(page.name)
      continue
    }
    const data = await loadPageSnapshot(source, page)
    if (!data) {
      if (entry) throw new Error(`Missing inventory snapshot: ${page.id} (${page.name})`)
      skippedPages.push(page.name)
      continue
    }
    const components = entry?.componentIds
      ? data.components.filter((component) => entry.componentIds!.includes(component.id))
      : data.components
    const selectedComponents =
      scope === 'promotion' && !inventory
        ? components.filter(
            (component) =>
              !config.selection?.exceptions?.some((entry) => entry.componentId === component.id)
          )
        : components
    if (
      entry?.componentIds?.some(
        (id) =>
          !isRetired(retirements, page.id!, id) &&
          !components.some((component) => component.id === id)
      )
    )
      throw new Error(`Missing inventory component on page ${page.id}`)
    pages.push({
      name: page.name,
      category: page.category ?? 'Components',
      data: {
        ...data,
        components: selectedComponents.filter(
          (component) => !isRetired(retirements, data.page.id, component.id)
        )
      }
    })
  }
  if (pages.length === 0) return { specs: [], skippedPages, nameCollisions: [] }
  const presentIds = new Set(
    pages.flatMap((page) => page.data.components.map((component) => component.id))
  )
  const reservedNames = retirements.retired
    .filter((record) => !record.componentId || !presentIds.has(record.componentId))
    .flatMap((record) =>
      record.owned
        .filter((artifact) =>
          artifact.startsWith('figma/review/src/generated/components/manifest.ts#component:')
        )
        .map((artifact) => artifact.split('#component:')[1]!)
    )
  const built = buildComponentSpecsWithCollisions({ pages, config, reservedNames, ...dependencies })
  return { specs: built.specs, skippedPages, nameCollisions: built.collisions }
}

// The MCP tools need the same specs the generator emits, without writing any file.
async function loadSpecsFromSource(
  source: FigmaSource,
  config: FigmaConfig,
  scope: InventoryScope = 'review'
): Promise<LoadedSpecs> {
  const [collections, textStyles, icons] = await Promise.all([
    source.getTokenCollections(),
    source.getTextStyles(),
    source.getIcons()
  ])
  const { meta } = generateIcons(icons, {})
  return loadComponentSpecs(
    source,
    config,
    {
      collections,
      textStyles,
      icons: new Map(meta.map((entry) => [entry.name, entry.component]))
    },
    scope
  )
}

export { loadComponentSpecs, loadSpecsFromSource }
export type { LoadedSpecs, SpecDependencies }
