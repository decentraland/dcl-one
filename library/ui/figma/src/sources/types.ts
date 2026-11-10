import type { Retirements } from '../guard/retirements.js'
import type { GenerationInventory } from '../generation-inventory.js'
import type { RawComponentPage } from '../ir/raw-components.js'
import type {
  FigmaPage,
  GradientStyle,
  IconSpec,
  TextStyle,
  TokenCollection
} from '../ir/schemas.js'

interface FigmaSource {
  readonly name: 'snapshot' | 'rest'
  // Verifies the capture before any read; a source with nothing to verify omits it.
  ready?(): Promise<void>
  listPages(): Promise<FigmaPage[]>
  getTokenCollections(): Promise<TokenCollection[]>
  getTextStyles(): Promise<TextStyle[]>
  getGradients(): Promise<GradientStyle[]>
  getIcons(): Promise<IconSpec[]>
  getComponentPage(pageName: string): Promise<RawComponentPage | undefined>
  getRetirements?(): Promise<Retirements>
  getGenerationInventory?(): Promise<GenerationInventory | undefined>
  getComponentPageById?(
    pageId: string,
    snapshotName?: string
  ): Promise<RawComponentPage | undefined>
  getSyncState?(): Promise<
    { pages: { id: string; name: string; ready: boolean | null }[] } | undefined
  >
}

export type { FigmaSource }
