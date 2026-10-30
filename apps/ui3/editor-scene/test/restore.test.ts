import assert from 'node:assert/strict'
import { test } from 'node:test'
import { cmd } from '../src/cmd'
import { state } from '../src/state'
import { NAME_COMPONENT, customComponentId } from '../src/custom-components'
import { restoreComposite } from '../src/inspector/hydrate'
import { reloadSnapshot } from '../src/inspector/session'
import { addComponent } from '../src/hierarchy-runtime'

const composite = (entities: Record<string, string>) => JSON.stringify({ components: [
  { name: NAME_COMPONENT, data: Object.fromEntries(Object.entries(entities).map(([id, value]) => [id, { json: { value } }])) },
  { name: 'core::Transform', data: Object.fromEntries(Object.keys(entities).map(id => [id, { json: { position: { x: 1, y: 2, z: 3 }, parent: 0 } }])) }
] })

function componentScene() {
  state.status = 'ready'
  state.playing = false
  state.frozen = true
  state.scene = { hash: 'component-test' } as NonNullable<typeof state.scene>
  state.snapshot = { '512': {} }
  state.editedComponents.clear()
  state.editorValues.clear()
  cmd.setScene = async () => 'ok'
  cmd.sceneStats = async () => 'hash: component-test'
  cmd.componentSchema = async () => '{}'
}

test('component creation waits for the native acknowledgment and uses engine defaults', async () => {
  componentScene()
  let finish!: () => void
  let wrote!: () => void
  const started = new Promise<void>(resolve => { wrote = resolve })
  let writes = 0
  cmd.componentDefault = async () => '{"visible":true}'
  cmd.setComponent = async (id, name, json) => {
    writes++
    assert.deepEqual([id, name, JSON.parse(json)], ['512', 'core::VisibilityComponent', { visible: true }])
    wrote()
    await new Promise<void>(resolve => { finish = resolve })
    return 'ok'
  }
  let complete = false
  const pending = addComponent('512', 'core::VisibilityComponent').then(() => { complete = true })
  await started
  assert.equal(complete, false)
  assert.equal(state.snapshot['512']['core::VisibilityComponent'], undefined)
  finish()
  await pending
  assert.deepEqual(state.snapshot['512']['core::VisibilityComponent'], { visible: true })
  await addComponent('512', 'core::VisibilityComponent')
  assert.equal(writes, 1)
})

test('component default and write failures reject without recording authored state', async () => {
  componentScene()
  let writes = 0
  cmd.setComponent = async () => { writes++; throw new Error('engine refused') }
  cmd.componentDefault = async () => { throw new Error('unknown component') }
  await assert.rejects(addComponent('512', 'missing'), /unknown component/)
  assert.equal(writes, 0)
  cmd.componentDefault = async () => '{"visible":true}'
  await assert.rejects(addComponent('512', 'core::VisibilityComponent'), /engine refused/)
  assert.equal(state.editedComponents.size, 0)
  assert.deepEqual(state.snapshot['512'], {})
  for (const entity of ['0', '511', 'NaN', '513']) await assert.rejects(addComponent(entity, 'core::VisibilityComponent'), /authored entity/)
  assert.equal(writes, 1)
  state.playing = true
  await assert.rejects(addComponent('512', 'core::VisibilityComponent'), /Stop the preview/)
  state.playing = false
  state.scene = undefined
})

test('reset restoration works while frozen and restores removed components, names, and authored entity IDs', async () => {
  state.status = 'ready'
  state.scene = undefined
  state.frozen = true
  state.componentNames = ['Transform', 'MeshRenderer']
  state.editorValues.set('512/MeshRenderer', { mesh: { box: {} } })
  state.snapshot = { '512': { [NAME_COMPONENT]: { value: 'original' }, Transform: {}, MeshRenderer: {} } }
  const writes: unknown[] = []
  cmd.setComponent = async (id, name, value) => { writes.push([id, name, JSON.parse(value)]); return 'ok' }
  cmd.setComponentRaw = async (id, component) => { writes.push([id, component]); return 'ok' }
  cmd.deleteComponent = async (id, name) => { writes.push(['delete', id, name]); return 'ok' }
  cmd.newEntity = async (_component, _base64, count, ids) => {
    assert.equal(count, 1)
    assert.deepEqual(ids, [513])
    return ids!
  }
  await restoreComposite(composite({ '512': 'renamed', '513': 'added' }))
  assert.deepEqual(writes[0], ['delete', '512', 'MeshRenderer'])
  assert.equal(state.deletedComponents.has('512/MeshRenderer'), true)
  assert.equal(state.editorValues.has('512/MeshRenderer'), false)
  assert.equal(state.snapshot['512'][NAME_COMPONENT].value, 'renamed')
  assert.equal(state.snapshot['513'][NAME_COMPONENT].value, 'added')
  assert.deepEqual(state.snapshot['513'].Transform.position, { x: 1, y: 2, z: 3 })
})

test('failed allocation is reported instead of acknowledging partial restoration', async () => {
  state.status = 'ready'
  state.frozen = true
  state.snapshot = {}
  cmd.newEntity = async () => []
  await assert.rejects(restoreComposite(composite({ '513': 'added' })), /could be allocated/)
})

test('restoration preserves root scene metadata and clears metadata absent from the saved scene', async () => {
  state.status = 'ready'
  state.scene = undefined
  state.frozen = true
  state.snapshot = {}
  state.editorValues.clear()
  state.editedComponents.clear()
  state.deletedComponents.clear()
  const name = 'inspector::SceneMetadata-v3'
  const metadata = { name: 'Saved title', description: 'Saved description', layout: { base: { x: 0, y: 0 }, parcels: [{ x: 0, y: 0 }] } }
  const writes: unknown[] = []
  cmd.setComponentRaw = async (id, component) => { writes.push([id, component]); return 'ok' }
  await restoreComposite(JSON.stringify({ version: 1, components: [{ name, data: { '0': { json: metadata } } }] }))
  assert.deepEqual(writes, [['0', customComponentId(name)]])
  assert.deepEqual(state.snapshot['0'][name], metadata)
  cmd.setComponentRaw = async () => { throw new Error('renderer refused') }
  await assert.rejects(restoreComposite(JSON.stringify({ version: 1, components: [{ name, data: { '0': { json: { ...metadata, name: 'Rejected title' } } } }] })), /Scene restore failed/)
  assert.deepEqual(state.snapshot['0'][name], metadata)
  cmd.deleteComponent = async (id, component) => { writes.push(['delete', id, component]); return 'ok' }
  await restoreComposite(composite({}))
  assert.deepEqual(writes.at(-1), ['delete', '0', String(customComponentId(name))])
  assert.equal(state.snapshot['0'][name], undefined)
  assert.equal(state.deletedComponents.has(`0/${name}`), true)
})

test('restoration deletes an extra subtree once, without deleting its children again', async () => {
  state.status = 'ready'
  state.scene = undefined
  state.frozen = true
  state.snapshot = {
    '513': { [NAME_COMPONENT]: { value: 'parent' }, Transform: { parent: 0 } },
    '514': { [NAME_COMPONENT]: { value: 'child' }, Transform: { parent: 513 } }
  }
  const deleted: string[] = []
  cmd.deleteEntity = async (id, recursive) => {
    assert.equal(recursive, true)
    assert.equal(id, '513')
    deleted.push(id)
    return 'ok'
  }
  await restoreComposite(composite({}))
  assert.deepEqual(deleted, ['513'])
})

test('snapshot readiness waits for view freeze and recovers from replacement of the inspected runtime', async () => {
  state.status = 'ready'
  state.snapshot = {}
  cmd.crdtSnapshot = async () => ({})
  let finish!: () => void
  cmd.freezeView = () => new Promise(resolve => { finish = () => resolve('ok') })
  const pending = reloadSnapshot()
  await Promise.resolve()
  assert.equal(state.status, 'loading-snapshot')
  finish()
  await pending
  assert.equal(state.status, 'ready')

  cmd.freezeView = async () => { throw new Error('active inspection scene no longer exists; use /set_scene to update') }
  await reloadSnapshot()
  assert.equal(state.status, 'error')
  assert.match(state.error, /active inspection scene no longer exists/)
  cmd.freezeView = async () => 'ok'
  await reloadSnapshot()
  assert.equal(state.status, 'ready')
  assert.equal(state.error, '')
})

test('failed animation freeze is retried before the snapshot can become ready', async () => {
  state.editorValues.clear()
  cmd.crdtSnapshot = async () => ({ '610': { Animator: { states: [{ clip: 'walk', playing: true, speed: 1 }] } } })
  cmd.freezeView = async () => 'ok'
  let writes = 0
  cmd.setComponent = async (_id, _component, value) => {
    writes++
    assert.equal(JSON.parse(value).states[0].speed, 0)
    if (writes === 1) throw new Error('scene was replaced')
    return 'ok'
  }
  await reloadSnapshot()
  assert.equal(state.status, 'error')
  await reloadSnapshot()
  assert.equal(state.status, 'ready')
  assert.equal(writes, 2)
})

test('restore accepts an already removed extra only after a fresh engine snapshot confirms its absence', async () => {
  state.status = 'ready'
  state.scene = undefined
  state.frozen = true
  const extra = { [NAME_COMPONENT]: { value: 'rolled back allocation' }, Transform: { parent: 0 } }
  state.snapshot = { '520': extra }
  let confirmations = 0
  cmd.deleteEntity = async () => { throw new Error('entity 520 does not exist') }
  cmd.crdtSnapshot = async () => { confirmations++; return {} }
  await restoreComposite(composite({}))
  assert.equal(confirmations, 1)
  assert.equal(state.snapshot['520'], undefined)

  state.snapshot = { '520': extra }
  cmd.crdtSnapshot = async () => ({ '520': extra })
  await assert.rejects(restoreComposite(composite({})), /Scene restore failed/)
  assert.ok(state.snapshot['520'])

  cmd.crdtSnapshot = async () => { throw new Error('snapshot unavailable') }
  await assert.rejects(restoreComposite(composite({})), /Scene restore failed/)
  assert.ok(state.snapshot['520'])
})
