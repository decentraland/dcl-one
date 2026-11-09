import { Scene } from '@dcl/schemas'
import path from 'path'
import i18next from 'i18next'

import { CliComponents } from '../components'
import { colors } from '../components/log'
import { printProgressInfo, printWarning } from './beautiful-logs'
import { CliError } from './error'
import { getSceneFilePath, getValidSceneJson } from './scene-validations'
import { getInstalledPackageVersion, getSdkCommandsVersion, isCI, isEditor } from './config'
import { readStringConfig } from '../components/config'
import { getSmartWearableFile, getValidWearableJson } from './portable-experience-sw-validations'
import { getPackageJson } from './project-files'
import { SCRIPT_COMPONENT, isEditorComponent, renameLegacyComponentName } from './smart-items'

export type BaseProject = { workingDirectory: string }
export type SceneProject = { kind: 'scene'; scene: Scene } & BaseProject
export type WearableProject = { kind: 'smart-wearable'; scene: Scene } & BaseProject
export type ProjectUnion = SceneProject | WearableProject

/**
 * Asserts that the workingDirectory is a valid project
 */
export async function assertValidProjectFolder(
  components: Pick<CliComponents, 'fs' | 'logger'>,
  workingDirectory: string
): Promise<ProjectUnion> {
  // no validations for now, only check that it exists
  if (!(await components.fs.fileExists(path.resolve(workingDirectory, 'package.json'))))
    throw new CliError(
      'PROJECT_VALIDATIONS_PACKAGE_JSON_REQUIRED',
      i18next.t('errors.project_validations.package_json_required')
    )

  // now we will iterate over different file to evaluate the project kind
  switch (true) {
    case await components.fs.fileExists(getSmartWearableFile(workingDirectory)): {
      await getValidWearableJson(components, workingDirectory)
      return { kind: 'smart-wearable', scene: await getValidSceneJson(components, workingDirectory), workingDirectory }
    }

    case await components.fs.fileExists(getSceneFilePath(workingDirectory)): {
      return { kind: 'scene', scene: await getValidSceneJson(components, workingDirectory), workingDirectory }
    }

    default: {
      throw new CliError(
        'PROJECT_VALIDATIONS_INVALID_PROJECT_KIND',
        i18next.t('errors.project_validations.invalid_project_kind', { workingDirectory })
      )
    }
  }
}

/*
 * Returns true if the project contains an empty node_modules folder
 */
export async function needsDependencies(
  components: Pick<CliComponents, 'fs'>,
  workingDirectory: string
): Promise<boolean> {
  const nodeModulesPath = path.join(workingDirectory, 'node_modules')
  const hasNodeModulesFolder = await components.fs.directoryExists(nodeModulesPath)
  const isNodeModulesEmpty = hasNodeModulesFolder && (await components.fs.readdir(nodeModulesPath)).length === 0

  return !hasNodeModulesFolder || isNodeModulesEmpty
}

/* istanbul ignore next */
const npmBin = /^win/.test(process.platform) ? 'npm.cmd' : 'npm'

export type PackageManager = 'npm' | 'pnpm'

const LOCKFILES: [string, PackageManager][] = [
  ['pnpm-lock.yaml', 'pnpm'],
  ['pnpm-workspace.yaml', 'pnpm'],
  ['package-lock.json', 'npm']
]

/*
 * The package manager of the project's own lockfile, or of the workspace that lists the project as a
 * member (since a member keeps its lockfile at the workspace root); undefined when there is neither
 * between the project and its repository root. A project merely nested inside another one, such as a
 * scene scaffolded within an unrelated pnpm checkout, inherits nothing from it.
 */
export async function findPackageManager(
  components: Pick<CliComponents, 'fs'>,
  workingDirectory: string
): Promise<PackageManager | undefined> {
  const project = path.resolve(workingDirectory)
  for (const [lockfile, manager] of LOCKFILES) {
    if (await components.fs.fileExists(path.join(project, lockfile))) return manager
  }
  let dir = project
  while (!(await components.fs.fileExists(path.join(dir, '.git')))) {
    const parent = path.dirname(dir)
    if (parent === dir) return undefined
    dir = parent
    const manager = await workspaceManager(components, dir, path.relative(dir, project).split(path.sep).join('/'))
    if (manager) return manager
  }
  return undefined
}

/*
 * The manager of the workspace rooted at `root` when `member` (relative to it) is one of its packages:
 * pnpm reads them from pnpm-workspace.yaml (every directory when `packages` is absent), npm from the
 * `workspaces` of package.json
 */
async function workspaceManager(
  components: Pick<CliComponents, 'fs'>,
  root: string,
  member: string
): Promise<PackageManager | undefined> {
  const pnpmWorkspace = path.join(root, 'pnpm-workspace.yaml')
  if (await components.fs.fileExists(pnpmWorkspace)) {
    const packages = pnpmWorkspacePackages(await components.fs.readFile(pnpmWorkspace, 'utf8'))
    if (matchesWorkspace(packages ?? ['**'], member)) return 'pnpm'
  }
  const packageJson = path.join(root, 'package.json')
  if (await components.fs.fileExists(packageJson)) {
    try {
      const workspaces = JSON.parse(await components.fs.readFile(packageJson, 'utf8')).workspaces
      const packages = Array.isArray(workspaces) ? workspaces : workspaces?.packages
      if (Array.isArray(packages) && matchesWorkspace(packages, member)) return 'npm'
    } catch {
      // not a package.json; the directory is no workspace root
    }
  }
  return undefined
}

/*
 * The `packages` of a pnpm-workspace.yaml, a flow or block sequence of plain or quoted globs; undefined
 * when the key is absent
 */
function pnpmWorkspacePackages(yaml: string): string[] | undefined {
  const unquote = (item: string) =>
    item
      .replace(/\s+#.*$/, '')
      .trim()
      .replace(/^(['"])(.*)\1$/, '$2')
  const lines = yaml.split(/\r?\n/)
  const start = lines.findIndex((line) => /^packages:/.test(line))
  if (start === -1) return undefined
  const inline = lines[start].slice('packages:'.length).trim()
  if (inline.startsWith('['))
    return inline
      .replace(/^\[|\].*$/g, '')
      .split(',')
      .map(unquote)
      .filter(Boolean)
  const items: string[] = []
  for (const line of lines.slice(start + 1)) {
    const item = /^\s+-\s+(.*)$/.exec(line)
    if (item) items.push(unquote(item[1]))
    else if (line.trim() && !line.trim().startsWith('#')) break
  }
  return items
}

/*
 * Whether the workspace globs name the directory: some pattern matches it and no negated one does
 */
function matchesWorkspace(patterns: string[], dir: string): boolean {
  const matches = (pattern: string) => globToRegExp(pattern).test(dir)
  return (
    patterns.some((pattern) => !pattern.startsWith('!') && matches(pattern)) &&
    !patterns.some((pattern) => pattern.startsWith('!') && matches(pattern.slice(1)))
  )
}

function globToRegExp(glob: string): RegExp {
  const segments = glob.replace(/^\.\//, '').replace(/\/+$/, '').split('/')
  const source = segments
    .map((segment, i) => {
      const last = i === segments.length - 1
      if (segment === '**') return last ? '.+' : '(?:.+/)?'
      const literal = segment
        .replace(/[.+^${}()|[\]\\]/g, '\\$&')
        .replace(/\*/g, '[^/]*')
        .replace(/\?/g, '[^/]')
      return last ? literal : `${literal}/`
    })
    .join('')
  return new RegExp(`^${source}$`)
}

/*
 * The package manager that installs the project: pnpm when its lockfile is pnpm's, npm otherwise
 */
export async function getPackageManager(
  components: Pick<CliComponents, 'fs'>,
  workingDirectory: string
): Promise<PackageManager> {
  return (await findPackageManager(components, workingDirectory)) ?? 'npm'
}

/*
 * Runs "npm install" (or "pnpm install") for desired project
 */
export async function installDependencies(
  components: Pick<CliComponents, 'logger' | 'spawner' | 'fs'>,
  workingDirectory: string
): Promise<void> {
  printProgressInfo(components.logger, 'Installing dependencies...')
  const manager = await getPackageManager(components, workingDirectory)
  if (manager === 'pnpm') await assertPnpmInstalled(components, workingDirectory)
  // TODO: test in windows
  // The shell resolves the bare name to whichever shim pnpm installed (pnpm.cmd, or pnpm.exe from @pnpm/exe).
  await components.spawner.exec(workingDirectory, manager === 'npm' ? npmBin : 'pnpm', ['install'])
  printProgressInfo(components.logger, colors.white('✅ Installing dependencies...'))
}

/*
 * pnpm does not come with Node: a missing one is reported as such, not as the shell's exit code
 */
async function assertPnpmInstalled(components: Pick<CliComponents, 'spawner'>, workingDirectory: string) {
  try {
    await components.spawner.exec(workingDirectory, 'pnpm', ['--version'], { silent: true })
  } catch {
    throw new CliError('INSTALL_PNPM_NOT_FOUND', i18next.t('errors.install.pnpm_not_found', { workingDirectory }))
  }
}

/*
 * Suggests pnpm to a project installed with npm, unless the run cannot act on it: CI, the Creator Hub
 * (which installs with its own npm) or a DCL_DISABLE_PNPM_HINT=true opt-out
 */
export async function printPackageManagerHint(
  components: Pick<CliComponents, 'logger' | 'fs' | 'config'>,
  workingDirectory: string
): Promise<void> {
  if (isCI() || isEditor()) return
  if ((await readStringConfig(components, 'DCL_DISABLE_PNPM_HINT')) === 'true') return
  if ((await findPackageManager(components, workingDirectory)) !== 'npm') return
  const version = await getSdkCommandsVersion()
  printWarning(components.logger, i18next.t('warnings.install.use_pnpm', { workingDirectory, version }))
}

/*
 * Warns when the installed @dcl/js-runtime is not the installed @dcl/sdk's version: each pins its own
 * @dcl/ecs, so a second copy appears and the ~system module types disagree with the scene's
 */
export async function printRuntimeVersionWarning(
  components: Pick<CliComponents, 'logger' | 'fs'>,
  workingDirectory: string
): Promise<void> {
  const sdk = await getInstalledPackageVersion(components, '@dcl/sdk', workingDirectory)
  const runtime = await getInstalledPackageVersion(components, '@dcl/js-runtime', workingDirectory)
  if (sdk === 'unknown' || runtime === 'unknown' || sdk === runtime) return
  printWarning(components.logger, i18next.t('warnings.build.js_runtime_version', { runtime, sdk }))
}

/**
 * Run NPM commands
 */
export async function npmRun(
  components: Pick<CliComponents, 'spawner'>,
  cwd: string,
  command: string,
  ...args: string[]
): Promise<void> {
  // TODO: test in windows
  await components.spawner.exec(cwd, npmBin, ['run', command, '--silent', '--', ...args], { env: process.env as any })
}

/**
 * NPM commands
 */
export async function npmCommand(
  components: Pick<CliComponents, 'spawner'>,
  cwd: string,
  command: string,
  ...args: string[]
): Promise<void> {
  await components.spawner.exec(cwd, npmBin, [command, ...args, '--silent'], { env: process.env as any })
}

/**
 * Start validations to make the scene work.
 * Addd js-runtime package, so you can auto-import all the ~system modules.
 */
export async function startValidations(components: Pick<CliComponents, 'spawner' | 'fs' | 'logger'>, cwd: string) {
  try {
    const sdkVersion = await getInstalledPackageVersion(components, '@dcl/sdk', cwd)
    const packageJsonPath = path.resolve(cwd, 'package.json')

    if (!(await components.fs.fileExists(packageJsonPath))) {
      return
    }
    const packageJson = await getPackageJson(components, cwd)

    // Ignore relative or s3 versions
    const sdkPackageVersion =
      (packageJson.dependencies && packageJson.dependencies['@dcl/sdk']) ||
      (packageJson.devDependencies && packageJson.devDependencies['@dcl/sdk'])
    if (
      sdkPackageVersion?.startsWith('https://') ||
      sdkPackageVersion?.startsWith('file://') ||
      sdkPackageVersion === '7.0.0'
    ) {
      return
    }

    if (
      packageJson.dependencies &&
      (packageJson.dependencies['@dcl/js-runtime'] || packageJson.dependencies['@dcl/sdk'])
    ) {
      packageJson.dependencies['@dcl/js-runtime'] = sdkVersion
      if (packageJson.devDependencies && packageJson.devDependencies['@dcl/js-runtime']) {
        delete packageJson.devDependencies['@dcl/js-runtime']
      }
    } else {
      packageJson.devDependencies = packageJson.devDependencies || {}
      packageJson.devDependencies['@dcl/js-runtime'] = sdkVersion
    }
    await components.fs.writeFile(packageJsonPath, JSON.stringify(packageJson, null, 2), 'utf-8')
  } catch (e: any) {
    components.logger.error('Failed to run scene validations', e.message)
  }
}

/**
 * Returns true if the scene is an "editor scene" that requires the smart-item runtime.
 *
 * A scene is an editor scene when its main.composite file contains at least one
 * smart-item component (under `smart-items::` or the legacy `asset-packs::` prefix)
 * other than the Script component. The Script component is a build-time-only construct
 * (it defines JavaScript scripts bundled separately via `generateInitializeScriptsModule`)
 * and does not require the `initSmartItems` runtime.
 * Only Action/Trigger/State and similar runtime components need `initSmartItems`.
 *
 * Evaluated on every (re)build when `~sdk/script-utils` is generated.
 */
export async function isEditorScene(components: Pick<CliComponents, 'fs'>, workingDirectory: string): Promise<boolean> {
  const mainCompositePath = path.resolve(workingDirectory, 'assets', 'scene', 'main.composite')
  if (!(await components.fs.fileExists(mainCompositePath))) return false

  try {
    const fileBuffer = await components.fs.readFile(mainCompositePath)
    const json = JSON.parse(new TextDecoder().decode(fileBuffer))
    // the Script component is build-time only — it doesn't require the initSmartItems runtime entrypoint.
    return (
      Array.isArray(json.components) &&
      json.components.some((c: unknown) => {
        const name = (c as Record<string, unknown>).name
        return (
          typeof name === 'string' && isEditorComponent(name) && renameLegacyComponentName(name) !== SCRIPT_COMPONENT
        )
      })
    )
  } catch {
    // Malformed composite — treat as non-editor scene to avoid bundling unnecessary code
    return false
  }
}
