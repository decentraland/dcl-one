import { mkdtempSync, mkdirSync, promises, readFileSync, rmSync, writeFileSync } from 'fs'
import { tmpdir } from 'os'
import path from 'path'
import { isEditorScene, startValidations } from '../../../packages/@dcl/sdk-commands/src/logic/project-validations'

const WORKING_DIR = '/test/project'
const COMPOSITE_PATH = path.resolve(WORKING_DIR, 'assets', 'scene', 'main.composite')

function makeMockFs(fileExists: boolean, compositeContent?: object | string) {
  return {
    fileExists: jest.fn().mockImplementation((p: string) => {
      if (p === COMPOSITE_PATH) return Promise.resolve(fileExists)
      return Promise.resolve(false)
    }),
    readFile: jest.fn().mockImplementation(() => {
      const content = typeof compositeContent === 'string' ? compositeContent : JSON.stringify(compositeContent)
      return Promise.resolve(Buffer.from(content, 'utf-8'))
    })
  }
}

describe('isEditorScene', () => {
  afterEach(() => {
    jest.clearAllMocks()
  })

  it('returns false when main.composite does not exist', async () => {
    const fs = makeMockFs(false)
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
    expect(fs.readFile).not.toHaveBeenCalled()
  })

  it('returns false when composite JSON has no components field', async () => {
    const fs = makeMockFs(true, { version: 1 })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
  })

  it('returns false when composite has an empty components array', async () => {
    const fs = makeMockFs(true, { version: 1, components: [] })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
  })

  it('returns false when composite contains only core:: components', async () => {
    const fs = makeMockFs(true, {
      version: 1,
      components: [{ name: 'core::Transform' }, { name: 'core::GltfContainer' }]
    })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
  })

  it('returns false when composite contains only asset-packs::Script', async () => {
    const fs = makeMockFs(true, {
      version: 1,
      components: [{ name: 'asset-packs::Script' }]
    })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
  })

  it('returns true when composite contains a non-Script asset-packs:: component', async () => {
    const fs = makeMockFs(true, {
      version: 1,
      components: [{ name: 'asset-packs::Trigger' }]
    })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(true)
  })

  it('returns true when composite contains asset-packs::Script and a non-Script asset-packs:: component', async () => {
    const fs = makeMockFs(true, {
      version: 1,
      components: [{ name: 'asset-packs::Script' }, { name: 'asset-packs::Trigger' }]
    })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(true)
  })

  it('returns false when composite file contains invalid JSON', async () => {
    const fs = makeMockFs(true, 'this is not valid json {{{')
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
  })

  it('returns false when components is not an array', async () => {
    const fs = makeMockFs(true, { version: 1, components: { name: 'asset-packs::Trigger' } })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
  })

  it('returns false when component items do not have a name field', async () => {
    const fs = makeMockFs(true, {
      version: 1,
      components: [{ data: {} }, { jsonSchema: {} }]
    })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(false)
  })

  it('returns true when composite contains multiple non-Script asset-packs:: components', async () => {
    const fs = makeMockFs(true, {
      version: 1,
      components: [{ name: 'core::Transform' }, { name: 'asset-packs::Action' }, { name: 'asset-packs::State' }]
    })
    const result = await isEditorScene({ fs } as any, WORKING_DIR)
    expect(result).toBe(true)
  })
})

describe('startValidations', () => {
  let scene: string

  beforeEach(() => {
    scene = mkdtempSync(path.join(tmpdir(), 'scene-'))
    mkdirSync(path.join(scene, 'node_modules/@dcl/sdk'), { recursive: true })
    writeFileSync(
      path.join(scene, 'node_modules/@dcl/sdk/package.json'),
      JSON.stringify({ name: '@other-scope/sdk', version: '7.1.0-1.commit-abc' })
    )
  })

  afterEach(() => rmSync(scene, { recursive: true, force: true }))

  async function jsRuntimeWrittenFor(sdkSpec: string) {
    writeFileSync(path.join(scene, 'package.json'), JSON.stringify({ dependencies: { '@dcl/sdk': sdkSpec } }))
    const fs = {
      fileExists: (file: string) =>
        promises.access(file).then(
          () => true,
          () => false
        ),
      readFile: promises.readFile,
      writeFile: promises.writeFile
    }
    await startValidations({ fs, logger: { error: jest.fn() } } as any, scene)
    return JSON.parse(readFileSync(path.join(scene, 'package.json'), 'utf8')).dependencies['@dcl/js-runtime']
  }

  it('pins js-runtime to the installed sdk version', async () => {
    expect(await jsRuntimeWrittenFor('^7.1.0')).toBe('7.1.0-1.commit-abc')
  })

  it('keeps the scope of an aliased sdk', async () => {
    expect(await jsRuntimeWrittenFor('npm:@other-scope/sdk@7.1.0-1.commit-abc')).toBe(
      'npm:@other-scope/js-runtime@7.1.0-1.commit-abc'
    )
  })
})
