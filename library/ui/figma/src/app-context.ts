import path from 'node:path'
import { configuredPages } from './page-identity.js'
import { loadConfig, loadConfigFile, requireFileKey, type FigmaConfig } from './config.js'
import { EnvTokenProvider, FigmaClient } from './figma/client.js'
import { RestSource } from './sources/rest-source.js'
import { SnapshotSource } from './sources/snapshot-source.js'
import { FdsCacheError, verifyFdsCache } from './sources/fds-source.js'
import type { FigmaSource } from './sources/types.js'
import { RETIREMENTS_PATH } from './guard/retirements.js'

// The configuration and source every command starts from. It lives apart from sync so the component installer,
// whose import closure components/generation.json binds, does not load the sync tooling.

type SourceKind = FigmaSource['name']

type App = {
  config: FigmaConfig
  cwd: string
  configFile?: string
  createSource(kind?: SourceKind): FigmaSource
  resolveOutDir(override?: string): string
}

type AppOptions = {
  // Reads the capture directory without checking it against upstream.lock.json. Only tests over the synthetic
  // capture and the retirement cleanup's staged copy of an already verified cache pass it.
  unverifiedSnapshot?: boolean
}

// `configFile` lets a consuming project point at a config that lives elsewhere. Every path in the config
// (snapshotDir, outDir, contract.mappingRoot) is relative to the config file, not to the process.
async function createApp(
  cwd = process.cwd(),
  configFile?: string,
  options: AppOptions = {}
): Promise<App> {
  const loadedConfigFile = path.resolve(configFile ?? path.join(cwd, 'figma.config.json'))
  const config = configFile ? await loadConfigFile(configFile) : await loadConfig(cwd)
  if (configFile) cwd = path.dirname(path.resolve(configFile))
  return {
    config,
    configFile: loadedConfigFile,
    cwd,
    // `rest` reads the live file for local inspection only (FIGMA_TOKEN and FIGMA_FILE_KEY from the environment).
    // Everything that writes tracked files reads the verified fetch-source cache: see runSync.
    createSource(kind = 'snapshot') {
      if (kind === 'rest')
        return new RestSource(new FigmaClient(new EnvTokenProvider()), requireFileKey())
      const dir = path.resolve(cwd, config.snapshotDir)
      const lock = config.snapshotLock && path.resolve(cwd, config.snapshotLock)
      // Every command, the MCP server included, reads only the cache fetch-source verified, so a config that
      // names no lock fails here instead of reading whatever the directory holds.
      if (!lock && !options.unverifiedSnapshot)
        throw new FdsCacheError(
          `${path.basename(loadedConfigFile)} names no snapshotLock, so the Figma capture cannot be verified against upstream.lock.json: set snapshotLock and run \`pnpm figma fetch-source\``
        )
      return new SnapshotSource(
        dir,
        configuredPages(config)
          .filter((page) => page.id === null)
          .map((page) => page.name),
        lock && !options.unverifiedSnapshot ? () => verifyFdsCache(dir, lock) : undefined,
        path.resolve(cwd, '..', RETIREMENTS_PATH)
      )
    },
    resolveOutDir: (override) => path.resolve(cwd, override ?? config.outDir)
  }
}

export { createApp }
export type { App, AppOptions, SourceKind }
