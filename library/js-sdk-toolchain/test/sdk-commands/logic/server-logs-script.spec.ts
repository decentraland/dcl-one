import { stripVTControlCharacters } from 'util'
import { EventEmitter } from 'events'
import { PassThrough } from 'stream'
import child_process from 'child_process'
import fs from 'fs'
import os from 'os'
import path from 'path'
import esbuild from '../../../packages/@dcl/sdk-commands/node_modules/esbuild'
import { createFsComponent } from '../../../packages/@dcl/sdk-commands/src/components/fs'
import { bundleProject, BundleComponents, CompileOptions } from '../../../packages/@dcl/sdk-commands/src/logic/bundle'
import { initLanguage, Language } from '../../../packages/@dcl/sdk-commands/src/logic/lang'

jest.mock('../../../packages/@dcl/sdk-commands/node_modules/esbuild', () => ({ context: jest.fn() }))
jest.mock('child_process', () => ({ ...jest.requireActual('child_process'), fork: jest.fn() }))

/** A type checker that exits clean on the next turn. */
function fakeTypeChecker() {
  const child = Object.assign(new EventEmitter(), {
    stdout: new PassThrough(),
    stderr: new PassThrough(),
    killed: false,
    kill: jest.fn()
  })
  setImmediate(() => child.emit('close', 0))
  return child
}

/** A compiler whose only output is the list of files the bundle was built from. */
function compilerBuilding(inputs: string[]) {
  jest.mocked(esbuild.context).mockResolvedValue({
    rebuild: async () => ({ metafile: { inputs: Object.fromEntries(inputs.map((input) => [input, { bytes: 1 }])) } }),
    dispose: async () => {}
  } as any)
  jest.mocked(child_process.fork).mockImplementation(fakeTypeChecker as any)
}

// Stop at the compiler boundary: these tests exercise the build's real JSON updates.
describe('when a scene build configures its server logs script', () => {
  let directory: string
  let packageFile: string
  let components: BundleComponents
  let options: CompileOptions
  let compilerBoundary: Error
  let original: string

  beforeEach(() => {
    directory = fs.mkdtempSync(path.join(os.tmpdir(), 'sdk-logs-script-'))
    packageFile = path.join(directory, 'package.json')
    fs.mkdirSync(path.join(directory, 'src'))
    fs.writeFileSync(path.join(directory, 'src/index.ts'), '')
    fs.writeFileSync(path.join(directory, 'tsconfig.json'), '{}')
    fs.writeFileSync(path.join(directory, 'scene.json'), '{"authoritativeMultiplayer":false}')
    components = {
      fs: createFsComponent(),
      logger: { log: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn(), debug: jest.fn() }
    }
    options = {
      workingDirectory: directory,
      production: false,
      watch: false,
      emitDeclaration: false,
      ignoreComposite: true,
      customEntryPoint: true,
      projectDefaults: true
    }
    compilerBoundary = new Error('compiler boundary')
    jest.mocked(esbuild.context).mockRejectedValue(compilerBoundary)
  })

  afterEach(() => {
    jest.restoreAllMocks()
    fs.rmSync(directory, { recursive: true, force: true })
  })

  async function build(): Promise<void> {
    await expect(
      bundleProject(components, options, { main: 'bin/game.js', runtimeVersion: '7' } as Parameters<
        typeof bundleProject
      >[2])
    ).rejects.toBe(compilerBoundary)
  }

  describe.each([undefined, 'sdk-commands sdk-server-logs'])('and the previous script is %s', (previous) => {
    beforeEach(() => {
      fs.writeFileSync(
        packageFile,
        JSON.stringify({ name: 'scene', scripts: { build: 'sdk-commands build', 'server-logs': previous } })
      )
    })

    it('should use Insights and retain the other package and scene settings', async () => {
      await build()
      expect(
        jest.mocked(components.logger.log).mock.calls.map(([message]) => stripVTControlCharacters(message))
      ).toContain(
        `${previous === undefined ? 'Added' : 'Updated'} scripts.server-logs: "sdk-commands scene-insights tail" in package.json`
      )
      expect(JSON.parse(fs.readFileSync(packageFile, 'utf8'))).toEqual({
        name: 'scene',
        scripts: { build: 'sdk-commands build', 'server-logs': 'sdk-commands scene-insights tail' }
      })
      expect(JSON.parse(fs.readFileSync(path.join(directory, 'scene.json'), 'utf8')).authoritativeMultiplayer).toBe(
        false
      )
    })

    it('should leave the package unchanged on a second build', async () => {
      await build()
      original = fs.readFileSync(packageFile, 'utf8')
      await build()
      expect(fs.readFileSync(packageFile, 'utf8')).toBe(original)
    })
  })

  describe.each([
    'node custom-logs.js',
    'sdk-commands sdk-server-logs --world custom.dcl.eth',
    'sdk-commands scene-insights tail',
    ''
  ])('and the script is customized or current: %s', (script) => {
    beforeEach(() => {
      original = JSON.stringify({ scripts: { 'server-logs': script } })
      fs.writeFileSync(packageFile, original)
    })

    it('should preserve the package byte for byte', async () => {
      await build()
      expect(fs.readFileSync(packageFile, 'utf8')).toBe(original)
    })
  })

  describe('and there is no scripts object', () => {
    beforeEach(() => {
      fs.writeFileSync(packageFile, '{"name":"scene"}')
    })

    it('should add the Insights script', async () => {
      await build()
      expect(JSON.parse(fs.readFileSync(packageFile, 'utf8')).scripts).toEqual({
        'server-logs': 'sdk-commands scene-insights tail'
      })
    })
  })

  describe('and building a single file', () => {
    beforeEach(() => {
      options.single = 'src/index.ts'
      original = '{"scripts":{"server-logs":"sdk-commands sdk-server-logs"}}'
      fs.writeFileSync(packageFile, original)
    })

    it('should not migrate project defaults', async () => {
      await build()
      expect(fs.readFileSync(packageFile, 'utf8')).toBe(original)
    })
  })

  describe('and the build is not a `build` or a `start` (a deploy, a packaging step)', () => {
    beforeEach(() => {
      options.projectDefaults = false
      original = '{"scripts":{"server-logs":"sdk-commands sdk-server-logs"}}'
      fs.writeFileSync(packageFile, original)
    })

    it('should leave the project as it is', async () => {
      await build()
      expect(fs.readFileSync(packageFile, 'utf8')).toBe(original)
    })
  })

  describe('and the package keeps its own formatting', () => {
    beforeEach(() => {
      fs.writeFileSync(packageFile, '{\r\n\t"name": "scene",\r\n\t"scripts": {\r\n\t\t"build": "sdk-commands build"\r\n\t}\r\n}\r\n')
    })

    it('should add the script keeping tabs and line endings', async () => {
      await build()
      expect(fs.readFileSync(packageFile, 'utf8')).toBe(
        '{\r\n\t"name": "scene",\r\n\t"scripts": {\r\n\t\t"build": "sdk-commands build",\r\n\t\t"server-logs": "sdk-commands scene-insights tail"\r\n\t}\r\n}\r\n'
      )
    })
  })
})

describe('when a scene build succeeds', () => {
  let directory: string
  let sceneFile: string
  let packageFile: string
  let components: BundleComponents
  let options: CompileOptions

  const logs = () => jest.mocked(components.logger.log).mock.calls.map(([message]) => stripVTControlCharacters(message))
  const warnings = () => logs().filter((message) => message.includes('WARNING'))
  const write = (name: string, text: string) => {
    const file = path.join(directory, name)
    fs.mkdirSync(path.dirname(file), { recursive: true })
    fs.writeFileSync(file, text)
  }
  const read = (name: string) => fs.readFileSync(path.join(directory, name), 'utf8')

  beforeAll(async () => {
    await initLanguage(Language.EN)
  })

  beforeEach(() => {
    directory = fs.mkdtempSync(path.join(os.tmpdir(), 'sdk-build-'))
    sceneFile = path.join(directory, 'scene.json')
    packageFile = path.join(directory, 'package.json')
    write('src/index.ts', 'export {}\n')
    write('tsconfig.json', '{ "compilerOptions": { "strict": true } }')
    write('scene.json', '{\n  "name": "scene",\n  "main": "bin/game.js"\n}\n')
    write('package.json', '{"name":"scene"}')
    components = {
      fs: createFsComponent(),
      logger: { log: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn(), debug: jest.fn() }
    }
    options = {
      workingDirectory: directory,
      production: false,
      watch: false,
      emitDeclaration: false,
      ignoreComposite: true,
      customEntryPoint: true,
      projectDefaults: true
    }
  })

  afterEach(() => {
    jest.restoreAllMocks()
    fs.rmSync(directory, { recursive: true, force: true })
  })

  async function build(): Promise<void> {
    await bundleProject(components, options, { main: 'bin/game.js', runtimeVersion: '7' } as Parameters<
      typeof bundleProject
    >[2])
  }

  describe('and the scene uses the server APIs', () => {
    beforeEach(() => {
      compilerBuilding(['src/index.ts', 'node_modules/@dcl/sdk/index.js', 'node_modules/@dcl/sdk/server/index.js'])
    })

    it('should mark the scene as authoritative multiplayer, keeping the formatting of scene.json', async () => {
      await build()
      expect(fs.readFileSync(sceneFile, 'utf8')).toBe(
        '{\n  "name": "scene",\n  "main": "bin/game.js",\n  "authoritativeMultiplayer": true\n}\n'
      )
      expect(logs()).toContain('Added authoritativeMultiplayer: true in scene.json')
    })

    it('should keep an explicit choice', async () => {
      write('scene.json', '{"authoritativeMultiplayer":false}')
      await build()
      expect(fs.readFileSync(sceneFile, 'utf8')).toBe('{"authoritativeMultiplayer":false}')
    })

    it('should leave scene.json and package.json alone on a deploy', async () => {
      options.projectDefaults = false
      await build()
      expect(fs.readFileSync(sceneFile, 'utf8')).toBe('{\n  "name": "scene",\n  "main": "bin/game.js"\n}\n')
      expect(fs.readFileSync(packageFile, 'utf8')).toBe('{"name":"scene"}')
    })
  })

  describe('and the scene does not use the server APIs', () => {
    beforeEach(() => {
      compilerBuilding(['src/index.ts', 'src/server/helpers.ts', 'node_modules/@dcl/sdk/index.js'])
    })

    it('should not mark the scene as authoritative multiplayer', async () => {
      await build()
      expect(fs.readFileSync(sceneFile, 'utf8')).toBe('{\n  "name": "scene",\n  "main": "bin/game.js"\n}\n')
      expect(JSON.parse(read('package.json')).scripts['server-logs']).toBe('sdk-commands scene-insights tail')
    })
  })

  describe('and a source still names components with the asset-packs:: prefix', () => {
    beforeEach(() => {
      write('src/index.ts', "import { engine } from '@dcl/sdk/ecs'\nengine.getComponent('asset-packs::Actions')\n")
      write('src/other.ts', "export const legacy = 'asset-packs::States'\n")
      write('node_modules/@dcl/smart-items/dist/legacy.js', "exports.LEGACY_PREFIX = 'asset-packs::'\n")
      write('assets/scene/main.composite', '{"components":[{"name":"asset-packs::Actions"}]}')
      compilerBuilding([
        'src/index.ts',
        'src/other.ts',
        'src/missing.ts',
        'assets/scene/main.composite',
        'node_modules/@dcl/smart-items/dist/legacy.js',
        '<stdin>'
      ])
    })

    it('should warn once, naming the files, the lines and the smart-items:: replacement', async () => {
      await build()
      expect(warnings()).toHaveLength(1)
      expect(warnings()[0]).toContain('src/index.ts:2, src/other.ts:1')
      expect(warnings()[0]).toContain('"smart-items::"')
      expect(warnings()[0]).not.toContain('node_modules')
      expect(warnings()[0]).not.toContain('composite')
    })
  })

  describe('and no source names a legacy component', () => {
    beforeEach(() => {
      compilerBuilding(['src/index.ts'])
    })

    it('should not warn', async () => {
      await build()
      expect(warnings()).toEqual([])
    })
  })
})
