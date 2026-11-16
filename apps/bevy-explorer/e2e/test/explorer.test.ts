import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { ConsoleError, FakeConsole, commandOf, redactLine } from '../driver/console.ts'
import { type CommandRecord, Explorer, parseLiveScenes, parsePortables, parseSmartWearables, type WorldState } from '../driver/explorer.ts'
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
  test('connectivity reads safe status and requests immediate engine recovery', async () => {
    const fake = new FakeConsole({
      connectivity_state: JSON.stringify({ connectionLost: true, probing: false, everConnected: true, suppressed: false, rooms: [{ scope: 'scene', state: 'denied', retryAt: null }] }),
      connectivity_retry: 'Recovery requested'
    })
    const explorer = new Explorer(fake)
    assert.equal((await explorer.connectivity()).rooms[0]?.state, 'denied')
    await explorer.retryConnectivity()
    assert.deepEqual(fake.lines, ['/connectivity_state', '/connectivity_retry'])
  })

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

  test('community chat commands: the message, the channel edits, and the state', async () => {
    const fake = new FakeConsole({
      community_chat: (line) => `[${line.split(' ')[1]}] sent`,
      community_channels: (line) => (line.endsWith('clear') ? 'community channels: []' : 'community channels: ["abc-1", "def-2"]'),
      community_state: JSON.stringify({ routerIdentity: 'message-router-dev-0', routerInRoom: true, channels: ['abc-1'], channelCount: 1, roomSpawned: true, sent: 2 })
    })
    const explorer = new Explorer(fake)
    await explorer.communityChat('abc-1', 'hello there everyone')
    assert.deepEqual(fake.sent('community_chat'), ['/community_chat abc-1 hello there everyone'])
    assert.deepEqual(await explorer.communityChannels({ action: 'add', communityId: 'def-2' }), ['abc-1', 'def-2'])
    assert.deepEqual(await explorer.communityChannels({ action: 'clear' }), [])
    assert.deepEqual(fake.sent('community_channels'), ['/community_channels add def-2', '/community_channels clear'])
    const state = await explorer.communityState()
    assert.equal(state.routerIdentity, 'message-router-dev-0')
    assert.equal(state.sent, 2)
  })

  test('ban and block commands: the state, the lookup, and the block and unblock lines', async () => {
    const other = '0x' + '2'.repeat(40)
    const fake = new FakeConsole({
      ban_block_state: (line) =>
        JSON.stringify({
          address: '0x' + '1'.repeat(40),
          blocks: { state: 'live', reason: null, blocked: [other], blockedBy: [] },
          platformBan: { state: 'checked', isBanned: false, source: null, reason: null, matchedOn: null, customMessage: null, expiresAt: null, checkedAgoSecs: 3, error: null },
          sceneAdmission: { state: 'allowed', scope: null, message: null, sceneId: 'scene-1' },
          sceneRoom: { state: 'connected', sceneId: 'scene-1', bannedAddresses: [], sceneAdmins: [] },
          lookup: line.endsWith(other) ? { address: other, isBlockedEitherWay: true, direction: 'local_user_blocked_them', bannedInScene: false, adminPermissions: 'not_admin' } : null,
          unavailable: line.endsWith(other) ? {} : { lookup: 'no address given' }
        }),
      block_user: 'block requested',
      unblock_user: 'unblock requested'
    })
    const explorer = new Explorer(fake)
    const state = await explorer.banBlockState()
    assert.equal(state.blocks.state, 'live')
    assert.deepEqual(state.blocks.blocked, [other])
    assert.equal(state.platformBan.isBanned, false)
    assert.equal(state.lookup, null)
    assert.equal(state.unavailable.lookup, 'no address given')
    const lookup = await explorer.banBlockState(other)
    assert.equal(lookup.lookup?.direction, 'local_user_blocked_them')
    assert.equal(lookup.sceneRoom.state, 'connected')
    await explorer.blockUser(other)
    await explorer.unblockUser(other)
    assert.deepEqual(fake.sent('ban_block_state'), ['/ban_block_state', `/ban_block_state ${other}`])
    assert.deepEqual(fake.sent('block_user'), [`/block_user ${other}`])
    assert.deepEqual(fake.sent('unblock_user'), [`/unblock_user ${other}`])
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

  test('the voice chat actions compose their lines', async () => {
    const fake = new FakeConsole({ voice: 'queued' })
    const explorer = new Explorer(fake)
    await explorer.voice('call', '0xabc')
    await explorer.voice('accept')
    await explorer.voice('end')
    await explorer.voice('join', 'c1')
    await explorer.voice('close')
    await explorer.voice('hand', 'up')
    await explorer.voice('mic', 'off')
    await explorer.voice('deny', '0xabc')
    assert.deepEqual(fake.lines, ['/voice call 0xabc', '/voice accept', '/voice end', '/voice join c1', '/voice close', '/voice hand up', '/voice mic off', '/voice deny 0xabc'])
  })

  test('voiceState carries the call', async () => {
    const fake = new FakeConsole({
      voice_state: JSON.stringify({
        mic: { available: true, enabled: false, sceneBlocked: false },
        livekit: true,
        connected: true,
        rooms: [],
        speakers: [],
        call: { status: 'receivedCall', kind: 'private', callId: 'c1', communityId: null, peer: '0xabc', connection: 'none', participants: [], micEnabled: false, localSpeaker: false, localRole: 'none', localRequestingToSpeak: false, error: null, errorIsNotice: false }
      })
    })
    const state = await new Explorer(fake).voiceState()
    assert.equal(state.call?.status, 'receivedCall')
    assert.equal(state.call?.peer, '0xabc')
  })

  test('nearbyMute sends the switch then the address, and nearbyMutes reads the list', async () => {
    const fake = new FakeConsole({
      nearby_mute: 'Muted',
      nearby_mutes: JSON.stringify({ account: '0xme', load: 'loaded', count: 1, muted: ['0xabc'], unavailable: { account: 'Not signed in' } })
    })
    const explorer = new Explorer(fake)
    await explorer.nearbyMute('0xabc', true)
    await explorer.nearbyMute('0xabc', false)
    assert.deepEqual(fake.lines, ['/nearby_mute on 0xabc', '/nearby_mute off 0xabc'])
    const mutes = await explorer.nearbyMutes()
    assert.equal(mutes.load, 'loaded')
    assert.deepEqual(mutes.muted, ['0xabc'])
  })

  test('photoMode and takePhoto parse the JSON replies and wait for the mode', async () => {
    let active = false
    const fake = new FakeConsole({
      photo_mode: (line) => {
        if (line.endsWith(' on')) active = true
        if (line.endsWith(' off')) active = false
        return JSON.stringify({ active, requested: null, position: [1, 2, 3], storage: [0, 500], storageLoaded: true, phase: 'idle', captures: 0, uploaded: 0, failed: 0, lastPhotoId: null, lastError: null })
      },
      take_photo: () => JSON.stringify({ active: true, requested: null, position: [1, 2, 3], storage: [0, 500], storageLoaded: true, phase: 'idle', captures: 1, uploaded: 0, failed: 0, lastPhotoId: null, lastError: null })
    })
    const explorer = new Explorer(fake)
    assert.equal((await explorer.photoMode()).active, false)
    await explorer.photoMode('on')
    assert.equal((await explorer.waitPhotoMode(true, 1000)).active, true)
    assert.equal((await explorer.takePhoto()).captures, 1)
    await explorer.photoMode('off')
    assert.equal((await explorer.waitPhotoMode(false, 1000)).active, false)
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
    await explorer.loginIdentity('eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0=', { name: 'tester', look: 3, guest: true })
    assert.deepEqual(fake.lines, ['/login_identity eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0= --guest --name tester --look 3'])
    assert.equal(records[0]!.line, '/login_identity <redacted> --guest --name tester --look 3')
    assert.equal(records[0]!.reply, 'logged in with identity')
  })

  test('redactLine leaves other lines alone, and words that are no identity', () => {
    assert.equal(redactLine('/hud_click Backpack'), '/hud_click Backpack')
    assert.equal(redactLine('/login_identity abc'), '/login_identity abc')
    assert.equal(redactLine('/login_identity eyJwcml2YXRlS2V5IjoiMHhzZWNyZXQifQ=='), '/login_identity <redacted>')
  })

  test('a failed login keeps its payload out of the error, the record and the stack', async () => {
    const payload = 'eyJlcGhlbWVyYWwiOiJzZWNyZXQifQ=='
    const fake = new FakeConsole({
      login_identity: (line) => {
        throw new ConsoleError(line, `could not decode the identity ${payload}: bad padding`)
      }
    })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    await assert.rejects(explorer.loginIdentity(payload, { guest: true }), (error: unknown) => {
      assert.ok(error instanceof ConsoleError)
      assert.equal(error.message, '`/login_identity <redacted> --guest` failed: could not decode the identity <redacted>: bad padding')
      assert.equal(error.line, '/login_identity <redacted> --guest')
      assert.doesNotMatch(error.stack ?? '', /eyJlcGhl/)
      return true
    })
    assert.equal(records[0]!.line, '/login_identity <redacted> --guest')
    assert.equal(records[0]!.error, '`/login_identity <redacted> --guest` failed: could not decode the identity <redacted>: bad padding')
    assert.deepEqual(fake.lines, [`/login_identity ${payload} --guest`])
  })

  test("a transport's own error that quotes the line is scrubbed before it is rethrown", async () => {
    const payload = 'eyJwcml2YXRlS2V5IjoiMHhzZWNyZXQifQ=='
    const fake = new FakeConsole({
      login_identity: (line) => {
        throw new Error(`${line} did not finish within 5 ms`)
      }
    })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    await assert.rejects(explorer.run(`/login_identity ${payload}`), (error: unknown) => {
      assert.ok(error instanceof Error)
      assert.equal(error.message, '/login_identity <redacted> did not finish within 5 ms')
      assert.doesNotMatch(error.stack ?? '', /eyJwcml2/)
      return true
    })
    assert.equal(records[0]!.error, '/login_identity <redacted> did not finish within 5 ms')
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
    const synthetic = new FakeConsole({ hud_wait: control(), hud_read: control(), hud_click: control({ input: 'synthetic' }) })
    await assert.rejects(new Explorer(synthetic).hud.click('Backpack'), /was synthetic/)
    const allowed = new Explorer(synthetic, { allowSyntheticInput: true })
    assert.equal((await allowed.hud.click('Backpack')).input, 'synthetic')
    const trusted = new FakeConsole({ hud_wait: control(), hud_click: control({ input: 'trusted' }) })
    assert.equal((await new Explorer(trusted).hud.click({ name: 'Backpack', role: 'button' })).input, 'trusted')
    assert.deepEqual(trusted.lines, [
      '/hud_wait {"name":"Backpack","role":"button","state":"actionable","timeout":10000}',
      '/hud_click {"name":"Backpack","role":"button"}'
    ])
  })

  test('hud.click waits for the control to take a click, and fails naming what it saw', async () => {
    const fake = new FakeConsole({
      hud_wait: (line) => {
        throw new ConsoleError(line, 'timed out after 10000 ms waiting for {"name":"Emotes","role":"tab"} to be actionable: the control can\'t be clicked: covered by <header>')
      },
      hud_click: control({ input: 'trusted' })
    })
    await assert.rejects(new Explorer(fake).hud.click({ name: 'Emotes', role: 'tab' }), /covered by <header>/)
    assert.deepEqual(fake.sent('hud_click'), [])
  })

  test('hud.click leaves a moving control to the HUD, which does not call it actionable until it holds still', async () => {
    const fake = new FakeConsole({
      hud_wait: (line) => {
        throw new ConsoleError(line, 'timed out after 10000 ms waiting for {"name":"Emotes","role":"tab"} to be actionable: the control can\'t be clicked: still moving')
      },
      hud_click: control({ input: 'trusted' })
    })
    await assert.rejects(new Explorer(fake).hud.click({ name: 'Emotes', role: 'tab' }), /still moving/)
    assert.deepEqual(fake.lines.map(commandOf), ['hud_wait'])
  })

  test('hud.fill waits for the field, sends the selector with the text and returns the field', async () => {
    const fake = new FakeConsole({
      hud_wait: control({ role: 'textbox' }),
      hud_fill: control({ role: 'textbox', value: 'Genesis', input: 'trusted' }),
      world_state: world({ keyboardClaimed: true })
    })
    const result = await new Explorer(fake).hud.fill('Search', 'Genesis')
    assert.equal(result.value, 'Genesis')
    assert.deepEqual(fake.lines, ['/hud_wait {"name":"Search","state":"actionable","timeout":10000}', '/hud_fill {"name":"Search","text":"Genesis"}', '/world_state'])
  })

  test('hud.fill returns once the engine hands the field the keyboard, so the next key is typed', async () => {
    let reads = 0
    const fake = new FakeConsole({
      hud_wait: control({ role: 'textbox' }),
      hud_fill: control({ role: 'textbox', value: 'hi', input: 'trusted' }),
      // the HUD relays the field's focus through the bridge scene: two frames pass before it lands
      world_state: () => world({ keyboardClaimed: ++reads > 2 })
    })
    await new Explorer(fake).hud.fill('Write a message', 'hi')
    assert.equal(fake.sent('world_state').length, 3)
  })

  test('hud.fill does not wait for the keyboard after a synthetic fill, or from a client that takes no input', async () => {
    const synthetic = new FakeConsole({ hud_wait: control({ role: 'textbox' }), hud_fill: control({ role: 'textbox', value: 'hi', input: 'synthetic' }) })
    await new Explorer(synthetic, { allowSyntheticInput: true }).hud.fill('Search', 'hi')
    assert.deepEqual(synthetic.sent('world_state'), [])
    const inputless = new FakeConsole({ hud_wait: control({ role: 'textbox' }), hud_fill: control({ role: 'textbox', value: 'hi', input: 'trusted' }), world_state: world({ keyboardClaimed: null }) })
    await new Explorer(inputless).hud.fill('Search', 'hi')
    assert.equal(inputless.sent('world_state').length, 1)
  })

  test('the keyboard waits refuse a client older than the suite, whose /world_state does not say', async () => {
    const older = new FakeConsole({ hud_wait: control({ role: 'textbox' }), hud_fill: control({ role: 'textbox', value: 'hi', input: 'trusted' }), world_state: world() })
    await assert.rejects(new Explorer(older).hud.fill('Search', 'hi'), /older than this suite/)
    await assert.rejects(new Explorer(older).keyboardReleased(), /older than this suite/)
    assert.equal(older.sent('world_state').length, 2)
  })

  test('keyboardReleased waits for the engine to let the keyboard go', async () => {
    let reads = 0
    const fake = new FakeConsole({ world_state: () => world({ keyboardClaimed: ++reads < 3 }) })
    await new Explorer(fake).keyboardReleased()
    assert.equal(fake.sent('world_state').length, 3)
    const silent = new FakeConsole({ world_state: world({ keyboardClaimed: null }) })
    await new Explorer(silent).keyboardReleased()
    assert.equal(silent.sent('world_state').length, 1)
  })

  test('hud.waitReady waits for the control out of any region still loading', async () => {
    const fake = new FakeConsole({ hud_wait: control({ role: 'region', name: 'Catalog', busy: false }) })
    const read = await new Explorer(fake).hud.waitReady({ name: 'Catalog', role: 'region' }, 5000)
    assert.equal(read.busy, false)
    assert.deepEqual(fake.lines, ['/hud_wait {"name":"Catalog","role":"region","state":"ready","timeout":5000}'])
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

  test('hud.hover clicks the control centre once it takes a click', async () => {
    const fake = new FakeConsole({ hud_wait: control(), hud_read: control(), click_at: 'clicked at (30, 35)' })
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

  test('hoverEntity, clickEntity, raycast and scene log commands compose the lines', async () => {
    const report = JSON.stringify({ hit: null, entries: [], events: [], diagnosis: 'blocked by entity 9' })
    const fake = new FakeConsole({ hover_entity: report, click_entity: report, raycast_stats: JSON.stringify({ budgetMs: 1.67, ran: 1, deferred: 0, spentMs: 0.1, uncappedRan: 2, uncappedSpentMs: 0.2 }), raycast_budget: 'raycast budget: 0.00 ms (override)', scene_logs: '[1.00] Info: a\n[2.00] Info: b' })
    const explorer = new Explorer(fake)
    assert.equal((await explorer.hoverEntity(512, { bounds: true, frames: 10 })).diagnosis, 'blocked by entity 9')
    await explorer.clickEntity('512', { button: 'primary', event: 'down', timeoutFrames: 60 })
    await explorer.clearPointer()
    assert.equal((await explorer.raycastStats()).budgetMs, 1.67)
    assert.equal(await explorer.raycastBudget(0), 0)
    assert.deepEqual(await explorer.sceneLogs(50), ['[1.00] Info: a', '[2.00] Info: b'])
    assert.deepEqual(fake.lines, [
      '/hover_entity 512 --bounds --frames 10',
      '/click_entity 512 --button primary --event down --timeout-frames 60',
      '/hover_entity --clear',
      '/raycast_stats',
      '/raycast_budget 0',
      '/scene_logs 50'
    ])
  })

  test("profileDeployWindow waits out the catalyst's window after the own profile changed, and only then", async () => {
    let version = 3
    const fake = new FakeConsole({ get_user_data: () => `tester (0x${'a'.repeat(40)}): v${version}, web3=true` })
    const explorer = new Explorer(fake, { profileDeployWindowMs: 300 })
    let started = Date.now()
    await explorer.profileDeployWindow()
    assert.ok(Date.now() - started < 200, 'a profile seen unchanged needs no wait')
    version = 4
    started = Date.now()
    await explorer.profileDeployWindow()
    assert.ok(Date.now() - started >= 290, 'a change seen now waits the whole window')
    started = Date.now()
    await explorer.profileDeployWindow()
    assert.ok(Date.now() - started < 200, 'the window is spent once')
  })

  test('profileDeployWindow counts a deployment it was told of, such as a first login', async () => {
    const fake = new FakeConsole({ get_user_data: () => `tester (0x${'a'.repeat(40)}): v1, web3=true` })
    const explorer = new Explorer(fake, { profileDeployWindowMs: 300 })
    explorer.profileDeployed()
    const started = Date.now()
    await explorer.profileDeployWindow()
    assert.ok(Date.now() - started >= 250, 'the window starts at the deployment it was told of')
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

describe('smart wearables and live scenes', () => {
  const urn = 'urn:decentraland:off-chain:e2e:smart-wearable'

  test('parseLiveScenes reads the title, the hash and the flags', () => {
    assert.deepEqual(parseLiveScenes('no scenes loaded'), [])
    assert.deepEqual(parseLiveScenes('Genesis Plaza [bafyabc]\nSmart Wearable Fixture [e2e123, portable, smart wearable]\nOld [b1, portable, broken]'), [
      { title: 'Genesis Plaza', hash: 'bafyabc', portable: false, smartWearable: false, broken: false },
      { title: 'Smart Wearable Fixture', hash: 'e2e123', portable: true, smartWearable: true, broken: false },
      { title: 'Old', hash: 'b1', portable: true, smartWearable: false, broken: true }
    ])
    assert.throws(() => parseLiveScenes('no brackets here'), /unparseable live_scenes line/)
  })

  test('parseSmartWearables reads the header and each entry, with or without a reason', () => {
    assert.deepEqual(parseSmartWearables('no smart wearables'), { currentSceneAllows: null, entries: [] })
    assert.deepEqual(parseSmartWearables(`current scene allows: false\n${urn} running\nurn:a blocked [scene disables portable experiences]\nurn:b failed no scene.json`), {
      currentSceneAllows: false,
      entries: [
        { urn, state: 'running' },
        { urn: 'urn:a', state: 'blocked', reason: 'scene disables portable experiences' },
        { urn: 'urn:b', state: 'failed', reason: 'no scene.json' }
      ]
    })
    assert.deepEqual(parseSmartWearables('current scene allows: true\nurn:c stopped\nurn:d notSmart'), {
      currentSceneAllows: true,
      entries: [
        { urn: 'urn:c', state: 'stopped' },
        { urn: 'urn:d', state: 'notSmart' }
      ]
    })
    assert.throws(() => parseSmartWearables('urn:a running'), /unparseable smart_wearables header/)
    // the states of the up-front authorization are gone
    assert.throws(() => parseSmartWearables('current scene allows: true\nurn:a awaitingAuthorization'), /unparseable smart_wearables line/)
    assert.throws(() => parseSmartWearables('current scene allows: true\nurn:a killed'), /unparseable smart_wearables line/)
  })

  test('parsePortables reads the name, the hash or none, the kind and the state', () => {
    assert.deepEqual(parsePortables('no portables'), [])
    assert.deepEqual(parsePortables('Smart Wearable Fixture [e2e123-sw-male, smartWearable, running]\nRadio [bafyabc, scene, blocked]\nJetpack [-, smart wearable, stopped]'), [
      { name: 'Smart Wearable Fixture', hash: 'e2e123-sw-male', kind: 'smartWearable', state: 'running' },
      { name: 'Radio', hash: 'bafyabc', kind: 'scene', state: 'blocked' },
      { name: 'Jetpack', hash: null, kind: 'smartWearable', state: 'stopped' }
    ])
    assert.throws(() => parsePortables('Radio [bafyabc, scene]'), /unparseable portables line/)
    assert.throws(() => parsePortables('Radio [bafyabc, startup, running]'), /unparseable portables line/)
    assert.throws(() => parsePortables('Radio [bafyabc, scene, killed]'), /unparseable portables line/)
  })

  test('stopPortable checks the reply', async () => {
    const fake = new FakeConsole({ stop_portable: (line) => (line.endsWith(' gone') ? 'not a listed portable: gone' : `stopped ${line.split(' ')[1]}`) })
    const explorer = new Explorer(fake)
    await explorer.stopPortable('e2e123-sw-male')
    assert.deepEqual(fake.sent('stop_portable'), ['/stop_portable e2e123-sw-male'])
    await assert.rejects(explorer.stopPortable('gone'), /replied: not a listed portable: gone/)
  })

  test('smartWearableAction checks the reply and smartWearable finds the entry', async () => {
    const fake = new FakeConsole({
      smart_wearable: (line) => (line.startsWith('/smart_wearable kill') ? `not running: ${urn}` : `running ${urn}`),
      smart_wearables: `current scene allows: true\n${urn} running`
    })
    const explorer = new Explorer(fake)
    await explorer.smartWearableAction('run', urn)
    assert.deepEqual(fake.sent('smart_wearable'), [`/smart_wearable run ${urn}`])
    await assert.rejects(explorer.smartWearableAction('kill', urn), /replied: not running/)
    assert.deepEqual(await explorer.smartWearable(urn), { urn, state: 'running' })
    assert.equal(await explorer.smartWearable('urn:other'), undefined)
  })

  test('setScene, sceneLogs and uiTexts read the targeted scene', async () => {
    const fake = new FakeConsole({
      set_scene: "active scene set to 'Smart Wearable Fixture'",
      scene_logs: '(+3 older entries not retained)\n[1.25] Log: smart wearable fixture started\n[1.50] Warn: careful',
      scene_entities: '512\n513',
      inspect_component: (line) => (line.includes(' 512 ') ? JSON.stringify({ value: 'Smart wearable running', fontSize: 20 }) : JSON.stringify({ value: 'other' }))
    })
    const explorer = new Explorer(fake)
    await explorer.setScene('e2e123')
    await explorer.setScene()
    assert.deepEqual(fake.sent('set_scene'), ['/set_scene e2e123', '/set_scene'])
    assert.deepEqual(await explorer.sceneLogs(50), ['(+3 older entries not retained)', '[1.25] Log: smart wearable fixture started', '[1.50] Warn: careful'])
    assert.deepEqual(fake.sent('scene_logs'), ['/scene_logs 50'])
    assert.deepEqual(await explorer.uiTexts(), ['Smart wearable running', 'other'])
    assert.deepEqual(fake.sent('scene_entities'), ['/scene_entities UiText'])
    assert.deepEqual(fake.sent('inspect_component'), ['/inspect_component 512 UiText', '/inspect_component 513 UiText'])
  })

  test('sceneLogs is empty on the no-logs text', async () => {
    assert.deepEqual(await new Explorer(new FakeConsole({ scene_logs: '(no logs)' })).sceneLogs(), [])
  })
})

describe('hudLogin', () => {
  test('composes the flags, parses the reply and redacts the payload in the record', async () => {
    const fake = new FakeConsole({ hud_login: '{"address":"0xabc","guest":true,"phase":"entering"}' })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    const reply = await explorer.hudLogin('eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0=', { name: 'tester', look: 3, guest: true, realm: 'https://realm.example', position: '10,-20' })
    assert.deepEqual(reply, { address: '0xabc', guest: true, phase: 'entering' })
    assert.deepEqual(fake.lines, ['/hud_login eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0= --name tester --look 3 --guest --realm https://realm.example --position 10,-20'])
    assert.equal(records[0]!.line, '/hud_login <redacted> --name tester --look 3 --guest --realm https://realm.example --position 10,-20')
  })

  test("the credentials of the lines run are replaced in text that quotes them, for the page's console", async () => {
    const fake = new FakeConsole({ hud_login: '{"address":"0xabc","guest":false,"phase":"lobby"}', login_identity: 'logged in with identity' })
    const explorer = new Explorer(fake)
    assert.equal(explorer.redactSecrets('nothing yet eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0='), 'nothing yet eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0=')
    await explorer.hudLogin('eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0=')
    await explorer.loginIdentity('eyJlcGhlbWVyYWxJZGVudGl0eSI6Mn0=')
    assert.equal(explorer.redactSecrets('[login] failed: eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0= and eyJlcGhlbWVyYWxJZGVudGl0eSI6Mn0= again eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0='), '[login] failed: <redacted> and <redacted> again <redacted>')
  })

  test('a flag value with whitespace is refused before the line is sent', async () => {
    const fake = new FakeConsole({ hud_login: '{}' })
    await assert.rejects(new Explorer(fake).hudLogin('eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0=', { name: 'two words', look: 1 }), /a \/hud_login --name takes no whitespace: "two words"/)
    assert.deepEqual(fake.lines, [])
  })

  test('a reply that is not JSON fails the command, the payload redacted before the reply is cut', async () => {
    // longer than the cut, so a cut before the redaction would leave a part of it
    const payload = Buffer.from(JSON.stringify({ privateKey: '0x' + '56'.repeat(150) })).toString('base64')
    const fake = new FakeConsole({ hud_login: `signed in with ${payload}` })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    await assert.rejects(explorer.hudLogin(payload), (error: unknown) => {
      assert.ok(error instanceof Error)
      assert.equal(error.message, '`/hud_login <redacted>` did not reply JSON: signed in with <redacted>')
      return true
    })
    assert.equal(records[0]!.reply, 'signed in with <redacted>')
  })

  test('fails with the sign-in error the HUD reports, the payload redacted', async () => {
    const payload = 'eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0='
    const fake = new FakeConsole({
      hud_login: (line) => {
        throw new ConsoleError(line, `profile fetch failed for ${payload}: 503`)
      }
    })
    const explorer = new Explorer(fake)
    const records: CommandRecord[] = []
    explorer.onCommand((record) => records.push(record))
    await assert.rejects(explorer.hudLogin(payload, { defaultOnError: true }), (error: unknown) => {
      assert.ok(error instanceof ConsoleError)
      assert.equal(error.message, '`/hud_login <redacted> --default-on-error` failed: profile fetch failed for <redacted>: 503')
      return true
    })
    assert.equal(records[0]!.error, '`/hud_login <redacted> --default-on-error` failed: profile fetch failed for <redacted>: 503')
  })
})

describe('enterWorld', () => {
  test('presses the lobby Jump In once it is actionable, then waits for the world', async () => {
    let jumped = false
    let reads = 0
    const fake = new FakeConsole({
      world_state: () => world({ inWorld: jumped && ++reads > 1 }),
      hud_read: () => control({ name: 'Jump In', actionable: reads >= 0 }),
      hud_wait: () => control({ name: 'Jump In' }),
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
