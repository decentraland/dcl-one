import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, test } from 'node:test'
import { FakeConsole } from '../driver/console.ts'
import { Explorer } from '../driver/explorer.ts'
import { FIRST_RING_UNSUPPORTED, type LoadingTimeResults, measureTeleport, measureTimeToWorld, parseConfig, slowLines, summaryMarkdown, teleportLegs, unmeasuredLines, waitFirstRingReady } from '../driver/loading-time.ts'
import { PACKAGE_ROOT } from '../driver/paths.ts'

function loading(firstRing: { arrival: number; sinceReadyMs?: number } | null): string {
  return JSON.stringify({ stage: 'Running', firstRing: firstRing && { sinceReadyMs: 0, ...firstRing } })
}

function world(arrivals: number): string {
  return JSON.stringify({ inWorld: true, arrivals, signedIn: true, address: null, guest: true, realm: 'main', realmConnected: true, worldHeld: false, outOfWorld: false, position: [8, 0, 8] })
}

const GENESIS = { name: 'Genesis Plaza', position: '0,0' }
const MUSEUM = { name: 'DCL Museum', position: '20,80' }

describe('loading time', () => {
  test('the shipped config parses', () => {
    const config = parseConfig(readFileSync(path.join(PACKAGE_ROOT, 'loading-time.json'), 'utf8'))
    assert.ok(config.locations.length >= 2)
  })

  test('a config without thresholds or with a bad position is refused', () => {
    const thresholds = { timeToWorldMs: 1, teleportMs: 1 }
    assert.throws(() => parseConfig(JSON.stringify({ locations: [GENESIS] })), /thresholds/)
    assert.throws(() => parseConfig(JSON.stringify({ thresholds, locations: [{ name: 'x', position: 'nowhere' }] })), /position/)
  })

  test('a client whose /scene_loading_state has no firstRing fails the wait at once', async () => {
    const explorer = new Explorer(new FakeConsole({ scene_loading_state: JSON.stringify({ stage: 'Running' }) }))
    const started = Date.now()
    await assert.rejects(waitFirstRingReady(explorer, { arrival: 1, what: 'the first ring', timeoutMs: 60_000 }), new RegExp(FIRST_RING_UNSUPPORTED))
    assert.ok(Date.now() - started < 5000)
  })

  test('the wait needs the first ring of the given arrival or a later one, and dates it by sinceReadyMs', async () => {
    const replies = [loading(null), loading({ arrival: 1 }), loading({ arrival: 2, sinceReadyMs: 1500 })]
    const explorer = new Explorer(new FakeConsole({ scene_loading_state: () => replies.shift() ?? replies.at(-1)! }))
    const before = Date.now()
    const readyAt = await waitFirstRingReady(explorer, { arrival: 2, what: 'the first ring', timeoutMs: 5000 })
    assert.equal(replies.length, 0)
    assert.ok(readyAt <= Date.now() - 1500 && readyAt >= before - 1500)
  })

  test('a client without firstRing fails at once even after a console error', async () => {
    let reads = 0
    const explorer = new Explorer(new FakeConsole({
      scene_loading_state: () => {
        if (++reads === 1) throw new Error('document gone')
        return JSON.stringify({ stage: 'Running' })
      }
    }))
    const started = Date.now()
    await assert.rejects(waitFirstRingReady(explorer, { arrival: 1, what: 'the first ring', timeoutMs: 60_000 }), new RegExp(FIRST_RING_UNSUPPORTED))
    assert.ok(Date.now() - started < 5000)
  })

  test('a console error on the first read is retried, not a failed reading', async () => {
    let reads = 0
    const explorer = new Explorer(new FakeConsole({
      scene_loading_state: () => {
        if (++reads === 1) throw new Error('document gone')
        return loading({ arrival: 1 })
      }
    }))
    await waitFirstRingReady(explorer, { arrival: 1, what: 'the first ring', timeoutMs: 5000 })
    assert.ok(reads >= 2)
  })

  test('time to world does not take the first ring of an arrival before the boot\'s last', async () => {
    const explorer = new Explorer(new FakeConsole({ world_state: world(2), scene_loading_state: loading({ arrival: 1 }) }))
    const { measurement } = await measureTimeToWorld(GENESIS, async () => explorer, { deadline: Date.now() + 1500 })
    assert.equal(measurement.timeToWorldMs, null)
    assert.match(measurement.error!, /first ring/)
  })

  test('time to world runs from before the launch to the first ring ready for the boot\'s arrival', async () => {
    const explorer = new Explorer(new FakeConsole({ world_state: world(2), scene_loading_state: loading({ arrival: 2, sinceReadyMs: 2_000 }) }))
    const before = Date.now()
    // the clock started 5 s before the launch; the reply says ready 2 s before it came
    const { measurement } = await measureTimeToWorld(GENESIS, async () => explorer, { now: () => before - 5_000 })
    const after = Date.now()
    assert.equal(measurement.error, undefined)
    assert.ok(measurement.timeToWorldMs! >= 3_000 && measurement.timeToWorldMs! <= after - before + 3_000)
  })

  test('a measurement past the deadline is skipped without a boot', async () => {
    let booted = false
    const { measurement } = await measureTimeToWorld(GENESIS, async () => {
      booted = true
      throw new Error('booted')
    }, { deadline: Date.now() - 1 })
    assert.equal(booted, false)
    assert.match(measurement.error!, /deadline/)
  })

  test('a failed boot is a measurement with its error, not a throw', async () => {
    const { measurement, explorer } = await measureTimeToWorld(GENESIS, async () => {
      throw new Error('no console')
    })
    assert.equal(explorer, undefined)
    assert.equal(measurement.timeToWorldMs, null)
    assert.match(measurement.error!, /boot: no console/)
  })

  test('a teleport is timed from the command to the first ring ready for its arrival, not the one before', async () => {
    let arrivals = 1
    let staleReads = 0
    const fake = new FakeConsole({
      world_state: () => world(arrivals),
      teleport: () => {
        arrivals = 2
        return 'new location'
      },
      // the previous arrival's first ring stays the latest for a few polls after the teleport
      scene_loading_state: () => loading({ arrival: arrivals === 2 && ++staleReads > 3 ? 2 : 1, sinceReadyMs: 2_000 })
    })
    // the clock reads 10 s before now when the teleport is sent; the reply says ready 2 s before it came
    const before = Date.now()
    const { measurement: leg, lost } = await measureTeleport(new Explorer(fake), GENESIS, MUSEUM, { now: () => before - 10_000 })
    const after = Date.now()
    assert.equal(leg.error, undefined)
    assert.equal(lost, false)
    assert.ok(staleReads > 3)
    assert.ok(leg.teleportMs! >= 8_000 && leg.teleportMs! <= after - before + 8_000)
    assert.deepEqual(fake.sent('teleport'), ['/teleport 20 80'])
  })

  test('a teleport whose arrival is never seen leaves the player lost; one whose first ring times out does not', async () => {
    const never = new FakeConsole({ world_state: world(1), teleport: 'new location', scene_loading_state: loading({ arrival: 1 }) })
    const unseen = await measureTeleport(new Explorer(never), GENESIS, MUSEUM, { waitMs: 500 })
    assert.equal(unseen.lost, true)
    assert.equal(unseen.measurement.teleportMs, null)

    let arrivals = 1
    const slow = new FakeConsole({
      world_state: () => world(arrivals),
      teleport: () => {
        arrivals = 2
        return 'new location'
      },
      scene_loading_state: loading({ arrival: 1 })
    })
    const timedOut = await measureTeleport(new Explorer(slow), GENESIS, MUSEUM, { waitMs: 500 })
    assert.equal(timedOut.lost, false)
    assert.match(timedOut.measurement.error!, /first ring/)
  })

  test('a teleport near the deadline is skipped, and one whose wait the deadline cuts is not lost', async () => {
    const fake = new FakeConsole({ world_state: world(1), teleport: 'new location', scene_loading_state: loading({ arrival: 1 }) })
    const deadline = 1_000_000
    const skipped = await measureTeleport(new Explorer(fake), GENESIS, MUSEUM, { deadline, now: () => deadline - 59_000 })
    assert.match(skipped.measurement.error!, /deadline passed/)
    assert.deepEqual(fake.sent('teleport'), [])

    // enough left at the start; 300 ms left once the teleport is sent
    const clock = [deadline - 61_000, deadline - 61_000]
    const cut = await measureTeleport(new Explorer(fake), GENESIS, MUSEUM, { deadline, now: () => clock.shift() ?? deadline - 300 })
    assert.equal(cut.lost, false)
    assert.match(cut.measurement.error!, /deadline cut the wait short/)
  })

  test('the tour starts from the last location and visits each once', () => {
    const third = { name: 'Third', position: '1,1' }
    assert.deepEqual(teleportLegs([GENESIS, MUSEUM, third]).map(([a, b]) => `${a.name}>${b.name}`), ['Third>Genesis Plaza', 'Genesis Plaza>DCL Museum', 'DCL Museum>Third'])
    assert.deepEqual(teleportLegs([GENESIS]), [])
  })

  test('slow lines name what exceeded its threshold, unmeasured lines what failed', () => {
    const results: LoadingTimeResults = {
      startedAt: '2026-11-24T00:00:00.000Z',
      webUrl: 'https://example.test/play',
      baseDomain: 'example.test',
      realm: 'main',
      thresholds: { timeToWorldMs: 60_000, teleportMs: 30_000 },
      worlds: [
        { location: 'Genesis Plaza', position: '0,0', timeToWorldMs: 50_000 },
        { location: 'DCL Museum', position: '20,80', timeToWorldMs: 70_000 }
      ],
      teleports: [{ from: 'DCL Museum', to: 'Genesis Plaza', teleportMs: null, error: 'timed out | waiting @someone' }]
    }
    const slow = slowLines(results)
    assert.equal(slow.length, 1)
    assert.match(slow[0]!, /DCL Museum: 70\.0 s > 60\.0 s/)
    assert.deepEqual(unmeasuredLines(results), ['teleport DCL Museum → Genesis Plaza: `timed out | waiting @someone`'])
    const summary = summaryMarkdown(results, 'https://example.test/run')
    assert.match(summary, /\| Genesis Plaza \(0,0\) \| 50\.0 s \| 60\.0 s \|/)
    assert.match(summary, /failed: `timed out \\\| waiting @someone`/)
    assert.match(summary, /Over the threshold:/)
    assert.match(summary, /Not measured:/)
    assert.equal(slowLines({ ...results, worlds: [results.worlds[0]!], teleports: [] }).length, 0)
  })
})
