import assert from 'node:assert/strict'
import { createProjectRead } from '../src/project-read'
import { test } from 'node:test'
import { createRuntimeReset, runtimeStatus, type ResetDependencies } from '../src/runtime-reset'
import { makeCommands } from '../src/commands'
import { createRuntimeMonitor, replacementProjectScene } from '../src/runtime-monitor'

const stats = (instance: string, tick: number, inFlight = false, broken = false) =>
  `hash: project\ninstance: ${instance}\ntick: ${tick}\nin_flight: ${inFlight}\nbroken: ${broken}`
const composite = JSON.stringify({ components: [] })

function fixture(replies: Array<string | Error>, overrides: Partial<ResetDependencies> = {}) {
  let time = 0
  const calls: unknown[] = []
  const reset = createRuntimeReset({
    stats: async () => {
      const reply = replies.length > 1 ? replies.shift()! : replies[0]
      if (reply instanceof Error) throw reply
      return reply
    },
    reload: async (hash) => { calls.push(['reload', hash]) },
    pin: async (hash) => { calls.push(['pin', hash]) },
    freeze: async () => { calls.push('freeze') },
    restore: async (value) => { calls.push(['restore', value]) },
    finish: async () => { calls.push('finish') },
    sleep: async (ms) => { time += ms },
    now: () => time,
    ...overrides
  })
  return { reset, calls }
}

test('Stop waits for a different runtime, completed main, and settled freeze before restoring authoring', async () => {
  const { reset, calls } = fixture([
    stats('old', 500), stats('old', 510), new Error('scene not found'),
    stats('new', 0), stats('new', 1), stats('new', 2, true), stats('new', 3, true), stats('new', 3)
  ])
  await reset('project', composite)
  assert.deepEqual(calls.filter((call) => !Array.isArray(call) || call[0] !== 'pin'), [
    ['reload', 'project'], 'freeze', ['restore', composite], 'finish'
  ])
  assert.equal(calls.filter((call) => Array.isArray(call) && call[0] === 'pin').length, 8)
})

test('an unchanged runtime cannot produce a successful Stop', async () => {
  const { reset, calls } = fixture([stats('old', 5)])
  await assert.rejects(reset('project', composite), /Timed out/)
  assert.ok(!calls.includes('finish'))
})

test('Stop never freezes a restarted scene before its first main update', async () => {
  const { reset, calls } = fixture([stats('old', 5), stats('new', 2)], {
    freeze: async () => { throw new Error('froze before main') }
  })
  await assert.rejects(reset('project', composite), /Timed out/)
  assert.ok(!calls.includes('finish'))
})

test('older engines and malformed composites fail before destroying runtime state', async () => {
  const older = fixture(['hash: project\ntick: 5'])
  await assert.rejects(older.reset('project', composite), /instance identity/)
  assert.equal(older.calls.length, 1)
  const invalid = fixture([stats('old', 5)])
  await assert.rejects(invalid.reset('project', '{}'), /composite/)
  assert.deepEqual(invalid.calls, [])
})

test('broken runtimes and failed authored-state restoration never acknowledge Stop', async () => {
  const broken = fixture([stats('old', 5), stats('new', 0, false, true)])
  await assert.rejects(broken.reset('project', composite), /failed to initialize/)
  assert.ok(!broken.calls.includes('finish'))
  const failed = fixture([stats('old', 5), stats('new', 3)], {
    restore: async () => { throw new Error('restore rejected') }
  })
  await assert.rejects(failed.reset('project', composite), /restore rejected/)
  assert.ok(!failed.calls.includes('finish'))
})

test('scene stats preserve full-width runtime identity and explicit entity allocation preserves authored references', async () => {
  assert.equal(runtimeStatus(stats('18446744073709551615', 1)).instance, '18446744073709551615')
  const commands = makeCommands(async (command, args) => {
    assert.equal(command, 'new_entity')
    assert.deepEqual(args, ['500', 'AA==', '2', '--ids', '512,513'])
    return '[512,513]'
  })
  assert.deepEqual(await commands.newEntity(500, 'AA==', 2, [512, 513]), [512, 513])
})

test('SDK watcher replacements refresh after startup and retry failed refreshes', async () => {
  let current = stats('old', 5), refreshes = 0
  const monitor = createRuntimeMonitor({ stats: async () => current, changed: async () => { if (++refreshes === 1) throw new Error('not ready'); } })
  await monitor('project')
  current = stats('new', 1)
  await monitor('project')
  assert.equal(refreshes, 0)
  current = stats('new', 2)
  await monitor('project')
  assert.equal(refreshes, 0, 'onStart alone must not freeze main() out of the replacement runtime')
  current = stats('new', 3, true)
  await monitor('project')
  assert.equal(refreshes, 0, 'wait for main() to finish responding')
  current = stats('new', 3)
  await assert.rejects(monitor('project'), /not ready/)
  await monitor('project')
  await monitor('project')
  assert.equal(refreshes, 2)
})

test('intentional Stop acknowledges its new runtime without a recovery handshake', async () => {
  let current = stats('old', 5), refreshes = 0
  const monitor = createRuntimeMonitor({ stats: async () => current, changed: async () => { refreshes++ } })
  await monitor('project')
  current = stats('stopped', 3)
  monitor.accept(current)
  await monitor('project')
  assert.equal(refreshes, 0)
  current = stats('watcher-rebuilt', 3).replace('hash: project', 'hash: rebuilt-content')
  await monitor('rebuilt-content')
  assert.equal(refreshes, 1)
})

test('a watcher content-hash change rebinds only the same unambiguous project parcels', () => {
  const original = { hash: 'old', title: 'Project', parcels: [{ x: 0, y: 0 }], isPortable: false, isSuper: false, isBroken: false, isBlocked: true, sdkVersion: '7' }
  const replacement = { ...original, hash: 'new' }
  assert.equal(replacementProjectScene(original, [replacement]), replacement)
  assert.equal(replacementProjectScene(original, [{ ...replacement, isSuper: true }]), undefined)
  assert.equal(replacementProjectScene(original, [{ ...replacement, parcels: [{ x: 1, y: 0 }] }]), undefined)
  assert.equal(replacementProjectScene(original, [replacement, { ...replacement, hash: 'ambiguous' }]), undefined)
  assert.equal(replacementProjectScene(original, [original, replacement]), original)
})

test('initial snapshot recovery waits for the project runtime and retries transient failures', async () => {
  let current = stats('initial', 0), refreshes = 0, needsRecovery = true
  const monitor = createRuntimeMonitor({
    stats: async () => current,
    needsRecovery: () => needsRecovery,
    changed: async () => {
      if (++refreshes === 1) throw new Error('snapshot not available yet')
      needsRecovery = false
    }
  })
  await monitor('project')
  current = stats('initial', 1)
  await monitor('project')
  current = stats('initial', 2).replace('hash: project', 'hash: unrelated')
  await monitor('project')
  assert.equal(refreshes, 0)
  current = stats('initial', 3)
  await assert.rejects(monitor('project'), /snapshot not available yet/)
  assert.equal(needsRecovery, true)
  await monitor('project')
  await monitor('project')
  assert.equal(refreshes, 2)
  assert.equal(needsRecovery, false)
})

test('broken runtimes and persistent snapshot failures never become recovered', async () => {
  let current = stats('initial', 0, false, true), refreshes = 0
  const monitor = createRuntimeMonitor({
    stats: async () => current,
    needsRecovery: () => true,
    changed: async () => { refreshes++; throw new Error('snapshot rejected') }
  })
  await assert.rejects(monitor('project'), /failed to initialize/)
  assert.equal(refreshes, 0)
  current = stats('initial', 3)
  await assert.rejects(monitor('project'), /snapshot rejected/)
  await assert.rejects(monitor('project'), /snapshot rejected/)
  assert.equal(refreshes, 2)
})

test('project export waits across watcher replacement and never returns a mixed runtime snapshot', async () => {
  let time = 0, reads = 0, refreshes = 0
  const states = [new Error('active inspection scene no longer exists; use /set_scene to update'), stats('new', 1), stats('new', 2), stats('new', 3), stats('newer', 3), stats('newer', 3), stats('newer', 4)]
  const read = createProjectRead({
    stats: async () => { const next = states.shift(); if (next instanceof Error) throw next; assert.equal(typeof next, 'string'); return next! },
    refresh: async () => { refreshes++ },
    sleep: async ms => { time += ms }, now: () => time
  })
  assert.equal(await read('project', async () => ++reads), 2)
  assert.equal(refreshes, 2)
  assert.equal(time, 400)
})

test('project export reports persistent reload failure and unrelated errors without exporting', async () => {
  let time = 0, reads = 0
  const deps = {
    stats: async (): Promise<string> => { throw new Error('active inspection scene no longer exists; use /set_scene to update') },
    refresh: async () => {}, sleep: async (ms: number) => { time += ms }, now: () => time
  }
  await assert.rejects(createProjectRead(deps)('project', async () => ++reads), /Timed out.*no project data was exported/)
  assert.equal(reads, 0)
  await assert.rejects(createProjectRead({ ...deps, stats: async () => { throw new Error('permission denied') } })('project', async () => ++reads), /permission denied/)
  assert.equal(reads, 0)
})
