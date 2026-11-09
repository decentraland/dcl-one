import { loadConfigFile } from '../config.js'
import { configuredPages } from '../page-identity.js'
import { reviewComponents } from '../review-manifest.js'
import { SnapshotSource } from '../sources/snapshot-source.js'
import { resolveContained } from './fs-safe.js'
import { CONFIG, SNAPSHOT } from './ownership.js'
import { RETIREMENTS_PATH } from './retirements.js'

// Non-entry catalog fields that are computed from component trees rather than read from a foundations
// page. Only these may change without a foundations capture, and only to their recomputed value.
const COMPONENT_DERIVED_CATALOG_FOUNDATIONS = ['strokeWidths'] as const
type DerivedCatalogField = (typeof COMPONENT_DERIVED_CATALOG_FOUNDATIONS)[number]
type DerivedCatalogMetadata = Record<DerivedCatalogField, unknown>

// Recomputes the derived fields over the full staged inventory: frozen snapshots and new captures alike.
async function recomputeDerivedCatalogMetadata(stagedDir: string): Promise<DerivedCatalogMetadata> {
  const config = await loadConfigFile(await resolveContained(stagedDir, CONFIG))
  const source = new SnapshotSource(
    await resolveContained(stagedDir, SNAPSHOT),
    configuredPages(config)
      .filter((page) => page.id === null)
      .map((page) => page.name),
    undefined,
    await resolveContained(stagedDir, RETIREMENTS_PATH)
  )
  const { strokeWidths } = await reviewComponents(source, config)
  return { strokeWidths }
}

const derivedFieldLabel = (field: DerivedCatalogField) => `foundations.${field}`

export { COMPONENT_DERIVED_CATALOG_FOUNDATIONS, derivedFieldLabel, recomputeDerivedCatalogMetadata }
export type { DerivedCatalogField, DerivedCatalogMetadata }
