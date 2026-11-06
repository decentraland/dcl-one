import type { FigmaConfig } from '../config.js'
import { collisionReason } from '../mappers/component-spec.js'
import { loadComponentSpecs } from '../specs.js'
import type { FigmaSource } from '../sources/types.js'
import { generateComponents, type ManifestEntry } from './components.js'
import { generateIcons, type SkippedIcon } from './icons.js'
import { generateTokens } from './tokens.js'
import type { GeneratedFile, GeneratorContext } from './types.js'

type GenerateSummary = {
  source: FigmaSource['name']
  tokenCollections: number
  tokens: number
  textStyles: number
  gradients: number
  icons: number
  skippedIcons: SkippedIcon[]
  components: number
  componentViews: number
  needsManual: { component: string; page: string; reasons: string[] }[]
  skippedComponentPages: string[]
  files: number
}

type GenerateResult = { files: GeneratedFile[]; summary: GenerateSummary }

function needsManualOf(manifest: ManifestEntry[]): GenerateSummary['needsManual'] {
  return manifest
    .filter((entry) => entry.needsManual.length > 0)
    .map((entry) => ({ component: entry.name, page: entry.page, reasons: entry.needsManual }))
}

async function generateAll(
  source: FigmaSource,
  context: GeneratorContext,
  config?: FigmaConfig
): Promise<GenerateResult> {
  const [collections, textStyles, gradients, icons] = await Promise.all([
    source.getTokenCollections(),
    source.getTextStyles(),
    source.getGradients(),
    source.getIcons()
  ])

  const tokenFiles = generateTokens({ collections, textStyles, gradients }, context)
  const iconOutput = generateIcons(icons, context)

  const { specs, skippedPages, nameCollisions } = config
    ? await loadComponentSpecs(source, config, {
        collections,
        textStyles,
        icons: new Map(iconOutput.meta.map((meta) => [meta.name, meta.component]))
      })
    : { specs: [], skippedPages: [], nameCollisions: [] }
  const componentOutput = generateComponents(specs, context)

  const files = [...tokenFiles, ...iconOutput.files, ...(config ? componentOutput.files : [])].sort(
    (a, b) => a.path.localeCompare(b.path)
  )

  return {
    files,
    summary: {
      source: source.name,
      tokenCollections: collections.length,
      tokens: collections.reduce((total, collection) => total + collection.variables.length, 0),
      textStyles: textStyles.length,
      gradients: gradients.length,
      icons: iconOutput.meta.length,
      skippedIcons: iconOutput.skipped,
      components: specs.length,
      componentViews: specs.reduce((total, spec) => total + spec.views.length, 0),
      needsManual: [
        ...needsManualOf(componentOutput.manifest),
        // A colliding name generates nothing, so it reports here instead of in a manifest entry.
        ...nameCollisions.map((collision) => ({
          component: collision.name,
          page: [...new Set(collision.components.map((item) => item.page))].join(', '),
          reasons: [collisionReason(collision)]
        }))
      ],
      skippedComponentPages: skippedPages,
      files: files.length
    }
  }
}

export { generateAll }
export type { GenerateResult, GenerateSummary }
