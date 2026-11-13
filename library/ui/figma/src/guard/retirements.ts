import path from 'node:path'
import { z } from 'zod'
import { isPlainText } from '../../component-generation.mjs'
import { readIfExists } from './fs-safe.js'

// Retirement identities are permanent developer records, so later ready captures cannot readmit them.
// Keyless: the tool works on the one Figma file figma-design-system captures, so a record is identified by its page
// and component ids alone and never stores the file key (it is private). The strict schemas reject a
// record that carries one.
// The ledger is committed, so every string is allowlisted: node ids, plain-text names and reasons with no link,
// and repository-relative artifact paths. A Figma URL or a file key has no field it fits in.
const nodeId = z.string().regex(/^[\w:;-]{1,64}$/, 'must be a Figma node id')
const LINK = /:\/\/|figma\.com|www\./i
// The same plain-text rule as the accepted limits of the component registry.
const plainText = (max: number) =>
  z
    .string()
    .min(1)
    .max(max)
    .refine((text) => isPlainText(text, max), 'must be plain text with no link')
const artifact = z
  .string()
  .regex(
    /^[\w@ .()+,=%-]+(?:\/[\w@ .()+,=%-]+)*(?:#[\w:;=., ()/%-]+)?$/,
    'must be a repository-relative path'
  )
  .refine(
    (file) =>
      !file
        .split('#')[0]!
        .split('/')
        .some((part) => part === '..' || part === '.'),
    'must not leave the repository'
  )
  .refine((file) => !LINK.test(file), 'must not hold a link')

const retirementSchema = z.strictObject({
  pageId: nodeId,
  componentId: nodeId.optional(),
  name: plainText(200),
  reason: plainText(500),
  // Filled in and verified by `sync --developer --apply-retirements`.
  owned: z.array(artifact),
  // Shared artifacts remain until an explicit retirement of their last user authorizes removal.
  sharedExclusions: z.array(artifact)
})

const retirementsSchema = z
  .strictObject({
    schema: z.literal(1),
    retired: z.array(retirementSchema)
  })
  .superRefine((data, context) => {
    const identities = new Set<string>()
    for (const record of data.retired) {
      const key = retirementIdentity(record)
      if (identities.has(key))
        context.addIssue({ code: 'custom', message: `Duplicate retirement identity: ${key}` })
      identities.add(key)
      if (
        new Set(record.owned).size !== record.owned.length ||
        new Set(record.sharedExclusions).size !== record.sharedExclusions.length
      )
        context.addIssue({ code: 'custom', message: 'Duplicate retirement artifact' })
    }
  })

type Retirement = z.infer<typeof retirementSchema>
type Retirements = z.infer<typeof retirementsSchema>

function retirementIdentity(record: Retirement): string {
  return JSON.stringify([record.pageId, record.componentId ?? null])
}

// Committed, outside the gitignored capture cache: fetch-source overlays it onto the cache's retirements.json, so
// the record `retire` writes survives a re-fetch and lands in a commit.
const RETIREMENTS_PATH = 'figma/ledger/retirements.json'

const emptyRetirements = (): Retirements => ({ schema: 1, retired: [] })

async function loadRetirements(root: string): Promise<Retirements> {
  const text = await readIfExists(path.join(root, RETIREMENTS_PATH))
  if (text === undefined) return emptyRetirements()
  const parsed = retirementsSchema.parse(JSON.parse(text))
  return parsed
}

function serializeRetirements(records: Retirements): string {
  const parsed = retirementsSchema.parse(records)
  parsed.retired.sort((a, b) =>
    retirementIdentity(a) < retirementIdentity(b)
      ? -1
      : retirementIdentity(a) > retirementIdentity(b)
        ? 1
        : 0
  )
  for (const record of parsed.retired) {
    record.owned.sort()
    record.sharedExclusions.sort()
  }
  return JSON.stringify(parsed, null, 2) + '\n'
}

// A component is retired when its page is retired (a record without componentId) or the component is.
function isRetired(retirements: Retirements, pageId: string, componentId?: string): boolean {
  return retirements.retired.some(
    (entry) =>
      entry.pageId === pageId &&
      (entry.componentId === undefined ||
        (componentId !== undefined && entry.componentId === componentId))
  )
}

function filterRetired<T extends { pageId: string; componentId?: string }>(
  retirements: Retirements,
  items: T[]
): { kept: T[]; retired: T[] } {
  const kept: T[] = []
  const retired: T[] = []
  for (const item of items)
    (isRetired(retirements, item.pageId, item.componentId) ? retired : kept).push(item)
  return { kept, retired }
}

export {
  RETIREMENTS_PATH,
  emptyRetirements,
  filterRetired,
  isRetired,
  loadRetirements,
  serializeRetirements,
  retirementsSchema
}
export type { Retirement, Retirements }
