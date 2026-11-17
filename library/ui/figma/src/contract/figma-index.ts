import { configuredPages, loadPageSnapshot } from '../page-identity.js'
import type { FigmaConfig } from '../config.js'
import type { FigmaSource } from '../sources/types.js'

type FigmaVariant = { id: string; name: string; props: Record<string, string> }

type FigmaProperty = { type: string; default?: string | boolean; options?: string[] }

type FigmaSet = {
  setId: string
  name: string
  page: string
  kind: 'set' | 'component'
  properties: Record<string, FigmaProperty>
  variants: FigmaVariant[]
}

async function buildFigmaIndex(source: FigmaSource, config: FigmaConfig): Promise<FigmaSet[]> {
  const sets: FigmaSet[] = []
  for (const page of configuredPages(config)) {
    if (page.kind !== 'components') continue
    const data = await loadPageSnapshot(source, page)
    if (!data) continue
    for (const component of data.components) {
      const variants = [component.base, ...component.others].map((variant) => ({
        id: variant.id,
        name: variant.name,
        props: variant.props
      }))
      sets.push({
        setId: component.id,
        name: component.name,
        page: page.name,
        kind: component.kind,
        properties: component.properties as Record<string, FigmaProperty>,
        variants
      })
    }
  }
  return sets
}

const normalizeNodeId = (id: string) => id.trim().replace('-', ':')

export { buildFigmaIndex, normalizeNodeId }
export type { FigmaSet, FigmaVariant }
