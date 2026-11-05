import type { FigmaClient } from '../figma/client.js'
import type { RawComponentPage } from '../ir/raw-components.js'
import type {
  FigmaPage,
  GradientStyle,
  IconSpec,
  TextStyle,
  TokenCollection
} from '../ir/schemas.js'
import {
  collectIconComponents,
  mapRestGradient,
  mapRestTextStyle,
  mapRestVariables,
  type RestNode,
  type RestVariablesMeta
} from './rest-mappers.js'
import type { FigmaSource } from './types.js'

const IMAGE_BATCH = 100

type StyleMeta = { name: string; styleType: 'FILL' | 'TEXT' | 'EFFECT' | 'GRID' }

/**
 * Unverified: written against the documented Figma REST API but never run, because no
 * FIGMA_TOKEN was available. Use SnapshotSource until `dcl-figma probe` reports ok.
 */
class RestSource implements FigmaSource {
  readonly name = 'rest' as const

  constructor(
    private readonly client: FigmaClient,
    private readonly fileKey: string,
    private readonly iconsPage = 'Icons'
  ) {}

  async listPages(): Promise<FigmaPage[]> {
    const file = await this.client.get<{ document: { children: RestNode[] } }>(
      `/v1/files/${this.fileKey}?depth=1`
    )
    return file.document.children.map((page) => ({ id: page.id, name: page.name.trim() }))
  }

  async getTokenCollections(): Promise<TokenCollection[]> {
    const response = await this.client.get<{ meta: RestVariablesMeta }>(
      `/v1/files/${this.fileKey}/variables/local`
    )
    return mapRestVariables(response.meta)
  }

  private async getStyles(styleType: StyleMeta['styleType']) {
    const file = await this.client.get<{ styles: Record<string, StyleMeta> }>(
      `/v1/files/${this.fileKey}?depth=1`
    )
    return Object.entries(file.styles)
      .filter(([, style]) => style.styleType === styleType)
      .map(([nodeId, style]) => ({ nodeId, name: style.name }))
  }

  private async getStyleNodes(ids: string[]) {
    const response = await this.client.get<{
      nodes: Record<string, { document: Record<string, unknown> }>
    }>(`/v1/files/${this.fileKey}/nodes?ids=${ids.map(encodeURIComponent).join(',')}`)
    return response.nodes
  }

  async getTextStyles(): Promise<TextStyle[]> {
    const styles = await this.getStyles('TEXT')
    const nodes = await this.getStyleNodes(styles.map((style) => style.nodeId))
    return styles.flatMap(({ nodeId, name }) => {
      const typeStyle = nodes[nodeId]?.document.style as
        Parameters<typeof mapRestTextStyle>[1] | undefined
      return typeStyle ? [mapRestTextStyle(name, typeStyle)] : []
    })
  }

  async getGradients(): Promise<GradientStyle[]> {
    const styles = await this.getStyles('FILL')
    const nodes = await this.getStyleNodes(styles.map((style) => style.nodeId))
    return styles.flatMap(({ nodeId, name }) => {
      const fills = nodes[nodeId]?.document.fills as
        Parameters<typeof mapRestGradient>[1][] | undefined
      const gradient = fills?.[0] && mapRestGradient(name, fills[0])
      return gradient ? [gradient] : []
    })
  }

  // Component pages are captured with the official Figma MCP (capture/extract-components.figma.js);
  // there is no REST capture yet, so callers see "no data" and report the page as skipped.
  async getComponentPage(): Promise<RawComponentPage | undefined> {
    return undefined
  }

  async getIcons(): Promise<IconSpec[]> {
    const pages = await this.listPages()
    const page = pages.find((candidate) => candidate.name === this.iconsPage)
    if (!page) throw new Error(`Page "${this.iconsPage}" not found`)

    const response = await this.client.get<{ nodes: Record<string, { document: RestNode }> }>(
      `/v1/files/${this.fileKey}/nodes?ids=${encodeURIComponent(page.id)}`
    )
    const sections = (response.nodes[page.id]?.document.children ?? []).filter(
      (node) => node.type === 'FRAME' && node.visible !== false
    )
    const components = sections.flatMap((section) =>
      collectIconComponents(section).map((component) => ({ section: section.name, ...component }))
    )

    const urls: Record<string, string> = {}
    for (let start = 0; start < components.length; start += IMAGE_BATCH) {
      const ids = components.slice(start, start + IMAGE_BATCH).map((component) => component.nodeId)
      const images = await this.client.get<{ images: Record<string, string | null> }>(
        `/v1/images/${this.fileKey}?ids=${ids.map(encodeURIComponent).join(',')}&format=svg`
      )
      for (const [id, url] of Object.entries(images.images)) if (url) urls[id] = url
    }

    return Promise.all(
      components.map(async ({ section, path, ...component }) => {
        const url = urls[component.nodeId]
        if (!url) throw new Error(`No SVG export URL for ${path}`)
        return {
          ...component,
          section,
          category: path.split('/')[0]!,
          group: component.variant
            ? component.name
            : path.split('/').slice(0, -1).join('/') || path,
          file: `${path.replace(/\//g, '__').replace(/=/g, '-')}.svg`,
          svg: await (await fetch(url)).text()
        }
      })
    )
  }
}

export { RestSource }
