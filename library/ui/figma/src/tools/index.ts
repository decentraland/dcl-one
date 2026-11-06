import { configuredPages } from '../page-identity.js'
import { atomicWrite, resolveContained, validateOutputPath } from '../guard/fs-safe.js'
import { mkdir } from 'node:fs/promises'
import path from 'node:path'
import { isWindowsSafeName } from '../capture/page-file-name.js'
import { z } from 'zod'
import { runDiff, runSync, type App } from '../app.js'
import { requireFileKey } from '../config.js'
import { EnvTokenProvider, FigmaClient } from '../figma/client.js'
import type { IconSpec } from '../ir/schemas.js'
import { collisionReason } from '../mappers/component-spec.js'
import { loadSpecsFromSource } from '../specs.js'

type ToolDefinition<Shape extends z.ZodRawShape = z.ZodRawShape> = {
  name: string
  description: string
  inputSchema: Shape
  handler: (input: z.infer<z.ZodObject<Shape>>) => Promise<unknown>
}

const sourceSchema = z
  .enum(['snapshot', 'rest'])
  .default('snapshot')
  .describe('Where to read the design from. "rest" needs FIGMA_TOKEN and is unverified.')

const MAX_INLINE_SVGS = 20

function tool<Shape extends z.ZodRawShape>(definition: ToolDefinition<Shape>): ToolDefinition {
  return definition as unknown as ToolDefinition
}

function matchesIcon(
  icon: IconSpec,
  filters: { category?: string; search?: string; names?: string[] }
) {
  if (filters.category && icon.category !== filters.category) return false
  if (filters.names && !filters.names.includes(icon.name)) return false
  if (filters.search && !icon.name.toLowerCase().includes(filters.search.toLowerCase()))
    return false
  return true
}

const insideWorkspace = (app: App, target: string) =>
  validateOutputPath(path.resolve(app.cwd, '..'), path.resolve(app.cwd, target))

function createTools(app: App): ToolDefinition[] {
  return [
    tool({
      name: 'figma_list_pages',
      description:
        'List the pages of the Figma design system file (tokens, inputs, components, cards, assets).',
      inputSchema: { source: sourceSchema },
      handler: async ({ source }) => {
        const pages = await app.createSource(source).listPages()
        const configured = configuredPages(app.config)
        return {
          pages: pages.map((page) => {
            const entry = configured.find((item) => item.id === page.id)
            return {
              ...page,
              configured: entry
                ? { kind: entry.kind, ...(entry.category ? { category: entry.category } : {}) }
                : null
            }
          })
        }
      }
    }),
    tool({
      name: 'figma_get_tokens',
      description:
        'Return design tokens: variable collections (Primitives, Semantic, Spacing, Layout with their modes), text styles and gradients.',
      inputSchema: {
        source: sourceSchema,
        collection: z
          .string()
          .optional()
          .describe('Only return this variable collection, e.g. "Semantic".'),
        summaryOnly: z
          .boolean()
          .default(false)
          .describe('Return counts and modes without the values.')
      },
      handler: async ({ source, collection, summaryOnly }) => {
        const figma = app.createSource(source)
        const [collections, textStyles, gradients] = await Promise.all([
          figma.getTokenCollections(),
          figma.getTextStyles(),
          figma.getGradients()
        ])
        const selected = collection ? collections.filter((c) => c.name === collection) : collections
        if (collection && selected.length === 0) {
          throw new Error(
            `Collection "${collection}" not found. Available: ${collections.map((c) => c.name).join(', ')}`
          )
        }
        return {
          counts: {
            variables: selected.reduce((total, c) => total + c.variables.length, 0),
            byCollection: Object.fromEntries(selected.map((c) => [c.name, c.variables.length])),
            textStyles: textStyles.length,
            gradients: gradients.length
          },
          collections: selected.map((c) => ({
            name: c.name,
            modes: c.modes,
            ...(summaryOnly ? {} : { variables: c.variables })
          })),
          ...(summaryOnly ? {} : { textStyles, gradients })
        }
      }
    }),
    tool({
      name: 'figma_list_components',
      description:
        'List design-system components: icons (grouped by name with their variants) and the generated UI components of the configured component pages, with their props, view count and needs-manual flags.',
      inputSchema: {
        source: sourceSchema,
        kind: z.enum(['all', 'icons', 'components']).default('all'),
        category: z
          .string()
          .optional()
          .describe('Icon category (e.g. "general", "navbar") or component page category.'),
        search: z.string().optional().describe('Case-insensitive substring of the component name.'),
        needsManualOnly: z
          .boolean()
          .default(false)
          .describe('Only list components that cannot be mapped confidently.')
      },
      handler: async ({ source, kind, category, search, needsManualOnly }) => {
        const figma = app.createSource(source)
        const entries: unknown[] = []

        if (kind !== 'components' && !needsManualOnly) {
          const icons = (await figma.getIcons()).filter((icon) =>
            matchesIcon(icon, { category, search })
          )
          const byName = new Map<
            string,
            {
              kind: 'icon'
              name: string
              category: string
              section: string
              variants: string[]
              size: string
            }
          >()
          for (const icon of icons) {
            const entry = byName.get(icon.name) ?? {
              kind: 'icon' as const,
              name: icon.name,
              category: icon.category,
              section: icon.section,
              variants: [],
              size: `${icon.width}x${icon.height}`
            }
            if (icon.variant) entry.variants.push(icon.variant)
            byName.set(icon.name, entry)
          }
          entries.push(...byName.values())
        }

        if (kind !== 'icons') {
          const { specs, skippedPages, nameCollisions } = await loadSpecsFromSource(
            figma,
            app.config
          )
          const needle = search?.toLowerCase()
          for (const spec of specs) {
            if (category && spec.category !== category && spec.page !== category) continue
            if (needle && !`${spec.name} ${spec.figmaName}`.toLowerCase().includes(needle)) continue
            if (needsManualOnly && spec.needsManual.length === 0) continue
            entries.push({
              kind: 'component',
              name: spec.name,
              figmaName: spec.figmaName,
              page: spec.page,
              category: spec.category,
              props: spec.props.map((prop) => prop.name),
              views: spec.views.length,
              needsManual: spec.needsManual
            })
          }
          // Colliding names generate nothing; they are listed as needs-manual entries of their own.
          for (const collision of nameCollisions) {
            const needle = search?.toLowerCase()
            if (needle && !collision.name.toLowerCase().includes(needle)) continue
            entries.push({
              kind: 'component',
              name: collision.name,
              figmaName: collision.components.map((item) => item.figmaName).join(', '),
              page: [...new Set(collision.components.map((item) => item.page))].join(', '),
              needsManual: [collisionReason(collision)]
            })
          }
          if (skippedPages.length > 0 && kind === 'components') {
            return { count: entries.length, components: entries, skippedPages }
          }
        }
        return { count: entries.length, components: entries }
      }
    }),
    tool({
      name: 'figma_get_component_spec',
      description:
        'Return the normalised spec of one generated component: props, slots, default view, needs-manual reasons and inferred-rule notes. Pass includeViews to also get every view with its resolved CSS.',
      inputSchema: {
        source: sourceSchema,
        name: z
          .string()
          .describe('Generated name (e.g. "Button") or Figma name (e.g. "Button-Contained-Text").'),
        includeViews: z.boolean().default(false)
      },
      handler: async ({ source, name, includeViews }) => {
        const { specs } = await loadSpecsFromSource(app.createSource(source), app.config)
        const spec = specs.find((entry) => entry.name === name || entry.figmaName === name)
        if (!spec) {
          throw new Error(
            `Component "${name}" not found. Available: ${specs.map((entry) => entry.name).join(', ')}`
          )
        }
        const { views, ...summary } = spec
        return includeViews
          ? { ...summary, views }
          : {
              ...summary,
              views: views.map((view) => ({ key: view.key, figmaName: view.figmaName }))
            }
      }
    }),
    tool({
      name: 'figma_export_icons',
      description: `Export icon SVGs. Without outDir returns the markup of up to ${MAX_INLINE_SVGS} icons; with outDir (inside the workspace) writes every match as a .svg file.`,
      inputSchema: {
        source: sourceSchema,
        names: z.array(z.string()).optional().describe('Exact icon names, e.g. ["general/like"].'),
        category: z.string().optional(),
        outDir: z.string().optional()
      },
      handler: async ({ source, names, category, outDir }) => {
        const icons = (await app.createSource(source).getIcons()).filter((icon) =>
          matchesIcon(icon, { names, category })
        )
        if (outDir) {
          const target = await insideWorkspace(app, outDir)
          await mkdir(target, { recursive: true })
          for (const icon of icons) {
            if (
              path.basename(icon.file) !== icon.file ||
              !isWindowsSafeName(icon.file) ||
              !icon.file.endsWith('.svg')
            )
              throw new Error(`Invalid icon filename: ${JSON.stringify(icon.file)}`)
            await resolveContained(target, icon.file)
          }
          await Promise.all(
            icons.map((icon) => atomicWrite(path.join(target, icon.file), icon.svg))
          )
          return { written: icons.length, outDir: target, files: icons.map((icon) => icon.file) }
        }
        if (icons.length > MAX_INLINE_SVGS) {
          throw new Error(
            `${icons.length} icons match. Narrow the filter or pass outDir to write them to disk.`
          )
        }
        return {
          count: icons.length,
          icons: icons.map(({ name, variant, svg }) => ({ name, variant, svg }))
        }
      }
    }),
    tool({
      name: 'figma_sync',
      description:
        'Generate tokens, icons and component previews from saved Figma captures into the Review generated folder. Adds and updates only: it never deletes, and it refuses a run that would change output it does not own.',
      inputSchema: { source: sourceSchema, outDir: z.string().optional() },
      handler: async ({ source, outDir }) =>
        runSync(app, { source, outDir: outDir ? await insideWorkspace(app, outDir) : undefined })
    }),
    tool({
      name: 'figma_diff',
      description: 'Show what figma_sync would add, change or remove, without writing anything.',
      inputSchema: { source: sourceSchema, outDir: z.string().optional() },
      handler: async ({ source, outDir }) =>
        runDiff(app, { source, outDir: outDir ? await insideWorkspace(app, outDir) : undefined })
    }),
    tool({
      name: 'figma_screenshot',
      description:
        'Render a node as PNG through the Figma REST API and return its short-lived URL. Needs FIGMA_TOKEN; unverified.',
      inputSchema: {
        nodeId: z
          .string()
          .regex(/^\d+[:-]\d+$/)
          .describe('Node id such as "1:5239".'),
        scale: z.number().min(0.01).max(4).default(1)
      },
      handler: async ({ nodeId, scale }) => {
        const client = new FigmaClient(new EnvTokenProvider())
        const id = nodeId.replace('-', ':')
        const response = await client.get<{ images: Record<string, string | null> }>(
          `/v1/images/${requireFileKey()}?ids=${encodeURIComponent(id)}&format=png&scale=${scale}`
        )
        const url = response.images[id]
        if (!url) throw new Error(`Figma returned no image for node ${id}`)
        return { nodeId: id, url }
      }
    })
  ]
}

export { createTools, tool }
export type { ToolDefinition }
