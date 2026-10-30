import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createPlaybackController, type PlaybackDependencies } from '../src/playback-controller.ts'

function fixture(overrides: Partial<PlaybackDependencies> = {}) {
  const calls: unknown[] = []
  const playback = createPlaybackController({
    pin: async () => { calls.push('pin'); return true },
    freeze: async () => { calls.push('freeze') },
    unfreeze: async () => { calls.push('unfreeze') },
    visuals: async (playing, paused) => { calls.push(['visuals', playing, paused]) },
    commit: (playing, paused) => { calls.push(['commit', playing, paused]) },
    ...overrides
  })
  return { calls, playback }
}

test('Play, Pause, Resume and Stop select the project before changing simulation and input', async () => {
  const { calls, playback } = fixture()
  for (const [playing, paused] of [[true, false], [true, true], [true, false], [false, false]]) {
    calls.length = 0
    await playback(playing, paused)
    assert.deepEqual(calls, ['pin', paused ? 'freeze' : 'unfreeze', ['visuals', playing, paused], ['commit', playing, paused]])
  }
})

test('engine rejection cannot commit a successful camera/input handoff', async () => {
  const { calls, playback } = fixture({ freeze: async () => { throw new Error('engine unavailable') } })
  await assert.rejects(playback(true, true), /engine unavailable/)
  assert.deepEqual(calls, ['pin'])
})

test('failed scene selection never freezes another scene', async () => {
  const { calls, playback } = fixture({ pin: async () => false })
  await assert.rejects(playback(true, true), /could not be selected/)
  assert.deepEqual(calls, [])
})

test('already frozen/running replies are idempotent, but visual failure is reported', async () => {
  const { calls, playback } = fixture({
    freeze: async () => { throw new Error('scene is already frozen') },
    unfreeze: async () => { throw new Error('scene is not frozen') }
  })
  await playback(true, true)
  await playback(true, false)
  assert.equal(calls.filter((entry) => Array.isArray(entry) && entry[0] === 'commit').length, 2)
  const failed = fixture({ visuals: async () => { throw new Error('animation restore failed') } })
  await assert.rejects(failed.playback(true, false), /animation restore failed/)
  assert.deepEqual(failed.calls, ['pin', 'unfreeze'])
})
