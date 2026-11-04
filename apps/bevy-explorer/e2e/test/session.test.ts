import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { FakeConsole } from '../driver/console.ts'
import { Explorer, type WorldState } from '../driver/explorer.ts'
import { MAX_BOOTS, arrive, closeSessions, openSession, parsePosition, sameRealm } from '../driver/session.ts'
import type { Target } from '../driver/target.ts'

function world(overrides: Partial<WorldState> = {}): string {
  return JSON.stringify({
    inWorld: true,
    arrivals: 1,
    signedIn: true,
    address: '0x' + '1'.repeat(40),
    guest: false,
    realm: 'https://peer.example.test',
    realmConnected: true,
    worldHeld: false,
    outOfWorld: false,
    position: [8, 0, 8],
    ...overrides
  })
}

/** A native target whose client cannot be started: every boot fails at once. */
const target: Target = {
  baseDomain: 'example.test',
  realm: 'https://realm.example.test',
  position: '0,0',
  transport: 'native',
  webApp: '',
  headed: false,
  windowSize: [800, 600],
  accounts: [],
  worker: 0,
  ports: { web: 0, realm: 0 },
  record: false,
  native: { bin: 'no-such-client-binary', port: 1, args: [] }
}

describe('arrive', () => {
  test('parses a parcel and compares realms by name', () => {
    assert.deepEqual(parsePosition(' -3, 12 '), [-3, 12])
    assert.throws(() => parsePosition('100'), /a position is "x,y"/)
    assert.equal(sameRealm('https://peer.example.test/', 'HTTPS://peer.example.test'), true)
    assert.equal(sameRealm('https://peer.example.test', 'https://other.example.test'), false)
  })

  test('stays put when the player already stands in the parcel of the realm', async () => {
    const fake = new FakeConsole({ world_state: () => world({ position: [8, 0, 8] }) })
    const state = await arrive(new Explorer(fake), 'https://peer.example.test/', '0,0')
    assert.equal(state.inWorld, true)
    assert.deepEqual(fake.sent('teleport'), [])
    assert.deepEqual(fake.sent('changerealm'), [])
  })

  test('teleports to the parcel in the same realm', async () => {
    let arrivals = 1
    const fake = new FakeConsole({
      world_state: () => world({ arrivals, position: arrivals === 1 ? [8, 0, 8] : [1608, 0, 1608] }),
      teleport: () => {
        arrivals = 2
        return 'new location: (100, 100)'
      }
    })
    const state = await arrive(new Explorer(fake), 'https://peer.example.test', '100,100')
    assert.equal(state.arrivals, 2)
    assert.deepEqual(fake.sent('teleport'), ['/teleport 100 100'])
    assert.deepEqual(fake.sent('changerealm'), [])
  })

  test('changes realm first when the player is in another', async () => {
    let arrivals = 1
    let realm = 'https://peer.example.test'
    const fake = new FakeConsole({
      world_state: () => world({ arrivals, realm, position: [8, 0, 8] }),
      changerealm: (line) => {
        realm = line.slice('/changerealm '.length)
        arrivals = 2
        return 'realm changed'
      },
      teleport: () => {
        arrivals = 3
        return 'new location: (100, 100)'
      }
    })
    const state = await arrive(new Explorer(fake), 'https://zone.example.test', '100,100')
    assert.equal(state.arrivals, 3)
    assert.deepEqual(fake.sent('changerealm'), ['/changerealm https://zone.example.test'])
    assert.deepEqual(fake.sent('teleport'), ['/teleport 100 100'])
  })
})

describe('openSession', () => {
  test('boots again after a failed boot, then fails at once and says so as it happens', async () => {
    const lines: string[] = []
    const original = console.error
    console.error = (line: unknown) => lines.push(String(line))
    try {
      for (let boot = 1; boot <= MAX_BOOTS; boot++) {
        await assert.rejects(openSession({}, target), /could not be started/)
        assert.equal(lines.filter((line) => line.includes(`boot ${boot} of ${MAX_BOOTS} failed: the client no-such-client-binary could not be started`)).length, 1)
      }
      const started = Date.now()
      await assert.rejects(openSession({}, target), /failed to boot 2 times; not booting it again in this run: the client no-such-client-binary could not be started/)
      assert.ok(Date.now() - started < 1000, 'the refusal should not wait on anything')
      assert.equal(lines.filter((line) => line.includes('failed:')).length, MAX_BOOTS, 'a refused boot logs nothing new')
    } finally {
      console.error = original
      await closeSessions()
    }
  })
})
