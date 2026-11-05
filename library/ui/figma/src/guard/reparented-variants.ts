import { artifactPresent } from './aggregate-edits.js'
import { isRetired, type Retirement, type Retirements } from './retirements.js'
import { SNAPSHOT, type Ownership, type Remap, type VariantParent } from './ownership.js'

const VARIANT = '#variant:'

// A designer can merge a retired component into a new set, so its ID lives on as a variant of that set.
// The variant then belongs to the set and is neither pending nor deletable, but only when exactly one
// component lists it and that component is a complete, native, non-retired set on the retired component's
// own page, in a snapshot whose pages all read. Anything else (no parent, two parents, a standalone
// component, a truncated or external set, a retired or foreign-page parent, an unreadable page) keeps the
// retirement's claim.
function reparentedTo(
  ownership: Ownership,
  retirements: Retirements,
  record: Retirement,
  variantId: string
): string | undefined {
  if (!record.componentId || ownership.conflicts.length) return undefined
  const parents = ownership.variantParents.get(variantId) ?? []
  if (parents.length !== 1) return undefined
  const parent = JSON.parse(parents[0]!) as VariantParent
  if (parent.kind !== 'set' || parent.truncated || parent.external) return undefined
  if (parent.pageId !== record.pageId || parent.componentId === record.componentId) return undefined
  if (isRetired(retirements, parent.pageId, parent.componentId)) return undefined
  return parent.componentId
}

const variantOf = (artifact: string) =>
  artifact.startsWith(`${SNAPSHOT}/components/`) && artifact.includes(VARIANT)
    ? artifact.slice(artifact.indexOf(VARIANT) + VARIANT.length)
    : undefined

// Whether a retirement's artifact is still there as the retired item. `parents` is the ownership of the
// tree whose bytes are read; `ownership` parses the aggregate.
async function retiredArtifactPresent(
  root: string,
  artifact: string,
  ownership: Ownership,
  record: Retirement,
  retirements: Retirements,
  parents: Ownership = ownership
): Promise<boolean> {
  const variant = variantOf(artifact)
  if (variant !== undefined && reparentedTo(parents, retirements, record, variant)) return false
  return artifactPresent(root, artifact, ownership)
}

// An ownership move the automated guard accepts: a retired component's variant, or its snapshot entry,
// now belonging to the one live set it was merged into.
function isReparentingRemap(remap: Remap, staged: Ownership, retirements: Retirements): boolean {
  const move = (variant: string | undefined, from: string, to: string) =>
    variant !== undefined &&
    retirements.retired.some(
      (record) =>
        record.componentId === from && reparentedTo(staged, retirements, record, variant) === to
    )
  if (remap.map === 'variantRoot') return move(remap.key, remap.from, remap.to)
  if (remap.map !== 'artifactIdentity') return false
  const from = JSON.parse(remap.from) as { pageId?: string; componentId?: string }
  const to = JSON.parse(remap.to) as { pageId?: string; componentId?: string }
  return (
    from.pageId === to.pageId &&
    from.componentId !== undefined &&
    to.componentId !== undefined &&
    move(variantOf(remap.key), from.componentId, to.componentId)
  )
}

export { isReparentingRemap, reparentedTo, retiredArtifactPresent }
