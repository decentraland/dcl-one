import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { ConsoleError, FakeConsole } from '../driver/console.ts'
import { type CommandRecord, Explorer, type WorldState, redactLine } from '../driver/explorer.ts'
import { enterWorld } from '../driver/session.ts'

function world(overrides: Partial<WorldState> = {}): string {
  return JSON.stringify({
    inWorld: true,
    arrivals: 1,
    signedIn: true,
    address: '0x' + '1'.repeat(40),
    guest: false,
    realm: 'https://peer.example',
    realmConnected: true,
    worldHeld: false,
    outOfWorld: false,
    position: [8, 0, 8],
    ...overrides
  })
}

const control = (extra: Record<string, unknown> = {}) =>
  JSON.stringify({ found: true, role: 'button', name: 'Backpack', text: 'Backpack', disabled: false, visible: true, actionable: true, x: 10, y: 20, width: 40, height: 30, ...extra })

describe('Explorer', () => {
  test('teleport waits for a new arrival in the world', async () => {
    let arrivals = 1
    let teleported = false
    let reads = 0
    const fake = new FakeConsole({
      world_state: () => {
        reads++
        if (teleported && reads > 2) arrivals = 2
        return world({ arrivals, inWorld: arrivals === 2 || !teleported })
      },
      teleport: (line) => {
        teleported = true
        return `new location: ${line.slice(10)}`
      }
    })
    const explorer = new Explorer(fake)
    const state = await explorer.teleport(10, 20, { timeoutMs: 5000 })
    assert.equal(state.arrivals, 2)
    assert.deepEqual(fake.sent('teleport'), ['/teleport 10 20'])
    assert.ok(fake.sent('world_state').length >= 3)
  })

  test('teleport times out with the last state when the player never arrives', async () => {
    const fake = new FakeConsole({ world_state: () => world({ arrivals: 1, inWorld: false }), teleport: 'new location: (1, 2)' })
    const explorer = new Explorer(fake)
    await assert.rejects(explorer.teleport(1, 2, { timeoutMs: 300 }), /timed out after 300 ms waiting for the arrival at 1,2; last: .*"inWorld":false/)
  })

  test('a JSON reply that is text is an error naming the command', async () => {
    const fake = new FakeConsole({ world_state: 'oops' })
    await assert.rejects(new Explorer(fake).worldState(), /`\/world_state` did not reply JSON: oops/)
  })

  test('userData and position parse the text replies', async () => {
    const fake = new FakeConsole({
      get_user_data: 'Jane Doe (0x1111111111111111111111111111111111111111): v7, web3=false',
      player_position: '(8, 0.5, -3)'
    })
    const explorer = new Explorer(fake)
    assert.deepEqual(await explorer.userData(), { name: 'Jane Doe', address: '0x1111111111111111111111111111111111111111', version: 7, web3: false })
    assert.equal(await explorer.profileVersion(), 7)
    assert.deepEqual(await explorer.position(), { x: 8, y: 0.5, z: -3 })
  })

  test('connectedPlayers is empty on the no-players text', async () => {
    const fake = new FakeConsole({ connected_players: 'no other players connected' })
    assert.deepEqual(await new Explorer(fake).connectedPlayers(), [])
    const two = new FakeConsole({ connected_players: '0xaaaa, 0xbbbb' })
    assert.deepEqual(await new Explorer(two).connectedPlayers(), ['0xaaaa', '0xbbbb'])
  })

  test('loginIdentity composes the flags and the record redacts the payload', async () => {
    const fake = new FakeConsole({ login_identity: 'logged in with identity' })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    await explorer.loginIdentity('eyJhIjoxfQ==', { name: 'tester', look: 3, guest: true })
    assert.deepEqual(fake.lines, ['/login_identity eyJhIjoxfQ== --guest --name tester --look 3'])
    assert.equal(records[0]!.line, '/login_identity <redacted> --guest --name tester --look 3')
    assert.equal(records[0]!.reply, 'logged in with identity')
  })

  test('redactLine leaves other lines alone', () => {
    assert.equal(redactLine('/hud_click Backpack'), '/hud_click Backpack')
    assert.equal(redactLine('/login_identity abc'), '/login_identity <redacted>')
  })

  test('a failed command is recorded with its error and rethrown', async () => {
    const fake = new FakeConsole({
      teleport: (line) => {
        throw new ConsoleError(line, 'failed to set location')
      }
    })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    await assert.rejects(explorer.run('/teleport 1 1'), ConsoleError)
    assert.match(records[0]!.error ?? '', /failed to set location/)
  })

  test('hud.click refuses a synthetic click unless allowed', async () => {
    const synthetic = new FakeConsole({ hud_click: control({ input: 'synthetic' }) })
    await assert.rejects(new Explorer(synthetic).hud.click('Backpack'), /was synthetic/)
    const allowed = new Explorer(synthetic, { allowSyntheticInput: true })
    assert.equal((await allowed.hud.click('Backpack')).input, 'synthetic')
    const trusted = new FakeConsole({ hud_click: control({ input: 'trusted' }) })
    assert.equal((await new Explorer(trusted).hud.click({ name: 'Backpack', role: 'button' })).input, 'trusted')
    assert.deepEqual(trusted.lines, ['/hud_click {"name":"Backpack","role":"button"}'])
  })

  test('hud.fill sends the selector with the text and returns the field', async () => {
    const fake = new FakeConsole({ hud_fill: control({ role: 'textbox', value: 'Genesis', input: 'trusted' }) })
    const result = await new Explorer(fake).hud.fill('Search', 'Genesis')
    assert.equal(result.value, 'Genesis')
    assert.deepEqual(fake.lines, ['/hud_fill {"name":"Search","text":"Genesis"}'])
  })

  test('hud.find is undefined when nothing matches and an error on ambiguity', async () => {
    const fake = new FakeConsole({
      hud_read: (line) => (line.includes('Missing') ? '{"found":false}' : '{"found":false,"error":"3 controls match; pass an index"}')
    })
    const hud = new Explorer(fake).hud
    assert.equal(await hud.find('Missing'), undefined)
    assert.equal(await hud.isVisible('Missing'), false)
    await assert.rejects(hud.find('Equip'), /3 controls match; pass an index/)
  })

  test('hud.wait passes the state and timeout', async () => {
    const fake = new FakeConsole({ hud_wait: control() })
    await new Explorer(fake).hud.wait('Backpack', { state: 'actionable', timeoutMs: 5000 })
    await new Explorer(fake).hud.waitHidden('Backpack', 2000)
    assert.deepEqual(fake.lines, [
      '/hud_wait {"name":"Backpack","state":"actionable","timeout":5000}',
      '/hud_wait {"name":"Backpack","state":"hidden","timeout":2000}'
    ])
  })

  test('hud.hover clicks the control centre', async () => {
    const fake = new FakeConsole({ hud_read: control(), click_at: 'clicked at (30, 35)' })
    await new Explorer(fake).hud.hover('Backpack')
    assert.deepEqual(fake.sent('click_at'), ['/click_at 30 35'])
  })

  test('key, escape and clickAt compose the lines', async () => {
    const fake = new FakeConsole({ press_key: 'pressed', click_at: 'clicked' })
    const explorer = new Explorer(fake)
    await explorer.key('KeyB')
    await explorer.key('ShiftLeft', 'down')
    await explorer.escape()
    await explorer.clickAt(1, 2)
    await explorer.clickAt(1, 2, 'right')
    assert.deepEqual(fake.lines, ['/press_key KeyB', '/press_key ShiftLeft --state down', '/press_key Escape', '/click_at 1 2', '/click_at 1 2 --button right'])
  })

  test('a long reply is cut in the record, not in the result', async () => {
    const fake = new FakeConsole({ screenshot: Buffer.alloc(9000, 1).toString('base64') })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    const png = await explorer.screenshot()
    assert.equal(png.length, 9000)
    assert.match(records[0]!.reply ?? '', /\.\.\. \(12000 chars\)$/)
  })
})

describe('enterWorld', () => {
  test('presses the lobby Jump In once it is actionable, then waits for the world', async () => {
    let jumped = false
    let reads = 0
    const fake = new FakeConsole({
      world_state: () => world({ inWorld: jumped && ++reads > 1 }),
      hud_read: () => control({ name: 'Jump In', actionable: reads >= 0 }),
      hud_click: () => {
        jumped = true
        return control({ name: 'Jump In', input: 'trusted' })
      }
    })
    const state = await enterWorld(new Explorer(fake), 10_000)
    assert.equal(state.inWorld, true)
    assert.deepEqual(fake.sent('hud_click'), ['/hud_click {"name":"Jump In","role":"button","index":0}'])
  })

  test('needs no Jump In when the world comes on its own', async () => {
    const fake = new FakeConsole({ world_state: () => world(), hud_read: '{"found":false}' })
    await enterWorld(new Explorer(fake), 1000)
    assert.deepEqual(fake.sent('hud_click'), [])
  })
})
