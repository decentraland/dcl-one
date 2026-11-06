import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, before, describe, test } from 'node:test'
import { aboutDocument, buildScene, contentHash, serveRealm, type BuiltScene, type RealmServer } from '../driver/realm.ts'

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
    await writeFile(path.join(dir, 'assets', 'models', 'a.glb'), Buffer.from([1, 2, 3]))
    scene = await buildScene(dir)
    server = await serveRealm({ scenes: [scene], port: 0 })
  })

  after(async () => {
    await server.close()
    await rm(dir, { recursive: true, force: true })
  })

  test('buildScene lists the bundle and the assets by content hash', () => {
    assert.deepEqual(scene.entity.pointers, ['12,0', '13,0'])
    assert.equal(scene.entity.type, 'scene')
    assert.deepEqual(
      scene.entity.content.map((entry) => entry.file),
      ['bin/index.js', 'assets/models/a.glb']
    )
    assert.equal(scene.entity.content[1]!.hash, contentHash(Buffer.from([1, 2, 3])))
    assert.match(scene.entity.id, /^e2e[0-9a-f]{64}$/)
    assert.equal(scene.entity.metadata.main, 'bin/index.js')
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
    const configurations = about.configurations as { scenesUrn: string[]; realmName: string }
    assert.deepEqual(configurations.scenesUrn, [`urn:decentraland:entity:${scene.entity.id}?=&baseUrl=${server.realm}/content/contents/`])
    assert.equal(configurations.realmName, 'e2e-visual')
    assert.deepEqual(about.content, { healthy: true, publicUrl: `${server.realm}/content` })
    assert.deepEqual(about.comms, { healthy: true, protocol: 'v3', fixedAdapter: 'offline:offline' })
  })

  test('the entity and its files are served by hash', async () => {
    const entity = await (await fetch(`${server.realm}/content/contents/${scene.entity.id}`)).json()
    assert.deepEqual(entity, scene.entity)
    const file = await fetch(`${server.realm}/content/contents/${scene.entity.content[1]!.hash}`)
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
