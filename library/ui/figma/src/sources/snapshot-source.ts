import { readIfExists } from '../guard/fs-safe.js'
import { emptyRetirements, retirementsSchema } from '../guard/retirements.js'
import { generationInventorySchema } from '../generation-inventory.js'
import { readdir } from 'node:fs/promises'
import path from 'node:path'
import { z } from 'zod'
import { rawComponentPageSchema, type RawComponentPage } from '../ir/raw-components.js'
import {
  gradientStyleSchema,
  iconRecordSchema,
  pageSchema,
  textStyleSchema,
  tokenCollectionSchema,
  type FigmaPage,
  type GradientStyle,
  type IconSpec,
  type TextStyle,
  type TokenCollection
} from '../ir/schemas.js'
import type { FigmaSource } from './types.js'
import { readMaterialized } from '../lfs.js'

const readText = async (file: string) => (await readMaterialized(file)).toString('utf8')

const variablesFile = z.object({ collections: z.array(tokenCollectionSchema) })
const textStylesFile = z.object({ styles: z.array(textStyleSchema) })
const paintStylesFile = z.object({ gradients: z.array(gradientStyleSchema) })
const pagesFile = z.object({ pages: z.array(pageSchema) })
const iconsFile = z.object({ icons: z.array(iconRecordSchema) })

class SnapshotSource implements FigmaSource {
  readonly name = 'snapshot' as const

  constructor(
    private readonly dir: string,
    private readonly legacySnapshotNames: readonly string[] = []
  ) {}

  private async readJson<S extends z.ZodType>(file: string, schema: S): Promise<z.infer<S>> {
    const raw = await readText(path.join(this.dir, file))
    return schema.parse(JSON.parse(raw))
  }

  async listPages(): Promise<FigmaPage[]> {
    return (await this.readJson('pages.json', pagesFile)).pages
  }

  async getSyncState() {
    const raw = await readText(path.join(this.dir, 'sync-state.json')).catch(
      (error: NodeJS.ErrnoException) => {
        if (error.code === 'ENOENT') return undefined
        throw error
      }
    )
    return raw === undefined
      ? undefined
      : z
          .object({
            pages: z.array(
              z.object({ id: z.string(), name: z.string(), ready: z.boolean().nullable() })
            )
          })
          .parse(JSON.parse(raw))
  }

  async getRetirements() {
    const raw = await readIfExists(path.join(this.dir, 'retirements.json'))
    return raw === undefined ? emptyRetirements() : retirementsSchema.parse(JSON.parse(raw))
  }

  async getGenerationInventory() {
    const raw = await readIfExists(path.join(this.dir, 'generation-inventory.json'))
    return raw === undefined ? undefined : generationInventorySchema.parse(JSON.parse(raw))
  }

  async getComponentPageById(
    pageId: string,
    snapshotName?: string
  ): Promise<RawComponentPage | undefined> {
    if (snapshotName) {
      const canonical = await this.getComponentPage(snapshotName)
      if (canonical) {
        if (canonical.page.id !== pageId)
          throw new Error(`Snapshot page identity changed: ${snapshotName}`)
        return canonical
      }
    }
    const names = await readdir(path.join(this.dir, 'components')).catch(
      (error: NodeJS.ErrnoException) => {
        if (error.code === 'ENOENT') return []
        throw error
      }
    )
    const matches: RawComponentPage[] = []
    const excluded = new Set(this.legacySnapshotNames.map((name) => `${name}.json`))
    for (const name of names.filter((name) => name.endsWith('.json') && !excluded.has(name))) {
      const data = await this.readJson(path.join('components', name), rawComponentPageSchema)
      if (data.page.id === pageId) matches.push(data)
    }
    if (matches.length > 1) throw new Error(`Ambiguous snapshot page ID: ${pageId}`)
    return matches[0]
  }

  async getTokenCollections(): Promise<TokenCollection[]> {
    return (await this.readJson('variables.json', variablesFile)).collections
  }

  async getTextStyles(): Promise<TextStyle[]> {
    return (await this.readJson('text-styles.json', textStylesFile)).styles
  }

  async getGradients(): Promise<GradientStyle[]> {
    return (await this.readJson('paint-styles.json', paintStylesFile)).gradients
  }

  async getComponentPage(pageName: string): Promise<RawComponentPage | undefined> {
    const file = path.join('components', `${pageName}.json`)
    const raw = await readText(path.join(this.dir, file)).catch((error: NodeJS.ErrnoException) => {
      if (error.code === 'ENOENT') return undefined
      throw error
    })
    return raw === undefined ? undefined : rawComponentPageSchema.parse(JSON.parse(raw))
  }

  async getIcons(): Promise<IconSpec[]> {
    const { icons } = await this.readJson('icons.json', iconsFile)
    return Promise.all(
      icons.map(async (icon) => ({
        ...icon,
        svg: await readText(path.join(this.dir, 'icons', icon.file))
      }))
    )
  }
}

export { SnapshotSource }
