import { copyManagedTree } from '../guard/copy.js'
import { GENERATED } from '../guard/ownership.js'
import { LIBRARY_PROMOTION } from '../guard/library.js'
import { readIfExists, validateOutputPath } from '../guard/fs-safe.js'
import { catalogDiffFile, saveCatalogDiff } from './catalog-diff.js'
import { scopeErrorSummary, type MetadataChange } from '../guard/metadata-diagnostics.js'
import { sourceRetirements } from '../retirement-selection.js'
import { checkScope, isRetired, type Violation } from '../guard/index.js'
import { assertCompleteCapture } from './capture-completeness.js'
import {
  configuredPages,
  loadPageSnapshot,
  resolvePageIdentities,
  selectCapturePages,
  mergeSyncState,
  type ResolvedPage
} from '../page-identity.js'
import { addInventoryPages } from '../generation-inventory.js'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import type { App } from '../app.js'
import { createApp, runSync } from '../app.js'
import { redactFileKey, requireFileKey } from '../config.js'
import { FigmaClient, EnvTokenProvider } from '../figma/client.js'
import { readLiveFile, readLivePage, readReadiness, type LiveNode } from './live-file.js'
import {
  artworkIds,
  degenerateArtworkIds,
  capturePage,
  componentNodes,
  structuredImageRefs
} from './rest-capture.js'
import { exportImageFills, exportNodes } from './exports.js'
import { readSwatches, type SwatchChange } from './color-swatches.js'
import { validatePage } from '../capture/ingest-chunks.js'
import { stripImageMetadata, stripPngMetadata } from '../capture/strip-image-metadata.js'
import { writeReviewManifest } from '../review-manifest.js'
import { stripFileKey, stripFileKeyText } from '../capture/strip-file-key.js'
import { encodePageFileName } from '../capture/page-file-name.js'

function stable(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(stable).join(',') + ']'
  if (value && typeof value === 'object')
    return (
      '{' +
      Object.entries(value)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([key, item]) => JSON.stringify(key) + ':' + stable(item))
        .join(',') +
      '}'
    )
  return JSON.stringify(value) ?? 'null'
}
const hash = (value: unknown) => createHash('sha256').update(stable(value)).digest('hex')
const errorText = (error: unknown) =>
  redactFileKey(error instanceof Error ? error.message : String(error))
const json = async (file: string): Promise<unknown> => JSON.parse(await readFile(file, 'utf8'))
const save = async (file: string, data: unknown) => {
  await mkdir(path.dirname(file), { recursive: true })
  await writeFile(file, JSON.stringify(stripFileKey(data), null, 2) + '\n')
}
type Catalog = {
  components: {
    name: string
    page: string
    variants: { id: string; width: number; height: number }[]
  }[]
}
type Reference = { file: string; width: number; height: number; offsetX?: number; offsetY?: number }
type Baseline = {
  schema: number
  pages: Record<string, { hash: string; version: string }>
}

// Only actual removals are reported as blocked removals; every other violation is a scope conflict.
function scopeReport(violations: readonly Violation[]) {
  const describe = (violation: Violation) => `${violation.path}: ${violation.cause}`
  const metadataChanges = new Map<string, MetadataChange & { path: string }>()
  for (const violation of violations)
    for (const change of violation.metadataChanges ?? [])
      metadataChanges.set(JSON.stringify([violation.path, change.field]), {
        path: violation.path,
        ...change
      })
  return {
    removalsBlocked: violations.filter((violation) => violation.kind === 'removed').map(describe),
    scopeConflicts: violations
      .filter((violation) => violation.kind !== 'removed')
      .map((violation) => `${violation.kind}: ${describe(violation)}`),
    metadataChanges: [...metadataChanges.values()]
  }
}

async function pullLive(
  app: App,
  // `page` limits the refresh to one ready page so a single component page can be proposed and tested.
  options: { apply?: boolean; report: string; page?: string },
  client = new FigmaClient(new EnvTokenProvider())
) {
  if (!app.config.selection?.readyOnly)
    throw new Error('Live capture requires selection.readyOnly=true')
  // Read from the environment only. Reports, sync state and the baseline written below never
  // contain it: they are committed or uploaded as workflow artifacts.
  const fileKey = requireFileKey()
  await validateOutputPath(path.resolve(app.cwd, '..'), app.resolveOutDir(), GENERATED)
  const snapshot = path.resolve(app.cwd, app.config.snapshotDir)
  if (options.apply) await validateOutputPath(path.resolve(app.cwd, '..'), snapshot)
  const reviewRoot = path.resolve(app.resolveOutDir(), '../..')
  const reportFile = path.resolve(app.cwd, options.report)
  await rm(catalogDiffFile(reportFile), { force: true })
  const file = await readLiveFile(client, fileKey).catch(async (error: unknown) => {
    await save(reportFile, {
      apply: !!options.apply,
      changed: false,
      error: errorText(error)
    })
    throw error
  })
  const pages = readReadiness(file)
  const only = options.page?.trim()
  const source = app.createSource('snapshot')
  const previousState = await source.getSyncState?.()
  // Single-file by construction (FIGMA_FILE_KEY from the environment): saved state, inventory and
  // retirements carry no file key, so there is no file-identity check.
  const retirements = await sourceRetirements(source)
  const inventory = await source.getGenerationInventory?.()
  let configured: ResolvedPage[]
  let selected: ResolvedPage[]
  try {
    const activeConfig = {
      ...app.config,
      pages: Object.fromEntries(
        configuredPages(app.config)
          .filter((page) => !page.id || !isRetired(retirements, page.id))
          .map((page) => [page.key, app.config.pages[page.key]!])
      )
    }
    configured = resolvePageIdentities(activeConfig, pages)
    if (
      only &&
      !configured.some((page) => page.id === only) &&
      pages.filter((page) => page.name === only).length > 1
    )
      throw new Error(`Ambiguous Figma page: ${only}`)
    selected = selectCapturePages(configured, only)
  } catch (error) {
    await save(reportFile, {
      version: file.version,
      apply: !!options.apply,
      changed: false,
      error: errorText(error)
    })
    throw error
  }
  const capturedPageIds = selected
    .filter((page) => page.kind === 'components' || page.name === 'Colors')
    .map((page) => page.id!)
  const state = {
    pages: mergeSyncState(previousState?.pages ?? [], pages, capturedPageIds)
  }
  const report = {
    version: file.version,
    apply: !!options.apply,
    page: only,
    pages,
    foundationsCaptured: selected.some((page) => page.name === 'Colors'),
    capturedPages: selected
      .filter((page) => capturedPageIds.includes(page.id!))
      .map((page) => ({ id: page.id, name: page.liveName })),
    frozenPages: configured
      .filter((page) => !page.legacy && !capturedPageIds.includes(page.id!))
      .map((page) => ({ id: page.id, name: page.liveName })),
    renamedPages: configured
      .filter((page) => page.renamed)
      .map((page) => ({ id: page.id, name: page.liveName, previousName: page.name })),
    legacyEntries: configured
      .filter((page) => page.legacy)
      .map((page) => ({ id: null, name: page.name })),
    retiredPages: retirements.retired
      .filter((record) => !record.componentId)
      .map((record) => ({ id: record.pageId, name: record.name })),
    retiredNotCaptured: [] as { pageId: string; componentId: string }[],
    removalsBlocked: [] as string[],
    scopeConflicts: [] as string[],
    metadataChanges: [] as (MetadataChange & { path: string })[],
    removalsAuthorized: [] as string[],
    changedPages: [] as string[],
    changed: false,
    colorChanges: [] as SwatchChange[],
    colorNotes: [] as string[],
    notCaptured: [] as string[],
    frozenFoundations:
      'Colors are read from the swatch texts of a ready Colors page. Typography, gradients, icons and Effects retain the saved capture.',
    pendingReview: ['Progress Bar', 'All newly generated component previews'],
    // Set when Figma saved a newer version during capture; the output stays pinned to `version`.
    latestVersion: undefined as string | undefined,
    catalogDiff: undefined as string | undefined,
    catalogDiffError: undefined as string | undefined,
    error: undefined as string | undefined
  }
  const temp = await mkdtemp(path.join(os.tmpdir(), 'figma-live-'))
  let catalogBefore: string | undefined
  try {
    let baseline: Baseline = { schema: 1, pages: {} }
    try {
      const existing = (await json(path.join(snapshot, 'live-source.json'))) as Baseline
      if (existing.schema === 1) baseline = existing
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
    }
    for (const previous of previousState?.pages ?? []) {
      if (
        previous.ready === true &&
        configured.some((page) => page.id === previous.id && page.kind === 'components') &&
        pages.find((page) => page.id === previous.id)?.ready == null
      )
        throw new Error(`Previously ready page has no valid control: ${previous.name}`)
    }
    const captures = new Map<string, LiveNode>()
    const capturedComponents: Record<string, string[]> = {}
    for (const page of selected.filter((page) => page.kind === 'components')) {
      if (/[\\/]/.test(page.name) || page.name === '..')
        throw new Error(`Unsafe page name: ${page.id}`)
      const captured = await readLivePage(client, fileKey, page.id!, file.version)
      const retired = new Set(
        componentNodes(captured)
          .filter(({ node }) => isRetired(retirements, page.id!, node.id))
          .map(({ node }) => node.id)
      )
      report.retiredNotCaptured.push(
        ...[...retired].map((componentId) => ({ pageId: page.id!, componentId }))
      )
      const keep = (node: LiveNode): LiveNode => ({
        ...node,
        ...(node.children
          ? { children: node.children.filter((child) => !retired.has(child.id)).map(keep) }
          : {})
      })
      const live = keep(captured)
      const previous = await loadPageSnapshot(source, page)
      if (!previous && inventory?.review.some((entry) => entry.pageId === page.id))
        throw new Error(`Missing saved capture for inventory page: ${page.id}`)
      assertCompleteCapture(previous, live, (componentId) =>
        isRetired(retirements, page.id!, componentId)
      )
      capturedComponents[page.id!] = componentNodes(live).map(({ node }) => node.id)
      const digest = hash(componentNodes(live))
      if (baseline.pages[page.id!]?.hash !== digest) {
        captures.set(page.name, live)
        baseline.pages[page.id!] = { hash: digest, version: file.version }
        report.changedPages.push(page.name)
      }
    }
    let collections: Awaited<ReturnType<typeof readSwatches>>['collections'] | undefined
    for (const page of selected.filter((page) => page.kind !== 'components')) {
      if (page.name !== 'Colors') {
        report.notCaptured.push(page.name)
        continue
      }
      const live = await readLivePage(client, fileKey, page.id!, file.version)
      const result = readSwatches(live, await app.createSource('snapshot').getTokenCollections())
      if (result.errors.length)
        throw new Error(`Colors swatches need fixing in Figma:\n${result.errors.join('\n')}`)
      // Variable names come from live swatch labels: one naming the file or its key is rejected
      // before anything is staged, and only its redacted form is reported.
      const unsafe = result.collections
        .flatMap((collection) => collection.variables.map((variable) => variable.name))
        .filter((name) => stripFileKeyText(name) !== name)
      if (unsafe.length)
        throw new Error(
          `Colors swatch labels name the Figma file or its key: ${unsafe.map((name) => stripFileKeyText(name)).join(', ')}`
        )
      report.colorChanges = result.changes
      report.colorNotes = result.notes
      if (result.changes.length) {
        collections = result.collections
        report.changedPages.push(page.name)
      }
    }
    const nextInventory = inventory
      ? addInventoryPages(
          inventory,
          selected.filter((page) => page.kind === 'components').map((page) => page.id!),
          capturedComponents
        )
      : undefined
    report.changed =
      captures.size > 0 ||
      collections !== undefined ||
      hash(previousState) !== hash(state) ||
      (nextInventory !== undefined && hash(nextInventory) !== hash(inventory))
    if (!options.apply || !report.changed) return report
    const stagedCwd = path.join(temp, 'figma')
    const stagedSnapshot = path.join(stagedCwd, 'figma-snapshot')
    await copyManagedTree(snapshot, stagedSnapshot)
    if (!nextInventory) throw new Error('Generation inventory required to publish a capture')
    await save(path.join(stagedSnapshot, 'generation-inventory.json'), nextInventory)
    for (const [name, page] of captures) {
      const ids = componentNodes(page).flatMap(({ node }) => artworkIds(node))
      const strokes = new Set(
        componentNodes(page).flatMap(({ node }) => degenerateArtworkIds(node))
      )
      const artwork = await exportNodes(
        client,
        fileKey,
        file.version,
        ids.filter((id) => !strokes.has(id)),
        'svg'
      )
      const strokeExports = await exportNodes(
        client,
        fileKey,
        file.version,
        [...strokes],
        'svg',
        false
      )
      for (const [id, body] of strokeExports) artwork.set(id, body)
      const images = await exportImageFills(
        client,
        fileKey,
        componentNodes(page).flatMap(({ node }) => structuredImageRefs(node))
      )
      const capture = capturePage(
        page,
        new Map([
          ...[...artwork].map(([id, body]) => [id, body.toString('utf8')] as const),
          ...[...images].map(([ref, src]) => [`image:${ref}`, src] as const)
        ])
      )
      validatePage(capture)
      // Integrity is checked on the exported bytes; the stored capture drops non-rendering metadata.
      await save(
        path.join(stagedSnapshot, 'components', encodePageFileName(name)),
        stripImageMetadata(capture)
      )
    }
    if (collections)
      await writeFile(
        path.join(stagedSnapshot, 'variables.json'),
        JSON.stringify(stripFileKey({ collections })) + '\n'
      )
    await save(path.join(stagedSnapshot, 'sync-state.json'), state)
    await save(path.join(stagedSnapshot, 'live-source.json'), baseline)
    const stagedReview = path.join(temp, 'figma/review')
    catalogBefore = await readIfExists(path.join(reviewRoot, 'src/review/catalog.json'))
    await copyManagedTree(reviewRoot, stagedReview)
    // Review capture does not publish library output; retain its manifest for the general guard.
    await copyManagedTree(
      path.resolve(app.cwd, '..', LIBRARY_PROMOTION),
      path.join(temp, LIBRARY_PROMOTION)
    ).catch((error) => {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
    })
    const configPath = path.join(stagedCwd, 'figma.config.json')
    await copyManagedTree(app.configFile ?? path.join(app.cwd, 'figma.config.json'), configPath)
    const staged = await createApp(stagedCwd, configPath)
    staged.config.snapshotDir = stagedSnapshot
    staged.config.outDir = path.join(stagedReview, 'src/generated')
    if (staged.config.contract && app.config.contract)
      staged.config.contract.mappingRoot = path.resolve(app.cwd, app.config.contract.mappingRoot)
    await runSync(staged, {
      source: 'snapshot',
      capturedPageIds,
      ledgerOrphans: true,
      foundationsCaptured: selected.some((page) => page.name === 'Colors')
    })
    await writeReviewManifest(staged, stagedReview)
    const catalog = (await json(path.join(stagedReview, 'src/review/catalog.json'))) as Catalog
    const oldCatalog = (await json(path.join(reviewRoot, 'src/review/catalog.json'))) as Catalog
    const oldReferences = (await json(
      path.join(reviewRoot, 'src/review/references.json')
    )) as Record<string, Reference>
    const referencesById = new Map(
      oldCatalog.components.flatMap((component) =>
        component.variants.map(
          (variant, index) => [variant.id, oldReferences[`${component.name}-${index}`]] as const
        )
      )
    )
    const freshIds = catalog.components
      .filter((component) => captures.has(component.page))
      .flatMap((component) => component.variants.map((variant) => variant.id))
    const pngs = await exportNodes(client, fileKey, file.version, freshIds, 'png')
    const references: Record<string, Reference> = {}
    for (const component of catalog.components)
      for (const [index, variant] of component.variants.entries()) {
        const key = `${component.name}-${index}`
        const png = pngs.get(variant.id)
        if (png) {
          const filename = `live-${variant.id.replace(/[^a-zA-Z0-9_-]/g, '-')}.png`
          const target = path.join(stagedReview, 'public/dcl-ui-review/references', filename)
          await mkdir(path.dirname(target), { recursive: true })
          // Reference exports are imported rasters too: they keep only rendering and colour chunks.
          await writeFile(target, stripPngMetadata(png))
          references[key] = {
            file: `/dcl-ui-review/references/${filename}`,
            width: png.readUInt32BE(16),
            height: png.readUInt32BE(20),
            offsetX: 0,
            offsetY: 0
          }
        } else {
          const existing = referencesById.get(variant.id)
          if (!existing) throw new Error(`Missing reference for unchanged variant: ${variant.id}`)
          references[key] = existing
        }
      }
    await save(path.join(stagedReview, 'src/review/references.json'), references)
    // Every capture read above is pinned to file.version (page nodes, renders) or content-addressed (image
    // fills), so a newer file version cannot leak into the output. Only the readiness selection, which comes
    // from the unpinned whole-file read, must still hold.
    const latest = await readLiveFile(client, fileKey)
    if (hash(readReadiness(latest)) !== hash(pages))
      throw new Error('Figma readiness changed during capture. Retry the capture.')
    if (latest.version !== file.version) {
      report.latestVersion = latest.version
      process.stderr.write(
        `Figma advanced to version ${latest.version} during capture; output is pinned to ${file.version}.\n`
      )
    }
    const scope = await checkScope({
      baseDir: path.resolve(app.cwd, '..'),
      stagedDir: temp,
      capturedPageIds,
      runKind: 'automated',
      // The stage holds the snapshot and Review only; the library is not part of this publication.
      stage: 'partial',
      retirements,
      foundationsCaptured: selected.some((page) => page.name === 'Colors')
    })
    if (!scope.ok) {
      Object.assign(report, scopeReport(scope.violations))
      throw new Error(`Sync scope conflict: ${JSON.stringify(scopeErrorSummary(scope.violations))}`)
    }
    // Publish only the staged bytes that passed capture and scope validation.
    await copyManagedTree(stagedSnapshot, snapshot)
    await copyManagedTree(path.join(stagedReview, 'src/generated'), app.resolveOutDir())
    await copyManagedTree(
      path.join(stagedReview, 'src/review'),
      path.join(reviewRoot, 'src/review')
    )
    await copyManagedTree(path.join(stagedReview, 'public'), path.join(reviewRoot, 'public'))
    return report
  } catch (error) {
    report.error = errorText(error)
    try {
      report.catalogDiff = await saveCatalogDiff({
        before: catalogBefore,
        stagedFile: path.join(temp, 'figma/review/src/review/catalog.json'),
        reportFile
      })
    } catch (diagnosticError) {
      report.catalogDiffError = errorText(diagnosticError)
    }
    throw error
  } finally {
    await save(reportFile, report)
    await rm(temp, { recursive: true, force: true })
  }
}
export { pullLive, scopeReport, stable }
