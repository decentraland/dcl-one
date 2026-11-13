import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, before, describe, test } from 'node:test'
import { aboutDocument, BODY_SHAPES, buildPreviewWearable, buildScene, contentHash, EMPTY_REALM_MAP, PREVIEW_WEARABLE_URN, serveRealm, type BuiltScene, type PreviewWearable, type RealmServer } from '../driver/realm.ts'

describe('realm', () => {
  let dir: string
  let scene: BuiltScene
  let server: RealmServer

  before(async () => {
    dir = await mkdtemp(path.join(tmpdir(), 'e2e-realm-'))
    await mkdir(path.join(dir, 'bin'), { recursive: true })
    await mkdir(path.join(dir, 'assets', 'models'), { recursive: true })
    await writeFile(path.join(dir, 'scene.json'), JSON.stringify({ main: 'bin/index.js', scene: { base: '12,0', parcels: ['12,0', '13,0'] }, display: { title: 'fixture' } }))
    await writeFile(path.join(dir, 'bin', 'index.js'), 'export function main() {}')
    await writeFile(path.join(dir, 'bin', 'scene.js'), 'chunk')
    await writeFile(path.join(dir, 'assets', 'models', 'a.glb'), Buffer.from([1, 2, 3]))
    scene = await buildScene(dir)
    server = await serveRealm({ scenes: [scene], port: 0 })
  })

  after(async () => {
    await server.close()
    await rm(dir, { recursive: true, force: true })
  })

  test('buildScene lists the bundle, its chunks and the assets by content hash', () => {
    assert.deepEqual(scene.entity.pointers, ['12,0', '13,0'])
    assert.equal(scene.entity.type, 'scene')
    assert.deepEqual(
      scene.entity.content.map((entry) => entry.file),
      ['bin/index.js', 'bin/scene.js', 'assets/models/a.glb']
    )
    assert.equal(scene.entity.content[2]!.hash, contentHash(Buffer.from([1, 2, 3])))
    assert.match(scene.entity.id, /^e2e[0-9a-f]{64}$/)
    assert.equal(scene.entity.metadata.main, 'bin/index.js')
  })

  test('buildScene serves the project with a rewritten scene.json: its pointers, metadata and id follow', async () => {
    const host = await buildScene(dir, (metadata) => ({ ...metadata, scene: { base: '0,0', parcels: ['0,0'] }, featureToggles: { portableExperiences: 'disabled' } }))
    assert.deepEqual(host.entity.pointers, ['0,0'])
    assert.deepEqual(host.entity.metadata.featureToggles, { portableExperiences: 'disabled' })
    assert.deepEqual(host.entity.content, scene.entity.content)
    assert.notEqual(host.entity.id, scene.entity.id)
  })

  test('contentFailures answers a file 503 for its first GETs, then serves it; HEAD is not counted', async () => {
    const hash = contentHash(Buffer.from([1, 2, 3]))
    const flaky = await serveRealm({ scenes: [scene], port: 0, contentFailures: new Map([[hash, 2]]) })
    try {
      const url = `${flaky.realm}/content/contents/${hash}`
      assert.equal((await fetch(url, { method: 'HEAD' })).status, 200)
      const statuses = []
      for (let i = 0; i < 3; i++) {
        const response = await fetch(url)
        await response.arrayBuffer()
        statuses.push(response.status)
      }
      assert.deepEqual(statuses, [503, 503, 200])
      assert.equal(flaky.contentGets.get(hash), 3)
    } finally {
      await flaky.close()
    }
  })

  test('the hash is stable for the same bytes', () => {
    assert.equal(contentHash(Buffer.from('x')), contentHash(Buffer.from('x')))
    assert.notEqual(contentHash(Buffer.from('x')), contentHash(Buffer.from('y')))
  })

  test('a scene without its bundle is refused', async () => {
    const empty = await mkdtemp(path.join(tmpdir(), 'e2e-realm-empty-'))
    await writeFile(path.join(empty, 'scene.json'), JSON.stringify({ scene: { base: '0,0', parcels: ['0,0'] } }))
    await assert.rejects(buildScene(empty), /bin\/index.js is missing; build the scene first/)
    await rm(empty, { recursive: true, force: true })
  })

  test('/about names the scenes with the content base URL', async () => {
    const response = await fetch(`${server.realm}/about`)
    assert.equal(response.status, 200)
    assert.equal(response.headers.get('access-control-allow-origin'), '*')
    assert.equal(response.headers.get('cross-origin-resource-policy'), 'cross-origin')
    const about = (await response.json()) as ReturnType<typeof aboutDocument>
    const configurations = about.configurations as { scenesUrn: string[]; realmName: string; map?: unknown }
    assert.deepEqual(configurations.scenesUrn, [`urn:decentraland:entity:${scene.entity.id}?=&baseUrl=${server.realm}/content/contents/`])
    assert.equal(configurations.realmName, 'e2e-visual')
    assert.equal(configurations.map, undefined, 'a realm with scenes is bounded by them, like a World')
    assert.deepEqual(about.content, { healthy: true, publicUrl: `${server.realm}/content` })
    assert.deepEqual(about.comms, { healthy: true, protocol: 'v3', fixedAdapter: 'offline:offline' })
  })

  test('the entity and its files are served by hash', async () => {
    const entity = await (await fetch(`${server.realm}/content/contents/${scene.entity.id}`)).json()
    assert.deepEqual(entity, scene.entity)
    const chunk = await fetch(`${server.realm}/content/contents/${scene.entity.content[1]!.hash}`)
    assert.equal(chunk.headers.get('content-type'), 'text/javascript; charset=utf-8')
    assert.equal(await chunk.text(), 'chunk')
    const file = await fetch(`${server.realm}/content/contents/${scene.entity.content[2]!.hash}`)
    assert.equal(file.headers.get('content-type'), 'model/gltf-binary')
    assert.deepEqual(Buffer.from(await file.arrayBuffer()), Buffer.from([1, 2, 3]))
    assert.equal((await fetch(`${server.realm}/content/contents/e2enothing`)).status, 404)
  })

  test('entities/active answers the scenes at the pointers', async () => {
    const hit = await fetch(`${server.realm}/content/entities/active`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ pointers: ['13,0', '99,99'] }) })
    assert.deepEqual(await hit.json(), [scene.entity])
    const miss = await fetch(`${server.realm}/content/entities/active`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ pointers: ['99,99'] }) })
    assert.deepEqual(await miss.json(), [])
  })

  test('a preflight is answered and /scenes is empty', async () => {
    const preflight = await fetch(`${server.realm}/content/entities/active`, { method: 'OPTIONS' })
    assert.equal(preflight.status, 204)
    assert.equal(preflight.headers.get('access-control-allow-methods'), 'GET, HEAD, POST, OPTIONS')
    assert.deepEqual(await (await fetch(`${server.realm}/scenes`)).json(), { scenes: [], total: 0 })
  })
})

describe('realm with a previewed smart wearable', () => {
  let dir: string
  let wearable: PreviewWearable
  let server: RealmServer
  let plain: RealmServer

  before(async () => {
    dir = await mkdtemp(path.join(tmpdir(), 'e2e-realm-sw-'))
    await mkdir(path.join(dir, 'bin'), { recursive: true })
    await mkdir(path.join(dir, 'src'), { recursive: true })
    await writeFile(path.join(dir, 'scene.json'), JSON.stringify({ isPortableExperience: true, main: 'bin/index.js', runtimeVersion: '7', requiredPermissions: [], scene: { base: '0,0', parcels: ['0,0'] }, display: { title: 'Smart Wearable Fixture' } }))
    await writeFile(
      path.join(dir, 'wearable.json'),
      JSON.stringify({
        id: '00000000-0000-4000-8000-000000000000',
        name: 'Smart Wearable Fixture',
        rarity: 'mythic',
        data: {
          replaces: [],
          hides: [],
          tags: [],
          category: 'hat',
          representations: [{ bodyShapes: [BODY_SHAPES[0]], mainFile: 'model.glb', contents: ['model.glb'], overrideHides: [], overrideReplaces: [] }]
        }
      })
    )
    await writeFile(path.join(dir, 'bin', 'index.js'), 'export function main() {}')
    await writeFile(path.join(dir, 'bin', 'scene.js'), 'the chunk the loader reads')
    await writeFile(path.join(dir, 'model.glb'), Buffer.from([4, 5, 6]))
    await writeFile(path.join(dir, 'src', 'index.ts'), 'source, not shipped')
    wearable = await buildPreviewWearable(dir)
    server = await serveRealm({ scenes: [], port: 0, previewWearables: [wearable] })
    plain = await serveRealm({ scenes: [], port: 0 })
  })

  after(async () => {
    await server.close()
    await plain.close()
    await rm(dir, { recursive: true, force: true })
  })

  test('buildPreviewWearable lists the scene, the wearable, the bundle and its chunks and the model, not the sources', () => {
    assert.equal(wearable.urn, PREVIEW_WEARABLE_URN)
    assert.deepEqual(
      wearable.content.map((entry) => entry.file),
      ['scene.json', 'wearable.json', 'bin/index.js', 'bin/scene.js', 'model.glb']
    )
    assert.equal(wearable.content[4]!.hash, contentHash(Buffer.from([4, 5, 6])))
  })

  test('a project that is not a portable experience, or not built, is refused', async () => {
    const other = await mkdtemp(path.join(tmpdir(), 'e2e-realm-sw-bad-'))
    await writeFile(path.join(other, 'scene.json'), JSON.stringify({ main: 'bin/index.js' }))
    await writeFile(path.join(other, 'wearable.json'), JSON.stringify({ data: { representations: [] } }))
    await assert.rejects(buildPreviewWearable(other), /not a portable experience/)
    await writeFile(path.join(other, 'scene.json'), JSON.stringify({ isPortableExperience: true, main: 'bin/index.js' }))
    await assert.rejects(buildPreviewWearable(other), /bin\/index.js is missing; build the scene first/)
    await rm(other, { recursive: true, force: true })
  })

  test('/preview-wearables lists every file under male/ and female/, fetchable by hash', async () => {
    const response = await fetch(`${server.realm}/preview-wearables`)
    assert.equal(response.status, 200)
    assert.equal(response.headers.get('access-control-allow-origin'), '*')
    const body = (await response.json()) as { ok: boolean; data: { id: string; name: string; data: { category: string; representations: { bodyShapes: string[]; mainFile: string; contents: { key: string; url: string; hash: string }[] }[] } }[] }
    assert.equal(body.ok, true)
    assert.equal(body.data.length, 1)
    const [entry] = body.data
    assert.equal(entry!.id, PREVIEW_WEARABLE_URN)
    assert.equal(entry!.name, 'Smart Wearable Fixture')
    assert.equal(entry!.data.category, 'hat')
    assert.equal(entry!.data.representations.length, 1)
    const representation = entry!.data.representations[0]!
    assert.deepEqual(representation.bodyShapes, [...BODY_SHAPES])
    assert.equal(representation.mainFile, 'male/model.glb')
    const files = wearable.content.map((file) => file.file)
    assert.deepEqual(
      representation.contents.map((content) => content.key),
      [...files.map((file) => `male/${file}`), ...files.map((file) => `female/${file}`)]
    )
    for (const content of representation.contents) {
      assert.equal(content.url, `${server.realm}/content/contents/${content.hash}`)
      assert.equal((await fetch(content.url)).status, 200)
    }
    const model = representation.contents.find((content) => content.key === 'female/model.glb')!
    assert.deepEqual(Buffer.from(await (await fetch(model.url)).arrayBuffer()), Buffer.from([4, 5, 6]))
  })

  test('the previewed wearable is no scene of the realm', async () => {
    const about = (await (await fetch(`${server.realm}/about`)).json()) as { configurations: { scenesUrn: string[]; map?: unknown } }
    assert.deepEqual(about.configurations.scenesUrn, [])
    // with no scene to place, the map is what lets the client resolve the spawn parcel and arrive
    assert.deepEqual(about.configurations.map, EMPTY_REALM_MAP)
    const active = await fetch(`${server.realm}/content/entities/active`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ pointers: ['0,0'] }) })
    assert.deepEqual(await active.json(), [])
  })

  test('a realm without preview wearables answers 404, as a production catalyst does', async () => {
    assert.equal((await fetch(`${plain.realm}/preview-wearables`)).status, 404)
  })
})
