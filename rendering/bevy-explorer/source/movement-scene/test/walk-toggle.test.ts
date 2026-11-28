import assert from 'node:assert/strict'
import { test } from 'node:test'
import { keysGait, toggleWalk } from '../src/horizontal'

type Tick = { walkPress?: boolean; sprint?: boolean }

// Runs the gait keys tick by tick as readInput() does: a walk key press on a tick, sprint held or not. Returns each
// tick's gait.
function gaits(ticks: Tick[]): string[] {
  let walkOn = false
  return ticks.map(({ walkPress = false, sprint = false }) => {
    walkOn = toggleWalk(walkOn, walkPress)
    return keysGait(walkOn, sprint)
  })
}

test('a press of the walk key turns walking on, and the next press turns it off', () => {
  assert.deepEqual(
    gaits([{}, { walkPress: true }, {}, {}, { walkPress: true }, {}]),
    ['jog', 'walk', 'walk', 'walk', 'jog', 'jog']
  )
})

test('walking stays on with no key held, through a stop, until the next press', () => {
  const ticks: Tick[] = [{ walkPress: true }, ...Array.from({ length: 600 }, () => ({}))]
  assert.ok(gaits(ticks).every((gait) => gait === 'walk'))
})

test('without walking the keys jog, and sprint runs', () => {
  assert.deepEqual(gaits([{}, { sprint: true }, {}]), ['jog', 'run', 'jog'])
})

test('sprint outranks walking while held, and walking comes back on its release', () => {
  assert.deepEqual(
    gaits([{ walkPress: true }, { sprint: true }, { sprint: true }, {}]),
    ['walk', 'run', 'run', 'walk']
  )
})

test('a walk key press during sprint still toggles walking for after it', () => {
  assert.deepEqual(
    gaits([{ sprint: true }, { sprint: true, walkPress: true }, {}, { sprint: true, walkPress: true }, {}]),
    ['run', 'run', 'walk', 'run', 'jog']
  )
})
