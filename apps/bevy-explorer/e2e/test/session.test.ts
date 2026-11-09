import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { ConsoleError, FakeConsole } from '../driver/console.ts'
import { Explorer, type WorldState } from '../driver/explorer.ts'
import { type AuthIdentity, addressOf } from '../driver/identity.ts'
import { MAX_BOOTS, arrive, closeSessions, keepBootFailure, openSession, parsePosition, sameRealm, signIn } from '../driver/session.ts'
import type { Target } from '../driver/target.ts'
import { BootError } from '../driver/web.ts'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'

describe('keepBootFailure', () => {
  test("keeps the page's console with its credentials replaced", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'e2e-boot-'))
    try {
      const error = new BootError('the console never came', ['booting', 'connecting to livekit:wss://rooms.example.test/r?access_token=tok3n'], undefined)
      await keepBootFailure(1, error, dir)
      const kept = await readFile(path.join(dir, 'boot-1.console.txt'), 'utf8')
      assert.ok(kept.includes('access_token=REDACTED'), kept)
      assert.ok(!kept.includes('tok3n'), kept)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })
})

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

describe('signIn', () => {
  const account = '0x' + '11'.repeat(32)
  const web: Target = { ...target, transport: 'web', accounts: [account] }
  const landed = '{"address":"0xabc","guest":false,"phase":"entering"}'
  /** The identity a `/hud_login` line carries. */
  const identityOf = (line: string): AuthIdentity => JSON.parse(Buffer.from(line.split(' ')[1]!, 'base64').toString('utf8')) as AuthIdentity

  test('on the web signs in through the HUD with the account, the pick and the destination, then waits for the world', async () => {
    let reads = 0
    const fake = new FakeConsole({ hud_login: landed, world_state: () => world({ inWorld: ++reads > 1 }) })
    const state = await signIn(new Explorer(fake), web, 'account', { name: 'tester', look: 2 }, { realm: 'https://zone.example.test', position: '100,100' })
    assert.equal(state.inWorld, true)
    const [line] = fake.sent('hud_login')
    assert.match(line!, /^\/hud_login \S+ --name tester --look 2 --realm https:\/\/zone\.example\.test --position 100,100$/)
    assert.deepEqual(fake.sent('login_identity'), [])
    assert.deepEqual(fake.sent('hud_click'), [])
    assert.equal(identityOf(line!).authChain[0]!.payload, addressOf(account))
  })

  test("a web guest is a fresh account marked as one, bound for the target's realm and parcel", async () => {
    const fake = new FakeConsole({ hud_login: landed, world_state: () => world() })
    await signIn(new Explorer(fake), web, 'guest')
    const [line] = fake.sent('hud_login')
    assert.match(line!, /^\/hud_login \S+ --name e2etester --look 1 --guest --realm https:\/\/realm\.example\.test --position 0,0$/)
    assert.notEqual(identityOf(line!).authChain[0]!.payload, addressOf(account))
    assert.deepEqual(fake.sent('login_guest'), [])
  })

  test('a web unsigned guest is signed in by the page (?guest=1): no sign-in command, only the wait for the world', async () => {
    const fake = new FakeConsole({ hud_login: landed, login_guest: '', world_state: () => world() })
    await signIn(new Explorer(fake), web, 'unsigned-guest')
    assert.deepEqual(fake.sent('hud_login'), [])
    assert.deepEqual(fake.sent('login_guest'), [])
    assert.deepEqual(fake.sent('world_state'), ['/world_state'])
  })

  test('on the standalone an unsigned guest is the engine\'s /login_guest', async () => {
    const fake = new FakeConsole({ login_guest: '', world_state: () => world() })
    await signIn(new Explorer(fake), target, 'unsigned-guest')
    assert.deepEqual(fake.sent('login_guest'), ['/login_guest'])
  })

  test("a web sign-in the HUD refuses fails the boot with the HUD's reason, the identity redacted", async () => {
    const fake = new FakeConsole({
      hud_login: (line) => {
        throw new ConsoleError(line, 'the account has no profile yet: a first login needs --name and --look')
      },
      world_state: () => world()
    })
    await assert.rejects(signIn(new Explorer(fake), web, 'account'), (error: unknown) => {
      assert.ok(error instanceof ConsoleError)
      assert.match(error.message, /^`\/hud_login <redacted> --name e2etester --look 1 --realm \S+ --position 0,0` failed: the account has no profile yet/)
      return true
    })
    assert.deepEqual(fake.sent('world_state'), [])
  })

  test('a destination the HUD did not take fails the boot at once, not after the world\'s wait', async () => {
    const fake = new FakeConsole({ hud_login: '{"address":"0xabc","guest":false,"phase":"lobby"}', world_state: () => world() })
    await assert.rejects(signIn(new Explorer(fake), web, 'account'), /the HUD did not take the destination https:\/\/realm\.example\.test at 0,0: it answered the sign-in in phase lobby/)
    assert.deepEqual(fake.sent('world_state'), [])
  })

  test('a web app whose HUD has no /hud_login fails the boot saying so, with no fall back to the engine', async () => {
    const fake = new FakeConsole({ login_identity: 'logged in with identity', world_state: () => world() })
    await assert.rejects(signIn(new Explorer(fake), web, 'account'), /the web app under test has no \/hud_login/)
    assert.deepEqual(fake.sent('login_identity'), [])
  })

  test('on the standalone the engine signs in, already at its realm', async () => {
    const fake = new FakeConsole({ login_identity: 'logged in with identity', login_guest: '', world_state: () => world() })
    await signIn(new Explorer(fake), { ...target, accounts: [account] }, 'account')
    assert.match(fake.sent('login_identity')[0]!, /^\/login_identity \S+ --name e2etester --look 1$/)
    assert.deepEqual(fake.sent('hud_login'), [])
    await signIn(new Explorer(fake), target, 'guest')
    assert.deepEqual(fake.sent('login_guest'), ['/login_guest'])
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
