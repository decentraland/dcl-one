// this file is tested extensively to build scenes in the `make test` command
// but since it runs outiside the testing harness, coverage is not collected

import { Scene } from '@dcl/schemas'
import child_process from 'child_process'
import esbuild from 'esbuild'
import { future } from './future'
import { globSync } from 'fs'
import fs from 'fs'
import path from 'path'
import { pathToFileURL } from 'url'
import i18next from 'i18next'
import { applyEdits, findNodeAtLocation, getNodeValue, modify, parseTree, ParseError } from 'jsonc-parser'

import { CliComponents } from '../components'
import { colors } from '../components/log'
import { printProgressInfo, printProgressStep, printWarning } from './beautiful-logs'
import { CliError } from './error'
import { fixTsconfigForTypescript7 } from './tsconfig'
import { getAllComposites, type Script } from './composite'
import { isEditorScene } from './project-validations'
import { watch } from 'chokidar'
import { debounce } from './debounce'

export type BundleComponents = Pick<CliComponents, 'logger' | 'fs'>

export type SceneJson = {
  main: string
  runtimeVersion: string
}

export type CompileOptions = {
  production: boolean
  watch: boolean

  // build a single .ts file
  single?: string

  // build a project folder
  workingDirectory: string

  // emit typescript declarations
  emitDeclaration: boolean

  ignoreComposite: boolean
  customEntryPoint: boolean

  // write the project's build-time defaults (scene.json, package.json): `build` and `start`, never a deploy
  projectDefaults: boolean
}

const MAX_STEP = 2

// Keep only the last 10KB of output to prevent memory leaks in watch mode
const MAX_OUTPUT_SIZE = 10 * 1024

/**
 * Generate the entry-point code for a given original entry-point
 * @param entrypointPath - file to be imported as original entry point
 * @param forceCustomExport
 * @returns the Typescript code
 */

/**
 * Converts a file path to a safe JavaScript module path string.
 * Normalizes to Unix-style forward slashes and escapes special characters.
 */
function toSafeModulePath(filePath: string): string {
  const unixPath = filePath.replace(/\\/g, '/')
  return JSON.stringify(unixPath)
}

function getEntrypointCode(entrypointPath: string, forceCustomExport: boolean) {
  const safeEntrypointPath = toSafeModulePath(entrypointPath)
  if (forceCustomExport) return `;"use strict";export * from ${safeEntrypointPath}`

  return `// BEGIN AUTO GENERATED CODE "~sdk/scene-entrypoint"
"use strict";
import * as entrypoint from ${safeEntrypointPath}
import { engine, NetworkEntity } from '@dcl/sdk/ecs'
import * as sdk from '@dcl/sdk'
import { compositeProvider } from '@dcl/sdk/composite-provider'
import { compositeFromLoader } from '~sdk/all-composites'
import { _initializeScripts, _initializeSmartItems } from '~sdk/script-utils'

_initializeSmartItems()

if ((entrypoint as any).main !== undefined) {
  function _INTERNAL_startup_system() {
    try {
      // initialize and run all scripts
      _initializeScripts(engine)

      const maybePromise = (entrypoint as any).main()
      if (maybePromise && typeof maybePromise === 'object' && typeof (maybePromise as unknown as Promise<unknown>).then === 'function') {
        maybePromise.catch(console.error)
      }
    } catch (e) {
     console.error(e)
    } finally {
      engine.removeSystem(_INTERNAL_startup_system)
    }
  }
  engine.addSystem(_INTERNAL_startup_system, Infinity)
}

export * from '@dcl/sdk'
export * from ${safeEntrypointPath}
export * from '~sdk/script-utils'
`
}

/**
 * Ensures a key exists in a JSON file. Supports dot-separated paths (e.g. "scripts.server-logs").
 * Preserves existing values unless they match the supplied previous default, and keeps the file's
 * formatting (indentation, line endings, comments) when it writes.
 */
async function ensureJsonKey(
  components: BundleComponents,
  filePath: string,
  keyPath: string,
  value: unknown,
  previousDefault?: string
): Promise<void> {
  try {
    const raw = await components.fs.readFile(filePath, 'utf-8')
    const errors: ParseError[] = []
    const tree = parseTree(raw, errors)
    if (!tree || errors.length) return

    const keys = keyPath.split('.')
    const node = findNodeAtLocation(tree, keys)
    const existing = node ? getNodeValue(node) : undefined
    if (existing !== undefined && (previousDefault === undefined || existing !== previousDefault)) return

    const indent = /\n([ \t]+)\S/.exec(raw)?.[1] ?? '  '
    const formattingOptions = {
      insertSpaces: indent[0] !== '\t',
      tabSize: indent[0] === '\t' ? 1 : indent.length,
      eol: raw.includes('\r\n') ? '\r\n' : '\n'
    }
    await components.fs.writeFile(filePath, applyEdits(raw, modify(raw, keys, value, { formattingOptions })))
    printProgressInfo(
      components.logger,
      `${existing === undefined ? 'Added' : 'Updated'} ${colors.bold(`${keyPath}: ${JSON.stringify(value)}`)} in ${path.basename(filePath)}`
    )
  } catch (_) {
    // read/write failed — skip silently
  }
}

const SMART_ITEMS_SPECIFIER = '@dcl/smart-items'
const SMART_ITEMS_IMPORT = /^@dcl\/(asset-packs|smart-items)(\/.*)?$/

/**
 * The `@dcl/smart-items` specifier an `@dcl/asset-packs` import resolves to: the package root and
 * `dist/scene-entrypoint` map to the smart-items entry, `dist/<path>` (`.js` stripped) to the module
 * at that path. Any other subpath returns undefined and resolves as written.
 */
export function smartItemsSpecifier(specifier: string): string | undefined {
  if (specifier === '@dcl/asset-packs') return SMART_ITEMS_SPECIFIER
  const match = /^@dcl\/asset-packs\/dist\/(.+?)(\.js)?$/.exec(specifier)
  if (!match) return undefined
  return match[1] === 'scene-entrypoint' ? SMART_ITEMS_SPECIFIER : `${SMART_ITEMS_SPECIFIER}/${match[1]}`
}

/**
 * Resolves `@dcl/smart-items` imports, and the `@dcl/asset-packs` ones that map to it, from the first
 * of `resolveDirs` that reaches the package: the scene, then this package (its dependency).
 */
export function smartItemsResolver(resolveDirs: string[]): esbuild.Plugin {
  return {
    name: 'smart-items-resolver',
    setup(build) {
      build.onResolve({ filter: SMART_ITEMS_IMPORT }, async (args) => {
        if (args.pluginData === SMART_ITEMS_SPECIFIER) return undefined
        const specifier = args.path.startsWith(SMART_ITEMS_SPECIFIER) ? args.path : smartItemsSpecifier(args.path)
        if (!specifier) return undefined
        let result: esbuild.ResolveResult | undefined
        for (const resolveDir of resolveDirs) {
          result = await build.resolve(specifier, {
            kind: args.kind,
            importer: args.importer,
            resolveDir,
            pluginData: SMART_ITEMS_SPECIFIER
          })
          if (!result.errors.length) break
        }
        return result
      })
    }
  }
}

export async function bundleProject(components: BundleComponents, options: CompileOptions, sceneJson: Scene) {
  const tsconfig = path.join(options.workingDirectory, 'tsconfig.json')
  /* istanbul ignore if */
  if (!options.single && !sceneJson.main) {
    throw new CliError('BUNDLE_SCENE_MAIN_REQUIRED', i18next.t('errors.bundle.scene_main_required'))
  }

  /* istanbul ignore if */
  if ((sceneJson as any).runtimeVersion !== '7') {
    throw new CliError(
      'BUNDLE_SCENE_RUNTIME_VERSION_REQUIRED',
      i18next.t('errors.bundle.scene_runtime_version_required')
    )
  }

  /* istanbul ignore if */
  if (!(await components.fs.fileExists(tsconfig))) {
    throw new CliError('BUNDLE_TSCONFIG_REQUIRED', i18next.t('errors.bundle.tsconfig_required', { tsconfig }))
  }

  await fixTsconfigForTypescript7(components, tsconfig, options.workingDirectory)

  const entrypointSource = options.single || 'src/index.ts'
  const entrypoints = globSync(entrypointSource, { cwd: options.workingDirectory }).map((entry) =>
    path.resolve(options.workingDirectory, entry)
  )

  /* istanbul ignore if */
  if (!entrypoints.length)
    throw new CliError('BUNDLE_NO_INPUT_FILES', i18next.t('errors.bundle.no_input_files', { entrypointSource }))

  // const output = !options.single ? sceneJson.main : options.single.replace(/\.ts$/, '.js')
  // const outfile = path.join(options.workingDirectory, output)
  const inputs: { entrypoint: string; outputFile: string }[] = options.single
    ? entrypoints.map((entrypoint) => ({ entrypoint, outputFile: entrypoint.replace(/\.ts$/, '.js') }))
    : [{ entrypoint: entrypoints[0], outputFile: sceneJson.main }]

  // The project's build-time defaults: `build` and `start` write them, a deploy never edits the project
  const projectDefaults = options.projectDefaults && !options.single
  if (projectDefaults) {
    await ensureJsonKey(
      components,
      path.join(options.workingDirectory, 'package.json'),
      'scripts.server-logs',
      'sdk-commands scene-insights tail',
      'sdk-commands sdk-server-logs'
    )
  }

  for (const input of inputs) {
    await bundleSingleProject(components, {
      ...options,
      projectDefaults,
      tsconfig,
      ...input
    })
  }

  return { sceneJson, inputs }
}

type SingleProjectOptions = CompileOptions & {
  tsconfig: string
  entrypoint: string
  outputFile: string
}

/** A bundle input that belongs to `@dcl/sdk/server`: the scene uses the server APIs. */
const SDK_SERVER_INPUT = /(^|\/)@dcl\/sdk\/server\//
/** The component prefix the smart-items runtime dropped; `engine.getComponent` on it throws at run time. */
const LEGACY_COMPONENT_LITERAL = 'asset-packs::'
const SOURCE_FILE = /\.(ts|tsx|js|jsx|mjs|cjs)$/

/**
 * The scene's own source files among the bundle's inputs that still name components with the
 * `asset-packs::` prefix, as `file:line`.
 */
async function legacyComponentLiterals(
  components: BundleComponents,
  options: SingleProjectOptions,
  inputs: string[]
): Promise<string[]> {
  const found: string[] = []
  for (const input of inputs) {
    if (!SOURCE_FILE.test(input) || input.includes('node_modules/')) continue
    const file = path.resolve(options.workingDirectory, input)
    if (path.relative(options.workingDirectory, file).startsWith('..')) continue
    if (!(await components.fs.fileExists(file))) continue
    const text = await components.fs.readFile(file, 'utf-8')
    const index = text.indexOf(LEGACY_COMPONENT_LITERAL)
    if (index === -1) continue
    found.push(`${input}:${text.slice(0, index).split('\n').length}`)
  }
  return found
}

/** What a successful build tells about the scene: the legacy component names it still uses and whether it has server code. */
async function afterBuild(components: BundleComponents, options: SingleProjectOptions, result: esbuild.BuildResult) {
  const inputs = Object.keys(result.metafile?.inputs ?? {})
  try {
    const literals = await legacyComponentLiterals(components, options, inputs)
    if (literals.length) {
      printWarning(components.logger, i18next.t('warnings.bundle.asset_packs_literal', { files: literals.join(', ') }))
    }
  } catch {
    // a source file went away between the build and the scan: nothing to warn about
  }
  if (options.projectDefaults && inputs.some((input) => SDK_SERVER_INPUT.test(input))) {
    await ensureJsonKey(components, path.join(options.workingDirectory, 'scene.json'), 'authoritativeMultiplayer', true)
  }
}

export async function bundleSingleProject(components: BundleComponents, options: SingleProjectOptions) {
  printProgressStep(components.logger, `Bundling file ${colors.bold(options.entrypoint)}`, 1, MAX_STEP)

  // Pre-compute composite data so we can inject maxCompositeEntity via esbuild define.
  // This must happen before the esbuild context is created because the define values
  // are baked into the engine at compile time (the entity counter initializer reads it)
  let maxCompositeEntity = 0
  if (!options.ignoreComposite) {
    const composites = await getAllComposites(components, options.workingDirectory)
    maxCompositeEntity = composites.maxCompositeEntity
  }

  const sdkPackagePath = (() => {
    try {
      // First try to resolve from project's node_modules
      return path.dirname(require.resolve('@dcl/sdk/package.json', { paths: [options.workingDirectory] }))
    } catch {
      // Fallback to workspace @dcl/sdk
      return path.dirname(require.resolve('@dcl/sdk/package.json', { paths: [__dirname] }))
    }
  })()
  const context = await esbuild.context({
    bundle: true,
    platform: 'browser',
    format: 'cjs',
    preserveSymlinks: false,
    outfile: options.outputFile,
    allowOverwrite: false,
    sourcemap: options.production ? false : 'inline',
    minify: options.production,
    minifyIdentifiers: options.production,
    minifySyntax: options.production,
    minifyWhitespace: options.production,
    treeShaking: true,
    metafile: true,
    absWorkingDir: options.workingDirectory,
    target: 'es2020',
    external: [
      '~system/*',
      '@dcl/inspector-service',
      '@dcl/inspector-service/*' /* ban importing the inspector from the SDK */
    ],
    alias: {
      // Ensure React is always resolved to the same module to prevent duplication
      react: (() => {
        try {
          // First try to resolve from project's node_modules
          return require.resolve('react', { paths: [options.workingDirectory] })
        } catch {
          try {
            // Fallback to SDK's React dependency
            return require.resolve('react', { paths: [path.join(__dirname, '../../../@dcl/sdk')] })
          } catch {
            // Final fallback to bundled React
            return require.resolve('react')
          }
        }
      })(),
      // Ensure @dcl/sdk is always resolved to workspace version to prevent version conflicts
      '@dcl/sdk': sdkPackagePath,
      // Resolve ecs from sdk dependencies (nested in @dcl/sdk)
      '@dcl/ecs': (() => {
        try {
          // First try to resolve from project's @dcl/sdk node_modules
          return path.dirname(require.resolve('@dcl/ecs/package.json', { paths: [sdkPackagePath] }))
        } catch {
          try {
            // Fallback: try to resolve from project's node_modules
            return path.dirname(require.resolve('@dcl/ecs/package.json', { paths: [options.workingDirectory] }))
          } catch {
            // Last resort: try resolving from current directory
            return path.dirname(require.resolve('@dcl/ecs/package.json', { paths: [__dirname] }))
          }
        }
      })()
    },
    // convert filesystem paths into file:// to enable VSCode debugger
    sourceRoot: options.production ? 'dcl:///' : pathToFileURL(path.dirname(options.outputFile)).toString(),
    define: {
      document: 'undefined',
      window: 'undefined',
      DEBUG: options.production ? 'false' : 'true',
      'globalThis.DEBUG': options.production ? 'false' : 'true',
      'process.env.NODE_ENV': JSON.stringify(options.production ? 'production' : 'development'),
      DCL_MAX_COMPOSITE_ENTITY: String(maxCompositeEntity)
    },
    tsconfig: options.tsconfig,
    supported: {
      'import-assertions': false,
      'import-meta': false,
      'dynamic-import': false,
      hashbang: false
    },
    logOverride: {
      'import-is-undefined': 'silent'
    },
    plugins: [smartItemsResolver([options.workingDirectory, __dirname]), compositeLoader(components, options)],
    stdin: {
      contents: getEntrypointCode(options.entrypoint, options.customEntryPoint),
      resolveDir: path.dirname(options.entrypoint),
      sourcefile: path.basename(options.entrypoint) + '.entry-point.ts',
      loader: 'ts'
    }
  })

  /* istanbul ignore if */
  if (options.watch) {
    // Instead of using esbuild's watch, we create our own watcher
    const watcher = watch(path.resolve(options.workingDirectory), {
      ignored: ['**/node_modules/**', '**/dist/**', '**/*.crdt', '**/*.d.ts', path.resolve(options.outputFile)],
      ignoreInitial: true
    })

    const debouncedRebuild = debounce(async () => {
      try {
        const result = await context.rebuild()
        printProgressInfo(components.logger, `Bundle saved ${colors.bold(options.outputFile)}`)
        await afterBuild(components, options, result)
      } catch (err: any) {
        /* istanbul ignore next */
        components.logger.error(err.toString())
      }
    }, 100)

    watcher.on('all', async (event, filePath) => {
      // Rebuild for TypeScript, JavaScript, and composite files
      if (/\.(ts|tsx|js|jsx|composite)$/.test(filePath)) {
        printProgressInfo(components.logger, `File ${filePath} changed, rebuilding...`)
        debouncedRebuild()
      }
    })

    // Do initial build. A build error must not kill the process in watch mode:
    // the watcher above is already running and the preview server can still
    // start, so report the error and recover on the next file save — the same
    // contract as a failed re-build. Throwing here left `sdk-commands start`
    // dead when a scene was opened with a pre-existing syntax error, with no
    // watcher alive to pick up the fix.
    try {
      const result = await context.rebuild()
      printProgressInfo(components.logger, `Bundle saved ${colors.bold(options.outputFile)}`)
      await afterBuild(components, options, result)
    } catch (err: any) {
      /* istanbul ignore next */
      components.logger.error(err.toString())
    }
    printProgressInfo(components.logger, `The compiler is watching for changes`)
  } else {
    try {
      const result = await context.rebuild()
      printProgressInfo(components.logger, `Bundle saved ${colors.bold(options.outputFile)}`)
      await afterBuild(components, options, result)
    } catch (err: any) {
      /* istanbul ignore next */
      throw new CliError('BUNDLE_REBUILD_FAILED', i18next.t('errors.bundle.rebuild_failed', { error: err.toString() }))
    }
    await context.dispose()
  }

  /* istanbul ignore next */
  if (options.watch) printProgressInfo(components.logger, `The compiler is watching for changes`)

  await runTypeChecker(components, options)
}

/**
 * The type checker's script. `typescript` 7 reaches its compiler, shipped as the per-platform
 * optional dependency `@typescript/typescript-<platform>-<arch>`, through `lib/tsc.js`, and the
 * package exports no `typescript/lib/tsc` subpath: the script is reached from the package root. A
 * platform package left out of an install would otherwise surface as the script's own resolution
 * error.
 */
export function typeCheckerBin(): string {
  const packageJson = require.resolve('typescript/package.json')
  const packageDir = path.dirname(packageJson)
  const version: string = JSON.parse(fs.readFileSync(packageJson, 'utf-8')).version
  /* istanbul ignore else */
  if (parseInt(version, 10) >= 7) {
    const platformPackage = `@typescript/typescript-${process.platform}-${process.arch}`
    try {
      require.resolve(`${platformPackage}/package.json`, { paths: [packageDir] })
    } catch {
      /* istanbul ignore next */
      throw new CliError(
        'BUNDLE_TYPESCRIPT_PLATFORM_PACKAGE_MISSING',
        i18next.t('errors.bundle.typescript_platform_package_missing', { version, platformPackage, packageDir })
      )
    }
  }
  return path.join(packageDir, 'lib', 'tsc.js')
}

function runTypeChecker(components: BundleComponents, options: CompileOptions) {
  const tsBin = typeCheckerBin()
  const args = [
    '-p',
    'tsconfig.json',
    '--preserveWatchOutput',
    options.emitDeclaration ? '--emitDeclarationOnly' : '--noEmit'
  ]

  /* istanbul ignore if */
  if (options.watch) args.push('--watch')

  printProgressStep(components.logger, `Running type checker`, 2, MAX_STEP)
  const ts = child_process.fork(tsBin, args, {
    stdio: 'pipe',
    env: process.env,
    cwd: options.workingDirectory
  })
  const typeCheckerFuture = future<number>()

  let stdOutput = ''
  ts.stdout?.on('data', (data: string) => {
    stdOutput = (stdOutput + data.toString()).slice(-MAX_OUTPUT_SIZE)
  })

  ts.stdout?.pipe(process.stdout)
  ts.stderr?.pipe(process.stderr)

  const cleanup = () => {
    if (!ts.killed) ts.kill('SIGTERM')
  }

  process.on('SIGTERM', cleanup)
  process.on('SIGINT', cleanup)
  process.on('exit', cleanup)

  ts.on('close', (code) => {
    // Remove listeners after process exits to prevent memory leaks
    process.off('SIGTERM', cleanup)
    process.off('SIGINT', cleanup)
    process.off('exit', cleanup)

    /* istanbul ignore else */
    if (code === 0) {
      printProgressInfo(components.logger, `Type checking completed without errors`)
      typeCheckerFuture.resolve(code)
    } else {
      typeCheckerFuture.reject(
        new CliError(
          'BUNDLE_TYPE_CHECKER_FAILED',
          `${stdOutput.replace(/\x1b\[[0-9;]*m/g, '')}\n
          ${i18next.t('errors.bundle.type_checker_failed', { code })}`
        )
      )
    }
    stdOutput = ''
  })

  /* istanbul ignore if */
  if (options.watch) {
    typeCheckerFuture.resolve(-1)
  }

  return typeCheckerFuture
}

export function compositeLoader(components: BundleComponents, options: SingleProjectOptions): esbuild.Plugin {
  let shouldReload = true
  let compositeData: Awaited<ReturnType<typeof getAllComposites>> | null = null

  return {
    name: 'composite-loader',
    setup(build) {
      build.onStart(() => {
        shouldReload = true
        compositeData = null
      })

      // Handle composites virtual module
      build.onResolve({ filter: /~sdk\/all-composites/ }, (_args) => {
        return {
          namespace: 'sdk-composite',
          path: 'all-composites'
        }
      })

      build.onLoad({ filter: /.*/, namespace: 'sdk-composite' }, async (_) => {
        // Load compositeData if not already loaded
        if (shouldReload && !compositeData) {
          if (!options.ignoreComposite) {
            compositeData = await getAllComposites(
              components,
              // we pass the build.initialOptions.absWorkingDir to build projects with multiple roots at once
              build.initialOptions.absWorkingDir ?? options.workingDirectory
            )

            if (compositeData.withErrors) {
              printWarning(
                components.logger,
                'Some composites are not included because of errors while compiling them. There can be unexpected behavior in the scene, check the errors and try to fix them.'
              )
            }
          }
          shouldReload = false
        }

        const contents = compositeData
          ? `export const compositeFromLoader = {${compositeData.compositeLines.join(',')}}`
          : `export const compositeFromLoader = {}`
        const watchFiles = compositeData?.watchFiles || []

        return {
          loader: 'js',
          contents,
          watchFiles
        }
      })

      // Handle scripts virtual module
      build.onResolve({ filter: /~sdk\/script-utils/ }, (_args) => {
        return {
          namespace: 'sdk-scripts',
          path: 'script-utils'
        }
      })

      build.onLoad({ filter: /.*/, namespace: 'sdk-scripts' }, async (_) => {
        // ensure compositeData is loaded (in case scripts are imported before composites)
        if (!compositeData && !options.ignoreComposite) {
          compositeData = await getAllComposites(
            components,
            build.initialOptions.absWorkingDir ?? options.workingDirectory
          )
        }

        const { contents, watchFiles } = await generateInitializeScriptsModule(
          components,
          options.workingDirectory,
          compositeData,
          { editorScene: await isEditorScene(components, options.workingDirectory) }
        )

        return {
          loader: 'ts',
          contents,
          watchFiles,
          resolveDir: options.workingDirectory
        }
      })
    }
  }
}

/**
 * Generates a sanitized import name for a script path.
 * Example: "src/scripts/my-script.ts" -> "script_src_scripts_my_script"
 */
export function getScriptImportName(scriptPath: string): string {
  return (
    'script_' +
    scriptPath
      .replace(/\.tsx?$/, '') // remove .ts or .tsx
      .replace(/[^a-zA-Z0-9]/g, '_') // sanitize
  )
}

interface ScriptCollectionResult {
  runtimeImports: string
  typeImports: string
  scriptTypes: string
  scriptsArray: string
  watchFiles: string[]
}

/**
 * Collects and formats all script data for code generation
 */
function collectScriptData(
  compositeData: { scripts: Map<string, Script[]>; [key: string]: any },
  workingDirectory: string
): ScriptCollectionResult {
  const runtimeImports: string[] = []
  const typeImports: string[] = []
  const scriptTypes: string[] = []
  const scriptsArray: string[] = []
  const watchFiles: string[] = []

  for (const [scriptPath, scriptInstances] of compositeData.scripts.entries()) {
    const importName = getScriptImportName(scriptPath)
    const absolutePath = path.join(workingDirectory, scriptPath)

    // For the virtual module runtime
    runtimeImports.push(`import * as ${importName} from ${toSafeModulePath('./' + scriptPath)}`)

    // For .d.ts file using import() type syntax (works in ambient modules)
    typeImports.push(`  type ${importName} = typeof import(${toSafeModulePath(absolutePath)})`)

    // Add to ScriptRegistry interface
    scriptTypes.push(`  ${toSafeModulePath(scriptPath)}: ExtractScriptType<${importName}>`)

    // Add each script instance to the array
    for (const script of scriptInstances) {
      scriptsArray.push(`  { ...${JSON.stringify(script)}, module: ${importName} }`)
    }

    watchFiles.push(absolutePath)
  }

  return {
    runtimeImports: runtimeImports.join('\n'),
    typeImports: typeImports.join('\n'),
    scriptTypes: `interface ScriptRegistry {\n${scriptTypes.join(',\n')}\n}`,
    scriptsArray: `[\n${scriptsArray.join(',\n')}\n]`,
    watchFiles
  }
}

/**
 * Generates the TypeScript declaration for the ~sdk/script-utils module
 */
async function generateScriptModuleDeclaration(
  components: BundleComponents,
  typeImports: string,
  scriptTypes: string
): Promise<string> {
  const templatePath = path.join(__dirname, 'script-module.d.ts.template')
  const template = await components.fs.readFile(templatePath, 'utf-8')

  return template.replace('__TYPE_IMPORTS__', typeImports).replace('__SCRIPT_TYPES__', scriptTypes)
}

/**
 * Updates the sdk.d.ts file with script type declarations
 */
async function updateSdkTypeDeclarations(
  components: BundleComponents,
  workingDirectory: string,
  typeImports: string,
  scriptTypes: string,
  scriptCount: number
): Promise<void> {
  try {
    const jsRuntimePath = require.resolve('@dcl/js-runtime/sdk.d.ts', { paths: [workingDirectory] })
    let existingSdkDts = await components.fs.readFile(jsRuntimePath, 'utf-8')

    // remove any existing ~sdk/script-utils module declaration
    existingSdkDts = existingSdkDts
      .replace(/\/\/ @internal[\s]*\ndeclare module '~sdk\/script-utils'[\s\S]*?(?=\n\/\/|$)/, '')
      .trim()

    // generate and append the new module declaration
    const scriptModuleDeclaration = await generateScriptModuleDeclaration(components, typeImports, scriptTypes)
    const updatedSdkDts = existingSdkDts + '\n' + scriptModuleDeclaration

    await components.fs.writeFile(jsRuntimePath, updatedSdkDts)
    components.logger.info(`✓ Updated sdk.d.ts with ${scriptCount} script type(s) at: ${jsRuntimePath}`)
  } catch (err: any) {
    components.logger.error('⚠ Could not update sdk.d.ts with script types:', err)
  }
}

/**
 * Minimal '~sdk/script-utils' for scenes with no scripts: same export surface,
 * empty-registry semantics, no embedded script runtime (skips the
 * @dcl/smart-items payload and its glue).
 */
function generateScriptStubModuleContent(): string {
  return `
${SMART_ITEMS_NOOP}
export function _initializeScripts(_engine) {}

export function getScriptInstance(_entity, _scriptPath) {
  return null
}

export function getScriptInstancesByPath(_scriptPath) {
  return []
}

export function getAllScriptInstances(_entity) {
  return []
}

export function callScriptMethod(entity, scriptPath, methodName, ..._args) {
  console.error(\`Method \${methodName} not found on script \${scriptPath} for entity \${entity}\`)
  return undefined
}
`
}

const SMART_ITEMS_BOOT = `
import { engine } from '@dcl/sdk/ecs'
import { syncEntity } from '@dcl/sdk/network'
import players from '@dcl/sdk/players'
import { initSmartItems } from '@dcl/smart-items'
export function _initializeSmartItems() {
  initSmartItems(engine, { syncEntity }, players)
}
`

const SMART_ITEMS_NOOP = 'export function _initializeSmartItems() {}'

/**
 * Generates the virtual module content with script initialization and helper functions
 */
function generateVirtualModuleContent(runtimeImports: string, scriptsArray: string, editorScene: boolean): string {
  return `
${editorScene ? SMART_ITEMS_BOOT : SMART_ITEMS_NOOP}
import { runScripts } from '@dcl/smart-items/scripts'
export { getScriptInstance, getScriptInstancesByPath, getAllScriptInstances, callScriptMethod } from '@dcl/smart-items/scripts'
${runtimeImports}

export function _initializeScripts(engine) {
  const scriptsArray = ${scriptsArray}
  return runScripts(engine, scriptsArray)
}
`
}

export async function generateInitializeScriptsModule(
  components: BundleComponents,
  workingDirectory: string,
  compositeData: { scripts: Map<string, Script[]>; [key: string]: any } | null,
  opts?: { editorScene?: boolean }
): Promise<{ contents: string; watchFiles: string[] }> {
  const hasScripts = !!compositeData && compositeData.scripts.size > 0

  // scriptless non-editor scenes skip the embedded script runtime entirely
  if (!hasScripts && !opts?.editorScene) {
    return {
      contents: generateScriptStubModuleContent(),
      watchFiles: []
    }
  }

  // default empty implementation if no scripts
  if (!hasScripts) {
    return {
      contents: generateVirtualModuleContent('', '[]', !!opts?.editorScene),
      watchFiles: []
    }
  }

  // Step 1: Collect all script data
  const scriptData = collectScriptData(compositeData, workingDirectory)

  // Step 2: Update TypeScript declarations
  await updateSdkTypeDeclarations(
    components,
    workingDirectory,
    scriptData.typeImports,
    scriptData.scriptTypes,
    compositeData.scripts.size
  )

  // Step 3: Generate virtual module content
  const contents = generateVirtualModuleContent(scriptData.runtimeImports, scriptData.scriptsArray, !!opts?.editorScene)

  return { contents, watchFiles: scriptData.watchFiles }
}
