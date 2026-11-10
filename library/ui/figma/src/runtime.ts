import { libraryComponentEntries } from './guard/library.js'
import { existsSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createApp, type App } from './app.js'
import { resolveContractConfig } from './contract/config.js'
import { contractSchema } from './config.js'
import { buildFigmaIndex } from './contract/figma-index.js'
import type { ResolveContext } from './contract/resolve.js'
import { promotionExclusion } from './promotion/eligibility.js'
import { loadSpecsFromSource } from './specs.js'
import type { ComponentSpec } from './ir/components.js'

type RuntimeOptions = {
  cwd: string
  env: NodeJS.ProcessEnv
  configPath?: string
  mappingRoot?: string
  libraryRange?: string
  // Resolve against the mappings only: of the Figma config only its contract block is read, never the capture.
  mappingOnly?: boolean
}

type Runtime = {
  // Undefined in resolve-only mode (no Figma config, or --mapping-only).
  app?: App
  resolveContext(): Promise<ResolveContext>
}

// The synced replacements check-mappings reports instead of validating the hand-written mapping, with
// the reasons promotion would still skip each. Supersession follows promotion.include, as promotion does:
// a ready, eligible replacement that include does not name leaves the hand-written mapping validated,
// and an already promoted one has the synced mapping.
function supersessionCandidates(
  specs: ComponentSpec[],
  include: string[] | undefined,
  promoted: string[]
): Map<string, string[]> {
  return new Map(
    specs
      .filter((spec) => include?.includes(spec.name) && !promoted.includes(spec.name))
      .map((spec) => [spec.name, promotionExclusion(spec)])
  )
}

// Only reached with a configured capture: a runtime without Figma config is resolve-only and has no
// loadSyncedComponents at all. Every load failure (an unmaterialized LFS capture, a stale fetch-source cache,
// an I/O or schema error) propagates, so check-mappings never silently validates only the hand-written mappings.
async function syncedComponents(app: App): Promise<Map<string, string[]>> {
  const { specs } = await loadSpecsFromSource(app.createSource('snapshot'), app.config)
  const record = await readFile(path.resolve(app.cwd, '..', 'components/promotion.json'), 'utf8')
    .then((text) => libraryComponentEntries(JSON.parse(text)).map((entry) => entry.name))
    .catch((error: NodeJS.ErrnoException) => {
      if (error.code === 'ENOENT') return [] as string[]
      throw error
    })
  return supersessionCandidates(specs, app.config.promotion?.include, record)
}

// The library's own mapping directory, the default mapping root under --mapping-only.
const LIBRARY_MAPPING_ROOT = fileURLToPath(new URL('../../components', import.meta.url))

// Under --mapping-only only the `contract` block of the Figma config is read (its mapping root and library
// range need no capture); the rest of the config, the app and the capture cache are never loaded.
async function contractBlock(
  configFile: string,
  explicit: boolean
): Promise<{ dir: string; mappingRoot?: string; libraryRange?: string } | undefined> {
  if (!existsSync(configFile)) {
    if (explicit) throw new Error(`Figma config not found: ${configFile}`)
    return undefined
  }
  let raw: { contract?: unknown }
  try {
    raw = JSON.parse(await readFile(configFile, 'utf8'))
  } catch (error) {
    throw new Error(`${configFile}: ${(error as Error).message}`, { cause: error })
  }
  const parsed = contractSchema.optional().safeParse(raw?.contract)
  if (!parsed.success)
    throw new Error(
      `${configFile}: contract must hold a string mappingRoot and an optional string libraryRange`
    )
  return { dir: path.dirname(configFile), ...parsed.data }
}

function mappingOnlyRuntime(
  options: RuntimeOptions,
  configFile: string,
  explicit: boolean
): Runtime {
  return {
    async resolveContext() {
      const contract = resolveContractConfig({
        cwd: options.cwd,
        args: { mappingRoot: options.mappingRoot, libraryRange: options.libraryRange },
        env: options.env,
        fileConfig: await contractBlock(configFile, explicit)
      })
      return {
        mappingRoot: contract.mappingRoot ?? LIBRARY_MAPPING_ROOT,
        libraryRange: contract.libraryRange,
        figmaSets: []
      }
    }
  }
}

async function createRuntime(options: RuntimeOptions): Promise<Runtime> {
  const { cwd, env } = options
  const configFile = path.resolve(
    cwd,
    options.configPath ?? env.DCL_FIGMA_CONFIG ?? 'figma.config.json'
  )
  const explicit = Boolean(options.configPath ?? env.DCL_FIGMA_CONFIG)
  if (explicit && !existsSync(configFile)) {
    throw new Error(`Figma config not found: ${configFile}`)
  }
  if (options.mappingOnly) return mappingOnlyRuntime(options, configFile, explicit)
  const app = existsSync(configFile) ? await createApp(cwd, configFile) : undefined

  return {
    app,
    async resolveContext() {
      const contract = resolveContractConfig({
        cwd,
        args: { mappingRoot: options.mappingRoot, libraryRange: options.libraryRange },
        env,
        fileConfig: app
          ? {
              dir: app.cwd,
              mappingRoot: app.config.contract?.mappingRoot,
              libraryRange: app.config.contract?.libraryRange
            }
          : undefined
      })
      let figmaSets: ResolveContext['figmaSets'] = []
      // Without --mapping-only, only a missing Figma configuration means resolve-only mode: a configured capture
      // that fails to load (malformed, or an unmaterialized LFS pointer) is a broken checkout and fails closed.
      if (app) figmaSets = await buildFigmaIndex(app.createSource('snapshot'), app.config)
      return {
        mappingRoot: contract.mappingRoot,
        libraryRange: contract.libraryRange,
        figmaSets,
        loadSyncedComponents: app ? () => syncedComponents(app) : undefined
      }
    }
  }
}

export { LIBRARY_MAPPING_ROOT, createRuntime, supersessionCandidates }
export type { Runtime, RuntimeOptions }
