import { existsSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { createApp, type App } from './app.js'
import { resolveContractConfig } from './contract/config.js'
import { buildFigmaIndex } from './contract/figma-index.js'
import { LfsPointerError } from './lfs.js'
import type { ResolveContext } from './contract/resolve.js'
import { promotionExclusion } from './promotion/manifest.js'
import { loadSpecsFromSource } from './specs.js'
import type { ComponentSpec } from './ir/components.js'

type RuntimeOptions = {
  cwd: string
  env: NodeJS.ProcessEnv
  configPath?: string
  mappingRoot?: string
  libraryRange?: string
}

type Runtime = {
  // Undefined in resolve-only mode: a consuming project that has no Figma config of its own.
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

async function syncedComponents(app: App): Promise<Map<string, string[]>> {
  let specs
  try {
    specs = (await loadSpecsFromSource(app.createSource('snapshot'), app.config)).specs
  } catch (error) {
    // Without a complete capture, hand-written mappings are validated as before; an unmaterialized
    // capture is a broken checkout, not a missing one.
    if (error instanceof LfsPointerError) throw error
    return new Map()
  }
  const record = await readFile(path.resolve(app.cwd, '..', 'components/promotion.json'), 'utf8')
    .then((text) => (JSON.parse(text) as { components: string[] }).components)
    .catch((error: NodeJS.ErrnoException) => {
      if (error.code === 'ENOENT') return [] as string[]
      throw error
    })
  return supersessionCandidates(specs, app.config.promotion?.include, record)
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
      if (app) {
        try {
          figmaSets = await buildFigmaIndex(app.createSource('snapshot'), app.config)
        } catch (error) {
          // A missing capture means resolve-only mode; an unmaterialized one is a broken checkout.
          if (error instanceof LfsPointerError) throw error
          figmaSets = []
        }
      }
      return {
        mappingRoot: contract.mappingRoot,
        libraryRange: contract.libraryRange,
        figmaSets,
        loadSyncedComponents: app ? () => syncedComponents(app) : undefined
      }
    }
  }
}

export { createRuntime, supersessionCandidates }
export type { Runtime, RuntimeOptions }
