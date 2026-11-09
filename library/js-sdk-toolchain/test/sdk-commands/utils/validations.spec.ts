import * as projectValidation from '../../../packages/@dcl/sdk-commands/src/logic/project-validations'
import * as sceneValidation from '../../../packages/@dcl/sdk-commands/src/logic/scene-validations'
import { Scene } from '../../../packages/@dcl/sdk-commands/node_modules/@dcl/schemas/dist'
import { initComponents } from '../../../packages/@dcl/sdk-commands/src/components'
import path from 'path'
import { initLanguage, Language } from '../../../packages/@dcl/sdk-commands/src/logic/lang'
import * as config from '../../../packages/@dcl/sdk-commands/src/logic/config'

beforeAll(async () => {
  await initLanguage(Language.EN)
})

afterEach(() => {
  jest.clearAllMocks()
  jest.restoreAllMocks()
})

describe('build:helpers', () => {
  it('assertValidProjectFolder: validate e2e with virtual file system', async () => {
    const components = await initComponents()
    const scene: Scene = {
      main: 'test.js',
      scene: {
        base: '0,0',
        parcels: ['0,0']
      }
    }

    const fs = {
      [path.resolve('some/path/package.json')]: '{}',
      [path.resolve('some/path/scene.json')]: JSON.stringify(scene)
    }

    const fileExists = jest.spyOn(components.fs, 'fileExists').mockImplementation(async (_file) => {
      return _file in fs
    })
    const readFile = jest.spyOn(components.fs, 'readFile').mockImplementation(async (_file) => {
      return fs[_file as any]
    })
    jest.spyOn(projectValidation, 'assertValidProjectFolder')

    const res = await projectValidation.assertValidProjectFolder(components, 'some/path')

    expect(res).toEqual({ kind: 'scene', scene, workingDirectory: 'some/path' })
    expect(fileExists).toHaveBeenCalledWith(path.resolve('some/path/package.json'))
    expect(readFile).toHaveBeenCalledWith(path.resolve('some/path/scene.json'), 'utf8')
  })

  it('assertValidProjectFolder: should fail on unrecognized file', async () => {
    const components = await initComponents()
    const fileExists = jest.spyOn(components.fs, 'fileExists').mockImplementation(async (_file) => {
      return _file.endsWith('package.json')
    })
    jest.spyOn(projectValidation, 'assertValidProjectFolder')
    jest.spyOn(sceneValidation, 'assertValidScene')

    await expect(() => projectValidation.assertValidProjectFolder(components, 'some/path')).rejects.toThrow()

    expect(fileExists).toHaveBeenCalledWith(path.resolve('some/path/package.json'))
  })

  it("assertValidProjectFolder: should throw if package.json doesn't exist", async () => {
    const components = await initComponents()
    const fileExists = jest.spyOn(components.fs, 'fileExists').mockResolvedValue(false)
    jest.spyOn(projectValidation, 'assertValidProjectFolder')

    await expect(() => projectValidation.assertValidProjectFolder(components, 'some/path')).rejects.toThrow()

    expect(fileExists).toHaveBeenCalledWith(path.resolve('some/path/package.json'))
  })

  it('needsDependencies: should return true if "node_modules" does not exist', async () => {
    const components = await initComponents()
    jest.spyOn(components.fs, 'directoryExists').mockResolvedValue(false)

    const res = await projectValidation.needsDependencies(components, 'some/path')

    expect(res).toBe(true)
  })

  it('needsDependencies: should return true if "node_modules" is empty', async () => {
    const components = await initComponents()
    jest.spyOn(components.fs, 'directoryExists').mockResolvedValue(true)
    jest.spyOn(components.fs, 'readdir').mockResolvedValue([])

    const res = await projectValidation.needsDependencies(components, 'some/path')

    expect(res).toBe(true)
  })

  it('needsDependencies: should return false if "node_modules" is valid', async () => {
    const components = await initComponents()
    jest.spyOn(components.fs, 'directoryExists').mockResolvedValue(true)
    jest.spyOn(components.fs, 'readdir').mockResolvedValue(['some', 'files'])

    const res = await projectValidation.needsDependencies(components, 'some/path')

    expect(res).toBe(false)
  })

  it('installDependencies: should run dependencies installation', async () => {
    const components = await initComponents()
    const execSpy = jest.spyOn(components.spawner, 'exec').mockResolvedValue()
    jest.spyOn(components.fs, 'fileExists').mockResolvedValue(false)

    await projectValidation.installDependencies(components, 'some/path')

    expect(execSpy).toHaveBeenCalledWith('some/path', 'npm', ['install'])
  })

  it('installDependencies: should run pnpm when the project has a pnpm lockfile', async () => {
    const components = await initComponents()
    const execSpy = jest.spyOn(components.spawner, 'exec').mockResolvedValue()
    jest.spyOn(components.fs, 'fileExists').mockImplementation(async (file) => file.endsWith('pnpm-lock.yaml'))

    await projectValidation.installDependencies(components, 'some/path')

    expect(execSpy).toHaveBeenCalledWith('some/path', 'pnpm', ['install'])
  })

  it('installDependencies: checks that pnpm is installed before running it', async () => {
    const components = await initComponents()
    const execSpy = jest.spyOn(components.spawner, 'exec').mockResolvedValue()
    jest.spyOn(components.fs, 'fileExists').mockImplementation(async (file) => file.endsWith('pnpm-lock.yaml'))

    await projectValidation.installDependencies(components, 'some/path')
    expect(execSpy.mock.calls).toStrictEqual([
      ['some/path', 'pnpm', ['--version'], { silent: true }],
      ['some/path', 'pnpm', ['install']]
    ])

    execSpy.mockClear()
    execSpy.mockRejectedValue(new Error('Command "pnpm --version" exited with code 127'))
    const error = await projectValidation.installDependencies(components, 'some/path').then(
      () => undefined,
      (e) => e
    )
    expect(error.name).toBe('INSTALL_PNPM_NOT_FOUND')
    expect(error.message).toContain('pnpm is not installed, and some/path installs with it')
    expect(execSpy).toHaveBeenCalledTimes(1)
  })

  it("getPackageManager: the project's lockfile, or the workspace that lists it, up to the repository root", async () => {
    const components = await initComponents()
    const files = new Map<string, string>()
    jest.spyOn(components.fs, 'fileExists').mockImplementation(async (file) => files.has(file))
    jest.spyOn(components.fs, 'readFile').mockImplementation((async (file: string) => files.get(file)) as any)
    const repo = path.resolve('repo')
    const scene = path.join(repo, 'packages', 'scene')
    const write = (dir: string, name: string, text = '') => files.set(path.join(dir, name), text)
    const manager = () => projectValidation.findPackageManager(components, scene)

    expect(await manager()).toBeUndefined()
    write(path.dirname(repo), 'pnpm-workspace.yaml', 'packages:\n  - "**"\n') // above the repository: not this project's
    write(repo, '.git')
    write(repo, 'pnpm-lock.yaml') // a single-package project the scene sits in: not its workspace
    expect(await manager()).toBeUndefined()
    expect(await projectValidation.getPackageManager(components, scene)).toBe('npm')
    write(repo, 'pnpm-workspace.yaml', 'packages:\n  - apps/*\n  - "packages/*" # members\n  - "!packages/scene"\n')
    expect(await manager()).toBeUndefined()
    write(repo, 'pnpm-workspace.yaml', 'packages:\n  - apps/*\n  - "packages/*" # members\n')
    expect(await manager()).toBe('pnpm')
    write(repo, 'pnpm-workspace.yaml', 'packages: [apps/*, "packages/**"]\n')
    expect(await manager()).toBe('pnpm')
    write(repo, 'pnpm-workspace.yaml', '# every directory\n')
    expect(await manager()).toBe('pnpm')
    files.delete(path.join(repo, 'pnpm-workspace.yaml'))
    write(repo, 'package.json', JSON.stringify({ workspaces: ['packages/*'] }))
    expect(await manager()).toBe('npm')
    write(repo, 'package.json', JSON.stringify({ workspaces: { packages: ['apps/*'] } }))
    expect(await manager()).toBeUndefined()
    write(scene, 'pnpm-lock.yaml')
    expect(await manager()).toBe('pnpm')
  })

  describe('printPackageManagerHint', () => {
    const env = { CI: process.env.CI, EDITOR_SDK7: process.env.EDITOR_SDK7 }
    beforeEach(() => {
      delete process.env.CI
      delete process.env.EDITOR_SDK7
    })
    afterEach(() => {
      for (const [key, value] of Object.entries(env)) {
        if (value === undefined) delete process.env[key]
        else process.env[key] = value
      }
    })

    it('printPackageManagerHint: should suggest pnpm only to a project installed with npm', async () => {
      const components = await initComponents()
      const logSpy = jest.spyOn(components.logger, 'log').mockImplementation(() => {})
      const lockfiles = new Set<string>()
      jest.spyOn(components.fs, 'fileExists').mockImplementation(async (file) => lockfiles.has(path.basename(file)))

      await projectValidation.printPackageManagerHint(components, 'some/path')
      lockfiles.add('package-lock.json')
      await projectValidation.printPackageManagerHint(components, 'some/path')
      lockfiles.add('pnpm-lock.yaml')
      await projectValidation.printPackageManagerHint(components, 'some/path')

      expect(logSpy).toHaveBeenCalledTimes(1)
      expect(logSpy.mock.calls[0][0]).toContain('some/path was installed with npm')
      expect(logSpy.mock.calls[0][0]).toContain('pnpm add -D @dcl/sdk-commands@')
    })

    it('stays silent where nobody can act on it: CI, the Creator Hub, or when opted out', async () => {
      const components = await initComponents()
      const logSpy = jest.spyOn(components.logger, 'log').mockImplementation(() => {})
      jest
        .spyOn(components.fs, 'fileExists')
        .mockImplementation(async (file) => path.basename(file) === 'package-lock.json')
      const getString = jest.spyOn(components.config, 'getString').mockResolvedValue(undefined)

      process.env.CI = 'true'
      await projectValidation.printPackageManagerHint(components, 'some/path')
      delete process.env.CI
      process.env.EDITOR_SDK7 = 'true'
      await projectValidation.printPackageManagerHint(components, 'some/path')
      delete process.env.EDITOR_SDK7
      getString.mockImplementation(async (key) => (key === 'DCL_DISABLE_PNPM_HINT' ? 'true' : undefined))
      await projectValidation.printPackageManagerHint(components, 'some/path')
      expect(logSpy).not.toHaveBeenCalled()

      getString.mockResolvedValue(undefined)
      await projectValidation.printPackageManagerHint(components, 'some/path')
      expect(logSpy).toHaveBeenCalledTimes(1)
    })
  })

  it('printRuntimeVersionWarning: warns only when @dcl/js-runtime is not the installed @dcl/sdk version', async () => {
    const components = await initComponents()
    const logSpy = jest.spyOn(components.logger, 'log').mockImplementation(() => {})
    const versions: Record<string, string> = { '@dcl/sdk': '7.33.0', '@dcl/js-runtime': '7.33.0' }
    jest.spyOn(config, 'getInstalledPackageVersion').mockImplementation(async (_, name) => versions[name])

    await projectValidation.printRuntimeVersionWarning(components, 'some/path')
    versions['@dcl/js-runtime'] = 'unknown'
    await projectValidation.printRuntimeVersionWarning(components, 'some/path')
    expect(logSpy).not.toHaveBeenCalled()

    versions['@dcl/js-runtime'] = '7.30.0'
    await projectValidation.printRuntimeVersionWarning(components, 'some/path')
    expect(logSpy).toHaveBeenCalledTimes(1)
    expect(logSpy.mock.calls[0][0]).toContain('@dcl/js-runtime 7.30.0 is installed next to @dcl/sdk 7.33.0')
    expect(logSpy.mock.calls[0][0]).toContain('install @dcl/js-runtime@7.33.0')
  })

  it('npmRun: should build pass on the process.env', async () => {
    const components = await initComponents()
    const execSpy = jest.spyOn(components.spawner, 'exec').mockResolvedValue()

    await projectValidation.npmRun(components, 'some/path', 'build', 'a')

    expect(execSpy).toHaveBeenCalledWith('some/path', 'npm', ['run', 'build', '--silent', '--', 'a'], {
      env: process.env
    })
  })

  it('npmCommand: should build pass on the process.env', async () => {
    const components = await initComponents()
    const execSpy = jest.spyOn(components.spawner, 'exec').mockResolvedValue()

    await projectValidation.npmCommand(components, 'some/path', 'install', 'boedo')

    expect(execSpy).toHaveBeenCalledWith('some/path', 'npm', ['install', 'boedo', '--silent'], {
      env: process.env
    })
  })
})
