import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createAuthoringCheckpoint, AUTHORING_RPCS, AUTHORING_MESSAGES } from '../src/authoring-checkpoint'
import { runRpcOperation } from '../src/rpc-operation'
import { prepareDragTransition } from '../src/drag-notification'

const composite = JSON.stringify({ components: [{ name: 'core::Name', data: { 512: { value: 'Peer edit' } } }] })
function fixture() {
  let instance = '1', playing = false, broken = false, tick = 3, inFlight = false
  const events: string[] = []
  const snapshots: { composite: string; runtime: string }[] = []
  const deps = {
    project: () => ({ hash: 'project', playing }),
    stats: async () => `hash: project\ninstance: ${instance}\ntick: ${tick}\nbroken: ${broken}\nin_flight: ${inFlight}`,
    flush: async () => { events.push('flush') },
    read: async () => { events.push('export'); return composite },
    send: (value: { composite: string; runtime: string }) => { events.push('checkpoint'); snapshots.push(value) }
  }
  return { deps, events, snapshots, replace: () => { instance = '2' }, play: () => { playing = true },
    break: () => { broken = true }, startup: () => { tick = 2 }, busy: () => { inFlight = true } }
}

test('authoritative export is handed off before successful mutation reply with no delay', async () => {
  const f = fixture()
  const capture = createAuthoringCheckpoint(f.deps)
  await runRpcOperation(() => capture(async () => { f.events.push('edit'); return 42 }), null, outcome => {
    assert.deepEqual(outcome, { ok: true, result: 42 })
    f.events.push('reply')
    f.replace() // peer PUT may replace the engine immediately after ACK
  })
  assert.deepEqual(f.events, ['edit', 'flush', 'export', 'checkpoint', 'reply'])
  assert.deepEqual(f.snapshots, [{ composite, runtime: 'project:1' }])
})

test('gizmo completion cannot export or acknowledge before renderer writes finish', async () => {
  const f = fixture()
  let release!: () => void
  f.deps.flush = () => new Promise<void>(resolve => { release = resolve })
  const pending = createAuthoringCheckpoint(f.deps)(async () => {})
  await new Promise(resolve => setImmediate(resolve))
  assert.deepEqual(f.snapshots, [])
  assert.deepEqual(f.events, [])
  release()
  await pending
  assert.deepEqual(f.events, ['export', 'checkpoint'])
})

for (const phase of ['edit', 'flush', 'export'] as const) {
  test(`runtime replacement during ${phase} fails without checkpoint or successful reply`, async () => {
    const f = fixture()
    if (phase === 'flush') f.deps.flush = async () => f.replace()
    if (phase === 'export') f.deps.read = async () => { f.replace(); return composite }
    await assert.rejects(createAuthoringCheckpoint(f.deps)(async () => { if (phase === 'edit') f.replace() }), /changed during the edit/)
    assert.equal(f.snapshots.length, 0)
  })
}

for (const state of ['play', 'break', 'startup', 'busy'] as const) {
  test(`${state} runtime cannot begin authored mutation`, async () => {
    const f = fixture()
    f[state]()
    await assert.rejects(createAuthoringCheckpoint(f.deps)(async () => { f.events.push('edit') }))
    assert.deepEqual(f.events, [])
  })
}

test('failed mutation, failed renderer flush and malformed exports cannot checkpoint', async () => {
  for (const phase of ['edit', 'flush', 'export']) {
    const f = fixture()
    if (phase === 'flush') f.deps.flush = async () => { throw new Error('renderer failed') }
    if (phase === 'export') f.deps.read = async () => '{}'
    await assert.rejects(createAuthoringCheckpoint(f.deps)(async () => { if (phase === 'edit') throw new Error('edit failed') }))
    assert.equal(f.snapshots.length, 0)
  }
})

test('Play transition during export cannot checkpoint simulation', async () => {
  const f = fixture()
  f.deps.read = async () => { f.play(); return composite }
  await assert.rejects(createAuthoringCheckpoint(f.deps)(async () => {}), /changed during the edit/)
  assert.equal(f.snapshots.length, 0)
})

test('mutation traffic is covered without treating hydration, reads or playback as edits', () => {
  assert.equal(AUTHORING_RPCS.has('writeComponents'), true)
  assert.equal(AUTHORING_RPCS.has('addEntity'), true)
  for (const method of ['restoreComposite', 'exportComposite', 'setPlayback', 'stopPlayback', 'sceneStats']) {
    assert.equal(AUTHORING_RPCS.has(method), false)
  }
  for (const message of ['set-component', 'add-component', 'delete-component', 'add-entity', 'entity-deleted', 'load-scene', 'component-written']) {
    assert.equal(AUTHORING_MESSAGES.has(message), true)
  }
})

test('a new drag during checkpoint preserves the prior completion and immutable history payload', async () => {
  let dragging = false
  let release!: () => void
  let lastDragging = true
  const transform = { position: { x: 4, y: 1, z: 2 } }
  const pending = prepareDragTransition(dragging, false, { '512': transform }, () => new Promise<void>(resolve => { release = resolve }))
  dragging = true
  transform.position.x = 8
  release()
  const ended = await pending
  lastDragging = ended.dragging
  assert.equal(lastDragging, false)
  assert.deepEqual(ended.message, { type: 'drag-end', transforms: { '512': { position: { x: 4, y: 1, z: 2 } } } })
  assert.notEqual(dragging, lastDragging, 'the next tick must still report the new drag')
  const started = await prepareDragTransition(dragging, false, {}, async () => { assert.fail('drag-start must not checkpoint') })
  assert.equal(started.dragging, true)
  assert.deepEqual(started.message, { type: 'drag-start' })
})

test('failed drag checkpoint does not yield a completion, and Play completion cannot checkpoint simulation', async () => {
  let completed = false
  await assert.rejects(prepareDragTransition(false, false, {}, async () => { throw new Error('renderer write failed') })
    .then(() => { completed = true }), /renderer write failed/)
  assert.equal(completed, false)
  const playing = await prepareDragTransition(false, true, {}, async () => { assert.fail('Play must not checkpoint') })
  assert.deepEqual(playing, { dragging: false, message: { type: 'drag-end', transforms: {} } })
})
