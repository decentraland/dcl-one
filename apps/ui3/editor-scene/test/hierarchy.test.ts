import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createHierarchyOperations, componentClipboard, pastedComponent } from '../src/hierarchy-operations'
import { hierarchyNodes, NODES_COMPONENT } from '../src/hierarchy-order'
import { localRelativeTo } from '../src/world-pos'
import { buildComposite } from '../src/composite'
import { state, type Snapshot } from '../src/state'
import { computeSaveDiff, buildAuthoredFromSelection } from '../src/save-diff'
const transform = (parent = 0, x = 0) => ({ parent, position: { x, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0, w: 1 }, scale: { x: 1, y: 1, z: 1 } })
const scene = (): Snapshot => ({ '0': {}, '512': { Transform: transform(0, 10) }, '513': { Transform: transform(512, 2) }, '514': { Transform: transform(0, 20) }, '515': { Transform: transform(0, 30) } })
function harness(snapshot = scene()) {
  const writes: string[] = []
  const move = createHierarchyOperations({ snapshot: () => snapshot, ready: async () => {}, localRelativeTo,
    write: async (id, name, value) => { writes.push(`${id}/${name}`); (snapshot[id] ??= {})[name] = value }, refresh: async () => {} })
  return { snapshot, writes, move }
}
test('reparent selected roots preserves world pose and descendant relationships', async () => {
  const { snapshot, move, writes } = harness()
  await move(['512', '513'], '514', 'inside')
  assert.deepEqual(snapshot['512'].Transform, transform(514, -10))
  assert.deepEqual(snapshot['513'].Transform, transform(512, 2))
  assert.deepEqual(writes, ['512/Transform', `0/${NODES_COMPONENT}`])
})
test('cycle and reserved entity rejection do not write any component', async () => {
  const { move, writes } = harness()
  await assert.rejects(move(['512'], '513', 'inside'), /children/)
  await assert.rejects(move(['0'], '512', 'inside'), /authored/)
  await assert.rejects(move(['514'], '0', 'before'), /root/)
  assert.deepEqual(writes, [])
})
test('sibling order persists in the exported upstream Nodes component', async () => {
  const { snapshot, move } = harness()
  await move(['515'], '514', 'before')
  assert.deepEqual(hierarchyNodes(snapshot).find(node => node.entity === 0)?.children, [512, 515, 514])
  state.componentNames = ['Transform']
  const composite = JSON.parse(buildComposite(snapshot))
  const nodes = composite.components.find((block: { name: string }) => block.name === NODES_COMPONENT)
  const value = nodes.data['0'].json ?? nodes.data['0']
  assert.deepEqual(value.value.find((node: { entity: number }) => node.entity === 0).children, [512, 515, 514])
})
test('save selection preserves acknowledged sibling metadata across export and reload', async () => {
  const initial = scene()
  const { snapshot, move } = harness(structuredClone(initial))
  await move(['515'], '514', 'before')
  state.componentNames = ['Transform']
  const key = `0/${NODES_COMPONENT}`
  const previousEdited = state.editedComponents.has(key)
  const previousValue = state.editorValues.get(key)
  state.editedComponents.add(key)
  state.editorValues.set(key, snapshot['0'][NODES_COMPONENT])
  try {
    const rows = computeSaveDiff(initial, snapshot)
    const authored = buildAuthoredFromSelection(initial, rows, new Map())
    const composite = JSON.parse(buildComposite(authored))
    const nodes = composite.components.find((block: { name: string }) => block.name === NODES_COMPONENT)
    const saved = nodes.data['0'].json ?? nodes.data['0']
    const reloaded = { ...initial, '0': { [NODES_COMPONENT]: saved } }
    assert.deepEqual(hierarchyNodes(reloaded).find(node => node.entity === 0)?.children, [512, 515, 514])
  } finally {
    if (!previousEdited) state.editedComponents.delete(key)
    if (previousValue === undefined) state.editorValues.delete(key)
    else state.editorValues.set(key, previousValue)
  }
})
test('failed engine writes reject the move before committing sibling metadata', async () => {
  const writes: string[] = []
  const move = createHierarchyOperations({ snapshot: scene, ready: async () => {}, localRelativeTo,
    write: async (_id, name) => { writes.push(name); throw new Error('engine disconnected') }, refresh: async () => {} })
  await assert.rejects(move(['512'], '514', 'inside'), /engine disconnected/)
  assert.deepEqual(writes, ['Transform'])
})
test('upstream-compatible component clipboard preserves destination hierarchy pointers', () => {
  const copied = componentClipboard('Transform', transform(512, 5))
  assert.equal(copied.__dclComponent, 'core::Transform')
  assert.equal('parent' in copied.value, false)
  assert.deepEqual(pastedComponent('Transform', copied, transform(514)), transform(514, 5))
  const ui = pastedComponent('UiTransform', { __dclComponent: 'core::UiTransform', value: { parent: 900, rightOf: 901, width: 40 } }, { parent: 512, rightOf: 513 })
  assert.deepEqual(ui, { parent: 512, rightOf: 513, width: 40 })
  assert.throws(() => pastedComponent('Transform', { __dclComponent: 'core::TextShape', value: {} }, {}), /does not contain/)
})

test('a later rejected move rolls completed transforms back', async () => {
  const snapshot = scene()
  let writes = 0
  const move = createHierarchyOperations({ snapshot: () => snapshot, ready: async () => {}, localRelativeTo,
    write: async (id, name, value) => {
      writes++
      if (writes === 2) throw new Error('connection lost')
      snapshot[id][name] = value
    }, refresh: async () => {} })
  await assert.rejects(move(['512', '515'], '514', 'inside'), /connection lost/)
  assert.deepEqual(snapshot['512'].Transform, transform(0, 10))
  assert.deepEqual(snapshot['515'].Transform, transform(0, 30))
})

import { createComponentBatch } from '../src/component-batch'
test('a failed interaction trigger restores prior actions and rejects the edit', async () => {
  const snapshot: Snapshot = { '512': { 'asset-packs::Actions': { id: 512, value: [] } } }
  const batch = createComponentBatch({ ready: async () => {}, snapshot: () => snapshot,
    write: async (id, name, value) => { if (name === 'asset-packs::Triggers') throw new Error('Invalid trigger'); snapshot[id][name] = value },
    remove: async (id, name) => { delete snapshot[id][name] }, refresh: async () => {} })
  await assert.rejects(batch('512', [{ name: 'asset-packs::Actions', value: { id: 512, value: [{ name: 'new action' }] } }, { name: 'asset-packs::Triggers', value: { value: [] } }]), /Invalid trigger/)
  assert.deepEqual(snapshot['512'], { 'asset-packs::Actions': { id: 512, value: [] } })
})
test('batch validation runs before any component writes', async () => {
  let writes = 0
  const batch = createComponentBatch({ ready: async () => {}, snapshot: scene, write: async () => { writes++ }, remove: async () => {}, refresh: async () => {} })
  await assert.rejects(batch('512', [{ name: 'Transform', value: {} }, { name: 'Transform', value: {} }]), /Invalid component changes/)
  await assert.rejects(batch('1', [{ name: 'Transform', value: {} }]), /authored entity/)
  assert.equal(writes, 0)
})

import { cmd } from '../src/cmd'
import { customTimestamp, decodeCustomComponents, encodeCustomComponent, customComponentId } from '../src/custom-components'
import { writeComponent } from '../src/inspector/mutate'
import { deleteComponent } from '../src/inspector/component-ops'
test('custom writes advance clocks only after acknowledgment and survive deletion and reload', async () => {
  const originalWrite = cmd.setComponentRaw
  const originalDelete = cmd.deleteComponent
  const timestamps: number[] = []
  const name = 'asset-packs::Counter'
  const entity = '9102'
  cmd.setComponentRaw = async (_entity, _component, timestamp) => { timestamps.push(timestamp); return 'set' }
  cmd.deleteComponent = async (_entity, component) => { assert.match(component, /^\d+$/); return 'removed' }
  try {
    await writeComponent(entity, name, JSON.stringify({ id: 9102, value: 7 }))
    await writeComponent(entity, name, JSON.stringify({ id: 9102, value: 8 }))
    assert.deepEqual(timestamps, [1, 2])
    cmd.setComponentRaw = async () => { throw new Error('write rejected') }
    await assert.rejects(writeComponent(entity, name, JSON.stringify({ id: 9102, value: 9 })), /write rejected/)
    assert.equal(customTimestamp(entity, name), 2)
    assert.equal((state.snapshot[entity][name] as { value: number }).value, 8)
    await deleteComponent(entity, name)
    decodeCustomComponents({})
    assert.equal(customTimestamp(entity, name), 3)
    cmd.setComponentRaw = async (_entity, _component, timestamp) => { timestamps.push(timestamp); return 'set' }
    await writeComponent(entity, name, JSON.stringify({ id: 9102, value: 8 }))
    assert.deepEqual(timestamps, [1, 2, 4])
  } finally {
    cmd.setComponentRaw = originalWrite
    cmd.deleteComponent = originalDelete
  }
})

import { createMaterialBatch } from '../src/material-batch'

test('material selection updates are acknowledged together and restore earlier entities after failure', async () => {
  const value = (roughness: number) => ({ material: { pbr: { roughness } } })
  const snapshot: Snapshot = { '512': { Material: value(0.2) }, '513': { Material: value(0.8) } }
  let writes = 0
  let refreshes = 0
  let rejectSecond = true
  const apply = createMaterialBatch({ ready: async () => {}, snapshot: () => snapshot,
    write: async (id, name, material) => { writes++; if (id === '513' && rejectSecond) throw new Error('engine refused'); snapshot[id][name] = material },
    remove: async () => { throw new Error('must not remove existing material') }, refresh: async () => { refreshes++ } })
  const changes = ['512', '513'].map(entity => ({ entity, name: 'Material', before: snapshot[entity].Material, value: value(0.5) }))
  await assert.rejects(apply(changes), /engine refused/)
  assert.deepEqual(snapshot, { '512': { Material: value(0.2) }, '513': { Material: value(0.8) } })
  assert.equal(writes, 3)
  assert.equal(refreshes, 0)
  rejectSecond = false
  assert.deepEqual(await apply(changes), ['512', '513'])
  assert.deepEqual(snapshot['513'].Material, value(0.5))
  assert.equal(refreshes, 1)
  await assert.rejects(apply(changes), /changed/)
  assert.equal(writes, 5, 'stale selection is validated before writing any entity')
})

test('material batch rejects missing or duplicate targets before writes and prevents concurrent apply', async () => {
  const material = { material: { pbr: { roughness: 0.5 } } }
  const snapshot: Snapshot = { '512': { Material: material }, '513': { Material: material } }
  let release!: () => void
  let writes = 0
  let hold = true
  const apply = createMaterialBatch({ ready: async () => {}, snapshot: () => snapshot,
    write: async () => { writes++; if (hold) { hold = false; await new Promise<void>(resolve => { release = resolve }) } }, remove: async () => {}, refresh: async () => {} })
  const changes = ['512', '513'].map(entity => ({ entity, name: 'Material', before: material, value: material }))
  await assert.rejects(apply([changes[0], changes[0]]), /Invalid material selection/)
  await assert.rejects(apply([changes[0], { ...changes[1], entity: '514' }]), /must still have/)
  assert.equal(writes, 0)
  const pending = apply(changes)
  await Promise.resolve()
  await Promise.resolve()
  await assert.rejects(apply(changes), /current material edit/)
  release()
  await pending
  assert.equal(writes, 2)
})

test('material batch never rolls old values into a replacement runtime', async () => {
  const old = { material: { pbr: { roughness: 0.2 } } }
  const next = { material: { pbr: { roughness: 0.5 } } }
  const replacement = { material: { pbr: { roughness: 0.9 } } }
  const snapshot: Snapshot = { '512': { Material: old }, '513': { Material: old } }
  let instance = 'original'
  const writes: string[] = []
  const apply = createMaterialBatch({ ready: async () => {}, snapshot: () => snapshot, context: async () => instance,
    write: async (id, name, value) => { writes.push(id); snapshot[id][name] = value; instance = 'replacement'; snapshot['512'].Material = replacement },
    remove: async () => {}, refresh: async () => {} })
  await assert.rejects(apply(['512', '513'].map(entity => ({ entity, name: 'Material', before: old, value: next }))), /could not be restored/)
  assert.deepEqual(writes, ['512'])
  assert.deepEqual(snapshot['512'].Material, replacement)
})

test('material refresh failure rolls acknowledged values back when the runtime remains current', async () => {
  const old = { material: { pbr: { roughness: 0.2 } } }
  const next = { material: { pbr: { roughness: 0.5 } } }
  const snapshot: Snapshot = { '512': { Material: old }, '513': { Material: old } }
  const writes: string[] = []
  const apply = createMaterialBatch({ ready: async () => {}, snapshot: () => snapshot, context: async () => 'same',
    write: async (id, name, value) => { writes.push(id); snapshot[id][name] = value }, remove: async () => {},
    refresh: async () => { throw new Error('refresh failed') } })
  await assert.rejects(apply(['512', '513'].map(entity => ({ entity, name: 'Material', before: old, value: next }))), /refresh failed/)
  assert.deepEqual(writes, ['512', '513', '513', '512'])
  assert.deepEqual(snapshot['512'].Material, old)
  assert.deepEqual(snapshot['513'].Material, old)
})


test('scene metadata uses the canonical schema and preserves settings through raw CRDT and composite export', () => {
  const name = 'inspector::SceneMetadata-v3'
  const value = { name: 'Winit Garden', description: 'Keep this', categories: ['art'],
    layout: { base: { x: 2, y: -3 }, parcels: [{ x: 2, y: -3 }] },
    spawnPoints: [{ name: 'Entrance', default: true, position: {
      x: { $case: 'single', value: 8 }, y: { $case: 'single', value: 0 }, z: { $case: 'range', value: [2, 4] }
    } }], silenceVoiceChat: true, skyboxConfig: { fixedTime: 3600 }, creator: 'Creator' }
  const id = 2492069143
  assert.equal(customComponentId(name), id)
  const encoded = encodeCustomComponent(name, value)
  assert.ok(encoded)
  const snapshot: Snapshot = { '0': { [id]: `1:${encoded}` } }
  decodeCustomComponents(snapshot)
  assert.deepEqual(snapshot['0'][name], value)
  const composite = JSON.parse(buildComposite(snapshot))
  assert.deepEqual(composite.components.find((component: { name: string }) => component.name === name).data['0'].json, value)
})
