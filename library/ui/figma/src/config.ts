import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { z } from 'zod'
import { semanticSchema, textRoleSchema } from './ir/components.js'

const pageSchema = z.object({
  id: z.string().min(1).nullable().optional(),
  name: z.string().min(1).optional(),
  kind: z.enum(['tokens', 'icons', 'components', 'assets', 'docs']),
  category: z.string().optional()
})

const componentOverrideSchema = z.object({
  stateWords: z.array(z.enum(['hover', 'pressed', 'selected', 'disabled', 'loading'])).optional(),
  optionAliases: z.record(z.string(), z.string()).optional(),
  propNames: z.record(z.string(), z.string()).optional(),
  skipOptions: z.array(z.string()).optional(),
  exposeTextSlots: z.boolean().optional(),
  semantics: z.record(z.string(), semanticSchema).optional(),
  textRoles: z.record(z.string(), textRoleSchema).optional()
})

// Exclusion removes a public token export, so each supported name is reviewed here, not only in config.
const SUPPORTED_PRIMITIVE_EXCLUSIONS: readonly string[] = ['Primitives/Primary/Primary blue TEST']

// Name checks that need no capture: config loading and generation both apply them.
function exclusionNameErrors(excluded: readonly string[]): string[] {
  const errors: string[] = []
  const seen = new Set<string>()
  for (const entry of excluded) {
    if (seen.has(entry)) errors.push(`Excluded primitive ${entry} is listed more than once`)
    else if (!SUPPORTED_PRIMITIVE_EXCLUSIONS.includes(entry))
      errors.push(`Excluded primitive ${entry} is not a supported exclusion`)
    seen.add(entry)
  }
  return errors
}

const contractSchema = z.object({
  // Directory inside the component library that holds the *.figma.ts mapping files.
  mappingRoot: z.string(),
  libraryRange: z.string().optional()
})

const configSchema = z.object({
  outDir: z.string(),
  snapshotDir: z.string().default('figma-snapshot'),
  // The lock that pins the figma-design-system capture. When set, snapshotDir is the cache `pnpm figma
  // fetch-source` materializes, and every read first checks it against this lock.
  snapshotLock: z.string().optional(),
  // How generated components carry image fills: as asset files, or as a neutral placeholder (the Review, which is
  // published and must carry no Figma imagery).
  pages: z.record(z.string(), pageSchema),
  variantAliases: z.record(z.string(), z.string()).default({}),
  componentNames: z.record(z.string(), z.string()).default({}),
  textStyleAliases: z.record(z.string(), z.string()).default({}),
  iconAliases: z.record(z.string(), z.string()).default({}),
  components: z.record(z.string(), componentOverrideSchema).default({}),
  // 'placeholder' replaces raster image fills with a neutral fill, so a consumer ships no Figma image exports.
  imageFills: z.enum(['assets', 'placeholder']).default('assets'),
  // Saved primitives that generation leaves out of the token exports. The capture keeps them.
  excludedPrimitives: z
    .array(z.string())
    .default([])
    .superRefine((excluded, context) => {
      for (const message of exclusionNameErrors(excluded))
        context.addIssue({ code: 'custom', message })
    }),
  contract: contractSchema.optional(),
  // Reviewed components promote-reviewed copies into library/ui; their preview dependencies are
  // added automatically. Omitted: every eligible component.
  promotion: z.object({ include: z.array(z.string().min(1)).min(1) }).optional(),
  selection: z
    .object({
      readyOnly: z.boolean().default(true),
      exceptions: z
        .array(
          z.object({
            pageId: z.string().min(1),
            componentId: z.string().min(1),
            reason: z.string().trim().min(1)
          })
        )
        .default([])
    })
    .optional()
})

type FigmaConfig = z.infer<typeof configSchema>

// The Figma file key is private: it is never stored in tracked config, snapshots or generated
// output, and never printed. Live Figma access reads it from the environment only.
const FILE_KEY_ENV = 'FIGMA_FILE_KEY'
const FILE_KEY_PATTERN = /^[0-9A-Za-z]{22,128}$/

function readFileKey(env: NodeJS.ProcessEnv = process.env): string | undefined {
  const value = env[FILE_KEY_ENV]?.trim()
  if (!value) return undefined
  if (!FILE_KEY_PATTERN.test(value))
    throw new Error(`${FILE_KEY_ENV} is not a valid Figma file key`)
  return value
}

/** Replaces the configured file key in text bound for logs, reports or errors. */
function redactFileKey(text: string, env: NodeJS.ProcessEnv = process.env): string {
  const value = env[FILE_KEY_ENV]?.trim()
  return value ? text.split(value).join('<DCL_FIGMA_FILE>') : text
}

function requireFileKey(env: NodeJS.ProcessEnv = process.env): string {
  const value = readFileKey(env)
  if (!value)
    throw new Error(
      `${FILE_KEY_ENV} is not set. Live Figma inspection needs it; sync, diff and check-mappings work from the fetched capture without it.`
    )
  return value
}

async function loadConfigFile(file: string): Promise<FigmaConfig> {
  return configSchema.parse(JSON.parse(await readFile(file, 'utf8')))
}

async function loadConfig(cwd = process.cwd()): Promise<FigmaConfig> {
  return loadConfigFile(path.join(cwd, 'figma.config.json'))
}

export {
  configSchema,
  FILE_KEY_ENV,
  exclusionNameErrors,
  loadConfig,
  loadConfigFile,
  readFileKey,
  redactFileKey,
  requireFileKey,
  SUPPORTED_PRIMITIVE_EXCLUSIONS
}
export type { FigmaConfig }
