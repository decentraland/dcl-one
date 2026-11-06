import { isAtomicTemporary } from '../guard/fs-safe.js'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import type { App } from '../app.js'
import { loadSpecsFromSource } from '../specs.js'
import type { ComponentSpec } from '../ir/components.js'
import { readMaterialized } from '../lfs.js'

const hash = (content: Buffer | string) => createHash('sha256').update(content).digest('hex')
const runtimeFiles = [
  'figma-module-runtime.tsx',
  'figma-module-runtime.module.css',
  'figma-view-state.ts',
  'figma-interactions.tsx'
]
// Paths are relative to library/ui. The manifest never records the Figma file key.
const MANIFEST = 'figma/review/src/review/promotion.json'
const REVIEW = 'figma/review'
type PromotionManifest = {
  schema: 1
  files: Record<string, string>
  components: ComponentSpec[]
  excluded: { name: string; reasons: string[] }[]
}
async function filesIn(root: string, relative: string): Promise<string[]> {
  const entries = await readdir(path.join(root, relative), { withFileTypes: true })
  return (
    await Promise.all(
      entries
        .filter((entry) => !isAtomicTemporary(entry.name))
        .map((entry) =>
          entry.isDirectory()
            ? filesIn(root, `${relative}/${entry.name}`)
            : [`${relative}/${entry.name}`]
        )
    )
  ).flat()
}
// Promotion skips components with unresolved needs-manual items and the Progress Bar pending visual review.
const promotionExclusion = (spec: ComponentSpec): string[] =>
  spec.page === 'Progress Bar' ? ['Pending visual review'] : spec.needsManual

async function createPromotionManifest(app: App): Promise<PromotionManifest> {
  const root = path.resolve(app.cwd, '..')
  const source = app.createSource('snapshot')
  const { specs } = await loadSpecsFromSource(source, app.config, 'promotion')
  const excluded = specs
    .map((spec) => ({ name: spec.name, reasons: promotionExclusion(spec) }))
    .filter((item) => item.reasons.length)
  const components = specs.filter((spec) => !excluded.some((item) => item.name === spec.name))
  const inputPaths = [
    ...(await filesIn(root, `${REVIEW}/src/generated`)),
    ...(await filesIn(root, 'figma/figma-snapshot')),
    ...runtimeFiles.map((file) => `${REVIEW}/src/components/${file}`),
    `${REVIEW}/src/review/catalog.json`,
    `${REVIEW}/src/review/references.json`,
    ...(await filesIn(root, `${REVIEW}/public/dcl-ui-review`))
  ].sort()
  const files: Record<string, string> = {}
  for (const file of inputPaths) files[file] = hash(await readMaterialized(path.join(root, file)))
  return { schema: 1, files, components, excluded }
}
// The committed manifest seals each reviewed spec by hash instead of embedding it: specs carry their
// inline artwork and image fills, which made the file too large for GitHub's blob API. Promotion
// rebuilds the specs from the reviewed capture and must reproduce every hash.
type SealedManifest = Omit<PromotionManifest, 'schema' | 'components'> & {
  schema: 2
  components: { name: string; hash: string }[]
}

const STALE = 'Reviewed source changed. Run a new preview sync and review its PR before promotion.'
const RESEAL = 'Reseal it after review with `pnpm figma promotion-manifest`.'
const SHA256 = /^[0-9a-f]{64}$/

function seal(manifest: PromotionManifest): SealedManifest {
  return {
    schema: 2,
    files: manifest.files,
    components: manifest.components.map((spec) => ({
      name: spec.name,
      hash: hash(JSON.stringify(spec))
    })),
    excluded: manifest.excluded
  }
}

// Every field is checked before use, so a malformed, hand-edited or older manifest fails with a message
// naming the field instead of passing or crashing.
function parseSealed(text: string): SealedManifest {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    throw new Error(`Promotion manifest is not valid JSON. ${RESEAL}`)
  }
  const manifest = value as Record<string, unknown>
  if (!manifest || typeof manifest !== 'object' || Array.isArray(manifest))
    throw new Error(`Promotion manifest is not an object. ${RESEAL}`)
  if (manifest.schema === 1)
    throw new Error(
      `${STALE} The sealed promotion manifest uses the legacy schema 1 (embedded specs); this tooling verifies schema 2 (specs sealed by hash). ${RESEAL}`
    )
  if (manifest.schema !== 2)
    throw new Error(
      `Promotion manifest field schema is ${JSON.stringify(manifest.schema)}; expected 2. ${RESEAL}`
    )
  const files = manifest.files
  if (!files || typeof files !== 'object' || Array.isArray(files))
    throw new Error(`Promotion manifest field files is missing or not an object. ${RESEAL}`)
  for (const [file, digest] of Object.entries(files))
    if (typeof digest !== 'string' || !SHA256.test(digest))
      throw new Error(
        `Promotion manifest field files[${JSON.stringify(file)}] is not a sha256 hex digest. ${RESEAL}`
      )
  if (!Array.isArray(manifest.components))
    throw new Error(`Promotion manifest field components is missing or not an array. ${RESEAL}`)
  manifest.components.forEach((entry, index) => {
    const component = entry as Record<string, unknown>
    if (!component || typeof component !== 'object' || Array.isArray(component))
      throw new Error(`Promotion manifest field components[${index}] is not an object. ${RESEAL}`)
    if (typeof component.name !== 'string' || !component.name)
      throw new Error(`Promotion manifest field components[${index}].name is missing. ${RESEAL}`)
    if (typeof component.hash !== 'string' || !SHA256.test(component.hash))
      throw new Error(
        `Promotion manifest field components[${index}].hash (${component.name}) is not a sha256 hex digest. ${RESEAL}`
      )
  })
  if (!Array.isArray(manifest.excluded))
    throw new Error(`Promotion manifest field excluded is missing or not an array. ${RESEAL}`)
  manifest.excluded.forEach((entry, index) => {
    const excluded = entry as Record<string, unknown>
    if (!excluded || typeof excluded !== 'object' || Array.isArray(excluded))
      throw new Error(`Promotion manifest field excluded[${index}] is not an object. ${RESEAL}`)
    if (typeof excluded.name !== 'string' || !excluded.name)
      throw new Error(`Promotion manifest field excluded[${index}].name is missing. ${RESEAL}`)
    if (
      !Array.isArray(excluded.reasons) ||
      excluded.reasons.some((reason) => typeof reason !== 'string')
    )
      throw new Error(
        `Promotion manifest field excluded[${index}].reasons (${excluded.name}) is not a list of strings. ${RESEAL}`
      )
  })
  return manifest as unknown as SealedManifest
}

// Names what no longer matches, so the reviewer knows which file or component to look at.
function staleness(approved: SealedManifest, current: SealedManifest): string | undefined {
  const files = new Set([...Object.keys(approved.files), ...Object.keys(current.files)])
  for (const file of [...files].sort())
    if (approved.files[file] !== current.files[file])
      return `${file} ${current.files[file] === undefined ? 'is gone' : approved.files[file] === undefined ? 'is new' : 'changed'} since review`
  const sealed = new Map(approved.components.map((item) => [item.name, item.hash]))
  const rebuilt = new Map(current.components.map((item) => [item.name, item.hash]))
  for (const name of [...new Set([...sealed.keys(), ...rebuilt.keys()])].sort()) {
    if (!rebuilt.has(name)) return `component ${name} is no longer eligible`
    if (!sealed.has(name)) return `component ${name} was not reviewed`
    if (sealed.get(name) !== rebuilt.get(name))
      return `component ${name} no longer matches its sealed spec hash`
  }
  if (
    JSON.stringify(approved.components.map((item) => item.name)) !==
    JSON.stringify(current.components.map((item) => item.name))
  )
    return 'component order changed since review'
  const reasons = (manifest: SealedManifest) =>
    new Map(manifest.excluded.map((item) => [item.name, JSON.stringify(item.reasons)]))
  const excludedBefore = reasons(approved)
  const excludedNow = reasons(current)
  for (const name of [...new Set([...excludedBefore.keys(), ...excludedNow.keys()])].sort())
    if (excludedBefore.get(name) !== excludedNow.get(name))
      return `excluded component ${name} changed its needs-manual reasons`
  if (JSON.stringify(approved.excluded) !== JSON.stringify(current.excluded))
    return 'the excluded components changed order since review'
  return undefined
}

async function writePromotionManifest(app: App) {
  const manifest = await createPromotionManifest(app)
  const file = path.resolve(app.cwd, '..', MANIFEST)
  await mkdir(path.dirname(file), { recursive: true })
  await writeFile(file, JSON.stringify(seal(manifest), null, 2) + '\n')
  return {
    file,
    components: manifest.components.map((item) => item.name),
    excluded: manifest.excluded
  }
}
// Every sealed entry that the tree no longer reproduces: inputs, component specs or exclusions.
function sealDifferences(approved: SealedManifest, current: SealedManifest): string[] {
  const differences: string[] = []
  // The seal carries no file key (it is private), so only the schema is compared in the header.
  if (approved.schema !== current.schema) differences.push('header')
  for (const file of new Set([...Object.keys(approved.files), ...Object.keys(current.files)]))
    if (approved.files[file] !== current.files[file]) differences.push(`file:${file}`)
  const hashes = (manifest: SealedManifest) =>
    new Map(manifest.components.map((item) => [item.name, item.hash]))
  const [before, after] = [hashes(approved), hashes(current)]
  for (const name of new Set([...before.keys(), ...after.keys()]))
    if (before.get(name) !== after.get(name)) differences.push(`component:${name}`)
  if (JSON.stringify(approved.excluded) !== JSON.stringify(current.excluded))
    differences.push('excluded')
  if (!differences.length && JSON.stringify(approved) !== JSON.stringify(current))
    differences.push('order')
  return differences
}

// The committed seal must equal a reseal of the same tree; a missing manifest has nothing to verify.
async function staleSeal(app: App): Promise<string[]> {
  const root = path.resolve(app.cwd, '..')
  let text: string
  try {
    text = await readFile(path.join(root, MANIFEST), 'utf8')
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return []
    throw error
  }
  return sealDifferences(
    JSON.parse(text) as SealedManifest,
    seal(await createPromotionManifest(app))
  )
}

async function verifyPromotionManifest(app: App): Promise<PromotionManifest> {
  const root = path.resolve(app.cwd, '..')
  const approved = parseSealed((await readMaterialized(path.join(root, MANIFEST))).toString('utf8'))
  const current = await createPromotionManifest(app)
  const changed = staleness(approved, seal(current))
  if (changed) throw new Error(`${STALE} Stale: ${changed}.`)
  return current
}
export {
  MANIFEST as PROMOTION_MANIFEST,
  createPromotionManifest,
  staleSeal,
  promotionExclusion,
  writePromotionManifest,
  verifyPromotionManifest,
  runtimeFiles
}
export type { PromotionManifest, SealedManifest }
