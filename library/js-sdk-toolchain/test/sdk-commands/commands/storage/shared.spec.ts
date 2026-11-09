import { initComponents } from '../../../../packages/@dcl/sdk-commands/src/components'
import * as workspaceValidations from '../../../../packages/@dcl/sdk-commands/src/logic/workspace-validations'
import * as sceneValidations from '../../../../packages/@dcl/sdk-commands/src/logic/scene-validations'
import { PassThrough, Writable } from 'stream'
import {
  validateWorkspaceAndWorld,
  buildStorageMetadata,
  createStorageInfo,
  readSecretValue
} from '../../../../packages/@dcl/sdk-commands/src/commands/storage/shared'

const REMOTE = 'https://storage.decentraland.org'
const LOCAL = 'http://localhost:3000'

afterEach(() => {
  jest.restoreAllMocks()
})

function mockSceneJson(scene: any) {
  jest.spyOn(workspaceValidations, 'getValidWorkspace').mockResolvedValue({} as any)
  jest.spyOn(sceneValidations, 'getValidSceneJson').mockResolvedValue(scene)
}

describe('storage/shared: validateWorkspaceAndWorld', () => {
  it('resolves a World scene from worldConfiguration.name', async () => {
    const components = await initComponents()
    mockSceneJson({ worldConfiguration: { name: 'boedo.dcl.eth' }, scene: { base: '0,0', parcels: ['0,0'] } })

    const res = await validateWorkspaceAndWorld(components, 'root', REMOTE)

    expect(res).toEqual({ worldName: 'boedo.dcl.eth', baseParcel: '0,0', parcels: ['0,0'] })
  })

  it('resolves a Genesis City scene (no worldConfiguration.name) by its base parcel', async () => {
    const components = await initComponents()
    mockSceneJson({ scene: { base: '20,2', parcels: ['20,2', '20,3'] } })

    const res = await validateWorkspaceAndWorld(components, 'root', REMOTE)

    expect(res).toEqual({ worldName: undefined, baseParcel: '20,2', parcels: ['20,2', '20,3'] })
  })

  it('falls back to the base parcel for parcels when scene.parcels is absent', async () => {
    const components = await initComponents()
    mockSceneJson({ scene: { base: '-3,-2' } })

    const res = await validateWorkspaceAndWorld(components, 'root', REMOTE)

    expect(res).toEqual({ worldName: undefined, baseParcel: '-3,-2', parcels: ['-3,-2'] })
  })

  it('throws when neither worldConfiguration.name nor scene.base is present', async () => {
    const components = await initComponents()
    mockSceneJson({ scene: {} })

    await expect(validateWorkspaceAndWorld(components, 'root', REMOTE)).rejects.toMatchObject({
      name: 'STORAGE_MISSING_LOCATION'
    })
  })

  it('does not throw against a local target even without world name or base parcel', async () => {
    const components = await initComponents()
    mockSceneJson({ scene: {} })

    const res = await validateWorkspaceAndWorld(components, 'root', LOCAL)

    expect(res).toEqual({ worldName: undefined, baseParcel: '0,0', parcels: ['0,0'] })
  })

  it('treats a lookalike localhost host as remote (exact-hostname match)', async () => {
    const components = await initComponents()
    mockSceneJson({ scene: {} })

    await expect(validateWorkspaceAndWorld(components, 'root', 'https://localhost.evil.com')).rejects.toMatchObject({
      name: 'STORAGE_MISSING_LOCATION'
    })
  })
})

describe('storage/shared: buildStorageMetadata', () => {
  it('sends realm + realmName + parcel for a World', () => {
    expect(JSON.parse(buildStorageMetadata('boedo.dcl.eth', '0,0'))).toEqual({
      realm: { serverName: 'boedo.dcl.eth' },
      realmName: 'boedo.dcl.eth',
      parcel: '0,0'
    })
  })

  it('sends only the parcel for a Genesis City scene (server defaults the realm to "main")', () => {
    expect(JSON.parse(buildStorageMetadata(undefined, '20,2'))).toEqual({ parcel: '20,2' })
  })
})

describe('storage/shared: createStorageInfo', () => {
  it('marks a World as isWorld and carries the world name', () => {
    const info = createStorageInfo(
      'player',
      'get',
      'https://storage.decentraland.org/players/0xabc/values/k',
      'boedo.dcl.eth',
      '0,0',
      ['0,0'],
      'k',
      '0xabc'
    )

    expect(info.isWorld).toBe(true)
    expect(info.world).toBe('boedo.dcl.eth')
    expect(JSON.parse(info.metadata)).toEqual({
      realm: { serverName: 'boedo.dcl.eth' },
      realmName: 'boedo.dcl.eth',
      parcel: '0,0'
    })
  })

  it('marks a Genesis City scene as not isWorld and omits the realm from metadata', () => {
    const info = createStorageInfo(
      'player',
      'get',
      'https://storage.decentraland.org/players/0xabc/values/k',
      undefined,
      '20,2',
      ['20,2'],
      'k',
      '0xabc'
    )

    expect(info.isWorld).toBe(false)
    expect(info.world).toBeUndefined()
    expect(JSON.parse(info.metadata)).toEqual({ parcel: '20,2' })
  })
})

describe('storage/shared: readSecretValue', () => {
  const logger = { log: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn(), debug: jest.fn() }
  const logged = () => [...logger.log.mock.calls, ...logger.warn.mock.calls].flat().join('\n')

  function capture() {
    const chunks: string[] = []
    const stream = new Writable({
      write(chunk, _encoding, callback) {
        chunks.push(chunk.toString())
        callback()
      }
    })
    return { stream, text: () => chunks.join('') }
  }

  function terminal() {
    return Object.assign(new PassThrough(), {
      isTTY: true,
      setRawMode() {
        return this
      }
    })
  }

  beforeEach(() => {
    jest.clearAllMocks()
  })

  it('takes --value and warns that other processes can read it', async () => {
    const out = capture()
    expect(await readSecretValue('s3cret', 'API_KEY', logger as any, { stdin: new PassThrough(), stdout: out.stream })).toBe(
      's3cret'
    )
    expect(logged()).toContain('process list')
    expect(out.text()).toBe('')
  })

  it('reads a piped value from stdin and drops one trailing newline', async () => {
    const stdin = new PassThrough()
    stdin.end('s3cret with spaces\n\n')
    const out = capture()
    expect(await readSecretValue(undefined, 'API_KEY', logger as any, { stdin, stdout: out.stream })).toBe(
      's3cret with spaces\n'
    )
    expect(logged()).toBe('')
    expect(out.text()).toBe('')
  })

  it('prompts without echoing when stdin is a terminal', async () => {
    const stdin = terminal()
    const out = capture()
    const pending = readSecretValue(undefined, 'API_KEY', logger as any, { stdin, stdout: out.stream })
    stdin.write('s3cret\n')
    expect(await pending).toBe('s3cret')
    expect(out.text()).toContain('Value for API_KEY')
    expect(out.text()).not.toContain('s3cret')
  })

  it('refuses an empty value', async () => {
    const stdin = new PassThrough()
    stdin.end('')
    await expect(readSecretValue(undefined, 'API_KEY', logger as any, { stdin, stdout: capture().stream })).rejects.toThrow(
      "No value given for 'API_KEY'"
    )
  })
})
