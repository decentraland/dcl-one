import path from 'node:path'

type ContractConfigInput = {
  cwd: string
  args: { mappingRoot?: string; libraryRange?: string }
  env: NodeJS.ProcessEnv
  fileConfig?: { dir: string; mappingRoot?: string; libraryRange?: string }
}

type ContractConfig = {
  mappingRoot?: string
  libraryRange?: string
  origin: 'argument' | 'environment' | 'figma.config.json' | 'none'
}

// Precedence: CLI argument, then environment, then figma.config.json. Argument and environment paths are
// relative to the process working directory (the consuming project); config-file paths are relative to
// the config file, so the same config works from any working directory.
function resolveContractConfig({
  cwd,
  args,
  env,
  fileConfig
}: ContractConfigInput): ContractConfig {
  const libraryRange = args.libraryRange ?? env.DCL_FIGMA_LIBRARY_RANGE ?? fileConfig?.libraryRange
  if (args.mappingRoot) {
    return { mappingRoot: path.resolve(cwd, args.mappingRoot), libraryRange, origin: 'argument' }
  }
  if (env.DCL_FIGMA_MAPPING_ROOT) {
    return {
      mappingRoot: path.resolve(cwd, env.DCL_FIGMA_MAPPING_ROOT),
      libraryRange,
      origin: 'environment'
    }
  }
  if (fileConfig?.mappingRoot) {
    return {
      mappingRoot: path.resolve(fileConfig.dir, fileConfig.mappingRoot),
      libraryRange,
      origin: 'figma.config.json'
    }
  }
  return { libraryRange, origin: 'none' }
}

export { resolveContractConfig }
export type { ContractConfig }
