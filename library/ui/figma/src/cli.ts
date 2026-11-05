#!/usr/bin/env node
import { componentOwnerById } from './guard/component-identity.js'
import { existsSync } from 'node:fs'
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { main as keyGuard } from '../key-guard.mjs'
import {
  PROMOTION_MANIFEST,
  staleSeal,
  verifyPromotionManifest,
  writePromotionManifest
} from './promotion/manifest.js'
import { promoteReviewed } from './promotion/promote.js'
import { pullLive } from './automation/pull-live.js'
import { parseArgs } from 'node:util'
import { createApp, runDiff, runSync, type SourceKind } from './app.js'
import { redactFileKey, requireFileKey } from './config.js'
import { checkMappings, resolveComponent } from './contract/resolve.js'
import { retirementArtifacts } from './guard/apply-retirements.js'
import { checkScope } from './guard/check-scope.js'
import { buildOwnership } from './guard/ownership.js'
import { RETIREMENTS_PATH, loadRetirements, serializeRetirements } from './guard/retirements.js'
import { createRuntime } from './runtime.js'
import { EnvTokenProvider, FigmaClient } from './figma/client.js'
import { ingestChunkFiles } from './capture/ingest-chunks.js'
import { mergeChunkFiles } from './capture/merge-component-chunks.js'
import { probeAccess } from './probe.js'
import { isEmptyDiff } from './sync.js'
import { writeReviewManifest } from './review-manifest.js'

const USAGE = `Usage:
  dcl-figma promotion-manifest
  dcl-figma promotion-verify
  dcl-figma key-guard [--range <base>..<head> | --no-range] [--no-worktree] [--path <dir>]...
  dcl-figma promote-reviewed
  dcl-figma probe
  dcl-figma pull-live [--apply] [--report <file>] [--page <name>] [--config <file>]
  dcl-figma sync [--source snapshot|rest] [--out <dir>] [--developer [--apply-retirements]]
  dcl-figma diff [--source snapshot|rest] [--out <dir>]
  dcl-figma retire --page-id <id> [--component-id <id>] --reason <text> [--name <text>]
  dcl-figma guard --base <dir> [--staged <dir>] [--captured <id,...>] [--run-kind automated|developer] [--library-only]
  dcl-figma capture-merge <out.json> <chunk.json>...
  dcl-figma capture-ingest <chunk.json>...
  dcl-figma resolve (--node-id <id> | --name <name>) [--variant "Prop=Value"]... [--mapping-root <dir>] [--config <file>]
  dcl-figma check-mappings [--mapping-root <dir>] [--config <file>]
  dcl-figma review-manifest [--out <review-root>] [--config <file>]
`

const print = (value: unknown) => process.stdout.write(`${JSON.stringify(value, null, 2)}\n`)

function sourceKind(value: string | undefined): SourceKind {
  if (value === undefined || value === 'snapshot') return 'snapshot'
  if (value === 'rest') return 'rest'
  throw new Error(`Unknown --source "${value}". Use snapshot or rest.`)
}

async function main(argv: string[]): Promise<number> {
  // The guard has its own flags (--range, --path, ...) and needs no Figma config.
  if (argv[0] === 'key-guard') return keyGuard(argv.slice(1))
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      source: { type: 'string' },
      apply: { type: 'boolean' },
      page: { type: 'string' },
      report: { type: 'string' },
      out: { type: 'string' },
      developer: { type: 'boolean' },
      'page-id': { type: 'string' },
      'component-id': { type: 'string' },
      reason: { type: 'string' },
      base: { type: 'string' },
      staged: { type: 'string' },
      captured: { type: 'string' },
      'run-kind': { type: 'string' },
      'library-only': { type: 'boolean' },
      'foundations-captured': { type: 'boolean' },
      'apply-retirements': { type: 'boolean' },
      'node-id': { type: 'string' },
      name: { type: 'string' },
      variant: { type: 'string', multiple: true },
      'mapping-root': { type: 'string' },
      'library-range': { type: 'string' },
      config: { type: 'string' }
    }
  })
  const [command] = positionals

  if (command === 'promotion-verify') {
    const manifest = await verifyPromotionManifest(await createApp(process.cwd(), values.config))
    print({
      verified: true,
      components: manifest.components.length,
      files: Object.keys(manifest.files).length
    })
    return 0
  }

  if (command === 'promotion-manifest' || command === 'promote-reviewed') {
    const app = await createApp(process.cwd(), values.config)
    const result = await (command === 'promotion-manifest'
      ? writePromotionManifest(app)
      : promoteReviewed(app))
    if (values.report) {
      const file = path.resolve(values.report)
      await mkdir(path.dirname(file), { recursive: true })
      await writeFile(file, `${JSON.stringify(result, null, 2)}\n`)
    }
    print(result)
    return 0
  }

  if (command === 'pull-live') {
    print(
      await pullLive(await createApp(process.cwd(), values.config), {
        apply: values.apply,
        report: values.report ?? '.figma-cache/live-report.json',
        page: values.page
      })
    )
    return 0
  }

  if (command === 'review-manifest') {
    print(await writeReviewManifest(await createApp(process.cwd(), values.config), values.out))
    return 0
  }

  if (command === 'resolve' || command === 'check-mappings') {
    const runtime = await createRuntime({
      cwd: process.cwd(),
      env: process.env,
      configPath: values.config,
      mappingRoot: values['mapping-root'],
      libraryRange: values['library-range']
    })
    const context = await runtime.resolveContext()
    if (command === 'check-mappings') {
      const results = await checkMappings(context)
      const failing = results.filter((r) => r.diagnostics.some((d) => d.severity === 'error'))
      print({ ok: failing.length === 0, checked: results.length, results })
      return failing.length === 0 ? 0 : 1
    }
    const variantProperties = Object.fromEntries(
      (values.variant ?? []).map((pair) => {
        const [key, ...rest] = pair.split('=')
        return [key!, rest.join('=')]
      })
    )
    const result = await resolveComponent(context, {
      nodeId: values['node-id'],
      name: values.name,
      variantProperties
    })
    print(result)
    return result.ok ? 0 : 1
  }

  if (command === 'probe') {
    const result = await probeAccess(new FigmaClient(new EnvTokenProvider()), requireFileKey())
    print(result)
    return result.file === 'ok' ? 0 : 1
  }

  if (command === 'capture-merge') {
    const [, output, ...chunks] = positionals
    if (!output || chunks.length === 0) throw new Error(USAGE)
    const merged = await mergeChunkFiles(output, chunks)
    print({ page: merged.page.name, components: merged.components.length })
    return 0
  }

  if (command === 'capture-ingest') {
    const [, ...chunks] = positionals
    if (chunks.length === 0) throw new Error(USAGE)
    const app = await createApp(process.cwd(), values.config)
    print(await ingestChunkFiles(path.resolve(app.cwd, app.config.snapshotDir), chunks))
    return 0
  }

  // A developer records a retirement: the only way output is ever removed. The owned artifacts are
  // computed from the ownership map and written into the record for the reviewer to read.
  if (command === 'retire') {
    const pageId = values['page-id']
    if (!pageId || !values.reason) throw new Error(USAGE)
    const root = path.resolve(process.cwd(), '..')
    const ownership = await buildOwnership(root)
    const componentId = values['component-id']
    const name = values.name ?? (componentId ? ownership.componentById.get(componentId) : undefined)
    if (!ownership.pageNames.has(pageId))
      throw new Error(`Unknown page ID ${pageId}: it is in no configured or saved page`)
    if (componentId) {
      const component = ownership.componentById.get(componentId)
      if (!component) throw new Error(`Unknown component ID ${componentId}`)
      if (componentOwnerById(ownership, componentId)?.pageId !== pageId)
        throw new Error(`Component ${componentId} does not belong to page ${pageId}`)
    }
    const retirements = await loadRetirements(root)
    if (
      retirements.retired.some(
        (entry) => entry.pageId === pageId && entry.componentId === componentId
      )
    )
      throw new Error('That page or component is already retired')
    const artifacts = await retirementArtifacts(root, { pageId, componentId })
    retirements.retired.push({
      pageId,
      ...(componentId ? { componentId } : {}),
      name: name ?? ownership.pageNames.get(pageId)!,
      reason: values.reason,
      owned: artifacts.owned,
      sharedExclusions: artifacts.sharedExclusions
    })
    const file = path.join(root, RETIREMENTS_PATH)
    await mkdir(path.dirname(file), { recursive: true })
    await writeFile(file, serializeRetirements(retirements))
    print(retirements.retired.at(-1))
    return 0
  }

  // The CI guard: compares a staged tree with the base checkout. Automated by default; only a developer
  // pull request may run as `developer`.
  if (command === 'guard') {
    if (!values.base) throw new Error(USAGE)
    const baseDir = path.resolve(values.base)
    const stagedDir = path.resolve(values.staged ?? path.resolve(process.cwd(), '..'))
    const runKind = values['run-kind'] === 'developer' ? 'developer' : 'automated'
    const result = await checkScope({
      baseDir,
      stagedDir,
      foundationsCaptured: values['foundations-captured'],
      capturedPageIds: (values.captured ?? '').split(',').filter(Boolean),
      runKind,
      // The CI guard compares two whole checkouts.
      stage: 'complete',
      retirements: await loadRetirements(runKind === 'developer' ? stagedDir : baseDir),
      removalsOnly: ['components'],
      ...(values['library-only']
        ? {
            directories: ['components'],
            files: ['index.ts', 'components/promotion.json']
          }
        : {})
    })
    // A developer or review change to sealed source must reseal the review promotion manifest in the
    // same tree, or promotion of the next proposal fails on an entry no capture owns.
    const sealIssues =
      runKind === 'developer' &&
      !values['library-only'] &&
      existsSync(path.join(stagedDir, PROMOTION_MANIFEST))
        ? await staleSeal(await createApp(path.join(stagedDir, 'figma')))
        : []
    const combined = {
      ok: result.ok && sealIssues.length === 0,
      violations: [
        ...result.violations,
        ...sealIssues.map((entry) => ({
          path: PROMOTION_MANIFEST,
          owner: { aggregate: 'promotion-manifest' },
          kind: 'out-of-scope-change',
          cause: `sealed ${entry} does not match the tree; run cli promotion-manifest`
        }))
      ]
    }
    print(combined)
    return combined.ok ? 0 : 1
  }

  if (command === 'sync' || command === 'diff') {
    const app = await createApp(process.cwd(), values.config)
    const options = {
      source: sourceKind(values.source),
      outDir: values.out,
      developer: values.developer,
      applyRetirements: values['apply-retirements']
    }
    const report = await (command === 'sync' ? runSync : runDiff)(app, options)
    print(report)
    return command === 'diff' && (!isEmptyDiff(report.diff) || report.ledgerIssues?.length) ? 1 : 0
  }

  process.stderr.write(USAGE)
  return 2
}

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (error: unknown) => {
    process.stderr.write(
      `${redactFileKey(error instanceof Error ? error.message : String(error))}\n`
    )
    process.exit(1)
  }
)
