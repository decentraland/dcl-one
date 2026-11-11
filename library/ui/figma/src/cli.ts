#!/usr/bin/env node
import { componentOwnerById } from './guard/component-identity.js'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { main as keyGuard } from '../key-guard.mjs'
import { parseArgs } from 'node:util'
import { createApp, runDiff, runSync, type SourceKind } from './app.js'
import { errorText } from './server-errors.js'
import { resolveBatch } from './contract/batch.js'
import { checkMappingsReport, resolveComponent } from './contract/resolve.js'
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
import { portPrepare, portVerify } from './port.js'
import { fetchFdsSource } from './sources/fds-source.js'

const USAGE = `Usage:
  dcl-figma fetch-source [--seal]
  dcl-figma port prepare | port verify
  dcl-figma key-guard [--range <base>..<head> | --no-range] [--no-worktree] [--path <dir>]...
  dcl-figma probe
  dcl-figma install foundations|components [--source snapshot] [--out <private-stage>]
  dcl-figma sync [--out <dir>] [--developer [--apply-retirements]]
  dcl-figma diff [--source snapshot|rest] [--out <dir>]
  dcl-figma retire --page-id <id> [--component-id <id>] --reason <text> [--name <text>]
  dcl-figma guard --base <dir> [--staged <dir>] [--captured <id,...>] [--run-kind automated|developer] [--library-only]
  dcl-figma resolve (--node-id <id> | --name <name>) [--variant 'Prop=Value']... [--boolean 'Prop=true|false']... [--text 'Layer=Value']... [--mapping-only] [--mapping-root <dir>] [--config <file>]
  dcl-figma resolve --batch <file|-> [--mapping-only] [--mapping-root <dir>] [--config <file>]
  dcl-figma check-mappings [--mapping-only] [--mapping-root <dir>] [--config <file>]
  dcl-figma review-manifest [--out <review-root>] [--config <file>]
`

// The `figma` script of this package runs `cd figma`, and pnpm sets INIT_CWD to where it was run, so a path
// reads as typed. Any other launch (the dcl-figma bin from another package's script, which inherits that
// script's INIT_CWD) reads paths from its own working directory.
function invocationDirectory(): string {
  const { INIT_CWD, npm_lifecycle_event, npm_package_name } = process.env
  return INIT_CWD && npm_lifecycle_event === 'figma' && npm_package_name === '@dcl/one-ui'
    ? INIT_CWD
    : process.cwd()
}

// Repeated "Key=Value" flags. A pair without "=", an empty key or a repeated key is refused, never guessed.
function pairs(flag: string, entries: string[] | undefined): Record<string, string> | undefined {
  if (!entries?.length) return undefined
  // No prototype: a key such as __proto__ is kept as data and its repeat is still detected.
  const result: Record<string, string> = Object.create(null)
  for (const entry of entries) {
    const split = entry.indexOf('=')
    const key = split > 0 ? entry.slice(0, split) : ''
    if (!key) throw new Error(`--${flag} takes "Name=Value", got "${entry}"`)
    if (Object.hasOwn(result, key)) throw new Error(`--${flag} names "${key}" twice`)
    result[key] = entry.slice(split + 1)
  }
  return result
}

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = []
  for await (const chunk of process.stdin) chunks.push(chunk as Buffer)
  return Buffer.concat(chunks).toString('utf8')
}

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
  // The maintainer port: prepare mutates the tree in order and stops at the first failed stage; verify runs the
  // seven checks on a clean tree and writes the record only when all pass. Maintainers only, never CI.
  if (argv[0] === 'port') {
    if (argv.length !== 2 || (argv[1] !== 'prepare' && argv[1] !== 'verify'))
      throw new Error('port takes prepare or verify')
    const result = argv[1] === 'prepare' ? portPrepare(process.cwd()) : portVerify(process.cwd())
    print(result)
    return result.ok ? 0 : 1
  }
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
      boolean: { type: 'string', multiple: true },
      text: { type: 'string', multiple: true },
      batch: { type: 'string' },
      'mapping-only': { type: 'boolean' },
      'mapping-root': { type: 'string' },
      'library-range': { type: 'string' },
      config: { type: 'string' }
    }
  })
  const [command] = positionals
  if (values['mapping-only'] && command !== 'resolve' && command !== 'check-mappings')
    throw new Error('--mapping-only applies only to resolve and check-mappings')
  if (values.text && command !== 'resolve') throw new Error('--text applies only to resolve')
  if (values.boolean && command !== 'resolve') throw new Error('--boolean applies only to resolve')
  if (values.batch !== undefined) {
    if (command !== 'resolve') throw new Error('--batch applies only to resolve')
    if (values['node-id'] || values.name || values.variant || values.boolean || values.text)
      throw new Error(
        '--batch takes its instances from the file; drop --node-id, --name, --variant, --boolean and --text'
      )
  }

  if (command === 'install') {
    if (positionals.length !== 2) throw new Error(USAGE)
    // Install runs in its own process, which loads only the import closure components/generation.json binds.
    const forwarded = [positionals[1]!, '--source', sourceKind(values.source)]
    // name=value keeps a value that starts with '-' a value in the child's strict parser.
    if (values.out !== undefined) forwarded.push(`--out=${values.out}`)
    if (values.config !== undefined) forwarded.push(`--config=${values.config}`)
    // Only the tsx loader flags reach the child (install-main.ts refuses any other), and tsx resolves through the
    // bound figma/tsconfig.json whatever the working directory.
    const loader: string[] = []
    for (let index = 0; index < process.execArgv.length; index += 1) {
      const flag = process.execArgv[index]!
      if ((flag === '--require' || flag === '--import') && index + 1 < process.execArgv.length)
        loader.push(flag, process.execArgv[++index]!)
    }
    const child = spawn(
      process.execPath,
      [...loader, fileURLToPath(new URL('./install-main.ts', import.meta.url)), ...forwarded],
      {
        stdio: 'inherit',
        env: {
          ...process.env,
          TSX_TSCONFIG_PATH: fileURLToPath(new URL('../tsconfig.json', import.meta.url))
        }
      }
    )
    // A stop sent to the CLI alone (a timeout, an IDE, a CI step) reaches the install too.
    const forward = (signal: NodeJS.Signals) => child.kill(signal)
    process.on('SIGINT', forward).on('SIGTERM', forward)
    const [code, signal] = await new Promise<[number | null, NodeJS.Signals | null]>(
      (resolve, reject) => {
        child.once('error', reject)
        child.once('exit', (exitCode, exitSignal) => resolve([exitCode, exitSignal]))
      }
    ).finally(() => process.off('SIGINT', forward).off('SIGTERM', forward))
    if (signal) throw new Error(`The install process ended on signal ${signal}`)
    return code ?? 1
  }

  if (command === 'review-manifest') {
    print(await writeReviewManifest(await createApp(process.cwd(), values.config), values.out))
    return 0
  }

  if (command === 'resolve' || command === 'check-mappings') {
    // Flags are parsed before the runtime, so a malformed flag is reported even when the cache is broken.
    const input = {
      nodeId: values['node-id'],
      name: values.name,
      variantProperties: pairs('variant', values.variant),
      booleanProperties: pairs('boolean', values.boolean),
      texts: pairs('text', values.text)
    }
    // In batch mode exit 1 means only that an entry failed: a batch that cannot be read, a broken invocation and a
    // configuration or library that cannot be used all exit 2 (see batchMode below).
    let batchEntries: unknown
    if (values.batch !== undefined) {
      let text: string
      try {
        text =
          values.batch === '-'
            ? await readStdin()
            : await readFile(path.resolve(invocationDirectory(), values.batch), 'utf8')
      } catch (error) {
        process.stderr.write(
          `--batch cannot read ${values.batch}: ${(error as NodeJS.ErrnoException).code ?? 'error'}\n`
        )
        return 2
      }
      try {
        batchEntries = JSON.parse(text)
      } catch {
        batchEntries = undefined
      }
      if (!Array.isArray(batchEntries) || batchEntries.length === 0) {
        process.stderr.write('--batch needs a non-empty JSON array of instances\n')
        return 2
      }
    }
    const runtime = await createRuntime({
      cwd: process.cwd(),
      env: process.env,
      configPath: values.config,
      mappingRoot: values['mapping-root'],
      libraryRange: values['library-range'],
      mappingOnly: values['mapping-only']
    })
    const context = await runtime.resolveContext()
    if (command === 'check-mappings') {
      const report = await checkMappingsReport(context)
      print(report)
      return report.ok ? 0 : 1
    }
    if (batchEntries !== undefined) {
      const batch = await resolveBatch(context, batchEntries)
      print(batch)
      if ('errors' in batch) return 2
      return batch.ok ? 0 : 1
    }
    const result = await resolveComponent(context, input)
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

// A thrown error in batch mode (a misused flag, an unusable configuration) is a broken invocation, never the
// exit 1 a caller reads as "an entry failed".
const batchMode = process.argv
  .slice(2)
  .some((arg) => arg === '--batch' || arg.startsWith('--batch='))

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (error: unknown) => {
    process.stderr.write(`${errorText(error)}\n`)
    process.exit(batchMode ? 2 : 1)
  }
)
