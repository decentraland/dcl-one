#!/usr/bin/env node
import { componentOwnerById } from './guard/component-identity.js'
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { main as keyGuard } from '../key-guard.mjs'
import { parseArgs } from 'node:util'
import { createApp, runDiff, runSync, type SourceKind } from './app.js'
import { errorText } from './server-errors.js'
import { checkMappings, resolveComponent } from './contract/resolve.js'
import { retirementArtifacts } from './guard/apply-retirements.js'
import { checkScope } from './guard/check-scope.js'
import { buildOwnership } from './guard/ownership.js'
import { RETIREMENTS_PATH, loadRetirements, serializeRetirements } from './guard/retirements.js'
import { createRuntime } from './runtime.js'
import { requireFileKey } from './config.js'
import { EnvTokenProvider, FigmaClient } from './figma/client.js'
import { probeAccess } from './probe.js'
import { isEmptyDiff } from './sync.js'
import { writeReviewManifest } from './review-manifest.js'
import { fetchFdsSource } from './sources/fds-source.js'

const USAGE = `Usage:
  dcl-figma fetch-source [--seal]
  dcl-figma key-guard [--range <base>..<head> | --no-range] [--no-worktree] [--path <dir>]...
  dcl-figma probe
  dcl-figma sync [--out <dir>] [--developer [--apply-retirements]]
  dcl-figma diff [--source snapshot|rest] [--out <dir>]
  dcl-figma retire --page-id <id> [--component-id <id>] --reason <text> [--name <text>]
  dcl-figma guard --base <dir> [--staged <dir>] [--captured <id,...>] [--run-kind automated|developer] [--library-only]
  dcl-figma resolve (--node-id <id> | --name <name>) [--variant "Prop=Value"]... [--mapping-root <dir>] [--config <file>]
  dcl-figma check-mappings [--mapping-root <dir>] [--config <file>]
  dcl-figma review-manifest [--out <review-root>] [--config <file>]
`

const print = (value: unknown) => process.stdout.write(`${JSON.stringify(value, null, 2)}\n`)

// `rest` reads the live file for local inspection (FIGMA_TOKEN and FIGMA_FILE_KEY from the environment); sync
// refuses it, because shipped output comes from the verified fetch-source cache.
function sourceKind(value: string | undefined): SourceKind {
  if (value === undefined || value === 'snapshot') return 'snapshot'
  if (value === 'rest') return 'rest'
  throw new Error(`Unknown --source "${value}". Use snapshot or rest.`)
}

async function main(argv: string[]): Promise<number> {
  // The guard has its own flags (--range, --path, ...) and needs no Figma config.
  if (argv[0] === 'key-guard') return keyGuard(argv.slice(1))
  // Materializes the figma-design-system capture pinned in upstream.lock.json; maintainers only, never CI.
  if (argv[0] === 'fetch-source') {
    const rest = argv.slice(1)
    if (rest.some((arg) => arg !== '--seal')) throw new Error('fetch-source takes only --seal')
    const result = await fetchFdsSource({ figmaRoot: process.cwd(), seal: rest.includes('--seal') })
    print({ ...result, sealed: rest.includes('--seal') })
    return 0
  }
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      source: { type: 'string' },
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

  // A developer records a retirement: the only way output is ever removed. The owned artifacts are
  // computed from the ownership map and written into the record for the reviewer to read.
  if (command === 'retire') {
    const pageId = values['page-id']
    if (!pageId || !values.reason) throw new Error(USAGE)
    // Ownership is derived from the capture at figma/figma-snapshot, so that exact directory is verified first:
    // a --config naming another snapshotDir would verify one cache and read another.
    const app = await createApp(process.cwd(), values.config)
    const root = path.resolve(process.cwd(), '..')
    if (
      path.resolve(app.cwd, app.config.snapshotDir) !== path.resolve(root, 'figma/figma-snapshot')
    )
      throw new Error(
        'retire reads the capture at figma/figma-snapshot, but the config names another snapshotDir'
      )
    await app.createSource('snapshot').ready?.()
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
    const combined = { ok: result.ok, violations: result.violations }
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
    process.stderr.write(`${errorText(error)}\n`)
    process.exit(1)
  }
)
