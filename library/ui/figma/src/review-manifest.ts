import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import type { App } from './app.js'
import type { FigmaConfig } from './config.js'
import { expandVariants } from './capture/expand-variants.js'
import { cssName, TokenIndex } from './generators/tokens.js'
import { textStyleClass } from './generators/naming.js'
import type { RawNode } from './ir/raw-components.js'
import { mapVariants, viewKey } from './mappers/variants.js'
import { loadSpecsFromSource } from './specs.js'
import type { FigmaSource } from './sources/types.js'

function visibleTexts(node: RawNode): string[] {
  if (node.hidden || node.opacity === 0) return []
  return [
    ...(node.type === 'TEXT' && node.text ? [node.text] : []),
    ...(node.children ?? []).flatMap(visibleTexts)
  ]
}

// Catalog entries in inventory order, plus the stroke widths derived from every variant tree. The scope
// guard recomputes the same stroke widths over a staged snapshot, so both must come from this function.
async function reviewComponents(source: FigmaSource, config: FigmaConfig) {
  const { specs } = await loadSpecsFromSource(source, config)
  const aliases = Object.fromEntries(
    Object.entries(config.variantAliases).map(([key, value]) => [key.toLowerCase(), value])
  )
  const strokeWidths = new Set<number>()
  const collectStrokes = (node: RawNode) => {
    for (const width of node.strokeWeights ??
      (typeof node.strokeWeight === 'number' ? [node.strokeWeight] : []))
      if (width > 0) strokeWidths.add(width)
    node.children?.forEach(collectStrokes)
  }
  const components = []
  for (const spec of specs) {
    const page = await source.getComponentPage(spec.page)
    const raw = page?.components.find((component) => component.id === spec.figmaId)
    if (!raw) throw new Error(`Missing review capture for ${spec.name}`)
    const expanded = expandVariants(raw)
    const propertiesById = new Map(expanded.map((variant) => [variant.id, variant.props]))
    const mapped = mapVariants(raw, expanded, {
      wordAliases: aliases,
      ...(config.components[raw.name] ?? {})
    })
    const exception = config.selection?.exceptions.find(
      (item) => item.pageId === page!.page.id && item.componentId === raw.id
    )
    components.push({
      name: spec.name,
      page: spec.page,
      id: spec.figmaId,
      needsManual: spec.needsManual,
      notes: spec.notes,
      ...(exception ? { exception: exception.reason } : {}),
      variants: mapped.variants.map((variant) => {
        collectStrokes(variant.tree)
        return {
          id: variant.figmaId,
          name: variant.figmaName,
          properties: propertiesById.get(variant.figmaId)!,
          key: viewKey(variant),
          state: variant.interaction,
          props: {
            ...variant.values,
            ...variant.booleans,
            ...variant.flags,
            ...(variant.interaction === 'base' ? {} : { forceState: variant.interaction })
          },
          width: variant.tree.w,
          height: variant.tree.h,
          texts: visibleTexts(variant.tree)
        }
      })
    })
  }
  return { components, strokeWidths: [...strokeWidths].sort((a, b) => a - b) }
}

async function writeReviewManifest(app: App, root?: string) {
  const source = app.createSource('snapshot')
  const { components, strokeWidths } = await reviewComponents(source, app.config)
  const [collections, textStyles, icons, syncState] = await Promise.all([
    source.getTokenCollections(),
    source.getTextStyles(),
    source.getIcons(),
    source.getSyncState?.()
  ])
  const index = new TokenIndex(collections)
  const tokens = collections.flatMap((collection) =>
    collection.variables.flatMap((token) =>
      collection.modes.map((mode) => ({
        collection: collection.name,
        mode,
        name: token.name,
        type: token.type,
        cssName: cssName(token.name),
        value: index.resolve(token, mode)
      }))
    )
  )
  const reviewRoot = root ? path.resolve(app.cwd, root) : path.resolve(app.resolveOutDir(), '../..')
  const directory = path.join(reviewRoot, 'src/review')
  const iconDir = path.join(reviewRoot, 'public/dcl-ui-review/icons')
  await mkdir(directory, { recursive: true })
  await mkdir(iconDir, { recursive: true })
  for (const icon of icons) await writeFile(path.join(iconDir, path.basename(icon.file)), icon.svg)
  const manifest = {
    components,
    pages: syncState?.pages ?? [],
    foundations: {
      tokens,
      textStyles: textStyles.map((style) => ({ ...style, className: textStyleClass(style.name) })),
      strokeWidths,
      icons: icons.map((entry) => {
        const icon = { ...entry, svg: undefined }
        return {
          ...icon,
          file: `/dcl-ui-review/icons/${encodeURIComponent(path.basename(icon.file))}`
        }
      }),
      effects: { ready: false, status: 'Disabled in Figma. Existing captured effects are frozen.' }
    }
  }
  await writeFile(path.join(directory, 'catalog.json'), JSON.stringify(manifest, null, 2) + '\n')
  return {
    components: components.length,
    variants: components.reduce((count, component) => count + component.variants.length, 0),
    tokens: tokens.length,
    textStyles: textStyles.length,
    icons: icons.length
  }
}

export { reviewComponents, writeReviewManifest }
