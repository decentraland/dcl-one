import { describe, expect, it } from 'vitest'
import { formatConsoleReply, HUD_HIDDEN_COMMANDS, parseChatCommand, resolveGoto, HELP_TEXT, type GotoHere } from './chatCommands'
import { parseGotoTarget } from '../lobby/goto'

describe('parseChatCommand', () => {
  it('passes a normal message through as send (trimmed)', () => {
    expect(parseChatCommand('hello world')).toEqual({ kind: 'send', text: 'hello world' })
    expect(parseChatCommand('  hi  ')).toEqual({ kind: 'send', text: 'hi' })
    // A slash mid-word is not a command.
    expect(parseChatCommand('and/or')).toEqual({ kind: 'send', text: 'and/or' })
  })

  it('/help echoes the help text as a system message', () => {
    const r = parseChatCommand('/help')
    expect(r.kind).toBe('system')
    expect(r).toEqual({ kind: 'system', message: HELP_TEXT })
    // The help lists /commands and the goto forms.
    expect(HELP_TEXT).toContain('/commands')
    expect(HELP_TEXT).toContain('/goto ')
    expect(HELP_TEXT).toContain('/goto-local ')
  })

  it('/commands and /reload map to their actions', () => {
    expect(parseChatCommand('/commands')).toEqual({ kind: 'commands' })
    expect(parseChatCommand('/reload')).toEqual({ kind: 'reload' })
  })

  const target = (t: object) => ({ world: null, parcel: null, spawnPoint: null, random: false, crowd: false, ...t })
  const goto = (t: object, local = false) => ({ kind: 'goto', target: target(t), local })

  it('/goto reads every form of destination', () => {
    expect(parseChatCommand('/goto 10,20')).toEqual(goto({ parcel: { x: 10, y: 20 } }))
    expect(parseChatCommand('/goto -5, 3')).toEqual(goto({ parcel: { x: -5, y: 3 } }))
    expect(parseChatCommand('/goto 10,20/lobby')).toEqual(goto({ parcel: { x: 10, y: 20 }, spawnPoint: 'lobby' }))
    expect(parseChatCommand('/goto random')).toEqual(goto({ random: true }))
    expect(parseChatCommand('/goto crowd')).toEqual(goto({ crowd: true }))
    expect(parseChatCommand('/goto boedo')).toEqual(goto({ world: 'boedo' }))
    expect(parseChatCommand('/goto boedo.dcl.eth/-51,1')).toEqual(goto({ world: 'boedo.dcl.eth', parcel: { x: -51, y: 1 } }))
    expect(parseChatCommand('/goto boedo/lobby')).toEqual(goto({ world: 'boedo', spawnPoint: 'lobby' }))
    expect(parseChatCommand('/goto boedo/0,0/Lobby')).toEqual(goto({ world: 'boedo', parcel: { x: 0, y: 0 }, spawnPoint: 'Lobby' }))
    // names of a realm, told apart when the destination is resolved
    expect(parseChatCommand('/goto genesis')).toEqual(goto({ world: 'genesis' }))
    expect(parseChatCommand('/goto main')).toEqual(goto({ world: 'main' }))
    // the command's name in any case, and spaces around its destination
    expect(parseChatCommand('/GOTO   10,20  ')).toEqual(goto({ parcel: { x: 10, y: 20 } }))
    // only the two words themselves are special
    expect(parseChatCommand('/goto Random')).toEqual(goto({ world: 'Random' }))
  })

  it('/goto-local takes a parcel, a parcel and a spawn point, or a spawn point', () => {
    expect(parseChatCommand('/goto-local 1,2')).toEqual(goto({ parcel: { x: 1, y: 2 } }, true))
    expect(parseChatCommand('/goto-local 1,2/roof')).toEqual(goto({ parcel: { x: 1, y: 2 }, spawnPoint: 'roof' }, true))
    expect(parseChatCommand('/goto-local roof')).toEqual(goto({ spawnPoint: 'roof' }, true))
    for (const refused of ['', 'random', 'crowd', 'boedo/1,2', 'boedo/roof', 'boedo/1,2/roof', '12,34,56', '1,2 roof']) {
      const r = parseChatCommand(`/goto-local ${refused}`)
      expect(r.kind).toBe('system')
      expect((r as { message: string }).message).toContain('🔴 Invalid parameters, usage:\n/goto-local ')
    }
  })

  it('/world takes a world name alone', () => {
    expect(parseChatCommand('/world boedo')).toEqual(goto({ world: 'boedo' }))
    expect(parseChatCommand('/world random')).toEqual(goto({ world: 'random' }))
    expect(parseChatCommand('/world boedo/1,2').kind).toBe('system')
    expect(parseChatCommand('/world 1,2').kind).toBe('system')
  })

  it('missing/invalid args → the usage as a system message, never a broadcast', () => {
    expect(parseChatCommand('/goto')).toEqual({
      kind: 'system',
      message:
        '🔴 Invalid parameters, usage:\n/goto <x,y | x,y/spawn | random | crowd | world | world/x,y | world/spawn | world/x,y/spawn>\n  Teleport inside of Genesis or World\n  x,y is in the realm you are in; random and crowd are in Genesis City; genesis or main is the city, any other name is a world'
    })
    expect(parseChatCommand('/world').kind).toBe('system')
    expect(parseChatCommand('/goto 1 2 3').kind).toBe('system')
    expect(parseChatCommand('/goto 10 ,20').kind).toBe('system')
    expect(parseChatCommand('/goto boedo/main lobby').kind).toBe('system')
  })

  it('any other /command passes to the engine console as the line typed, for the engine to split', () => {
    expect(parseChatCommand('/dance')).toEqual({ kind: 'console', command: 'dance', hasArgs: false, line: '/dance' })
    expect(parseChatCommand('/Emote 3')).toEqual({ kind: 'console', command: 'emote', hasArgs: true, line: '/emote 3' })
    expect(parseChatCommand('/changerealm boedo.dcl.eth "https://x.y/content"')).toEqual({
      kind: 'console',
      command: 'changerealm',
      hasArgs: true,
      line: '/changerealm boedo.dcl.eth "https://x.y/content"'
    })
    // quotes, escapes and JSON reach the engine untouched
    expect(parseChatCommand(`/emote_state {"a": "b  c"} it's "x\\"y"`)).toMatchObject({
      line: `/emote_state {"a": "b  c"} it's "x\\"y"`
    })
    // A bare slash is not a command.
    expect(parseChatCommand('/').kind).toBe('system')
  })

  it('HUD-hidden engine commands → the unknown-command hint, never the console', () => {
    for (const name of ['crdt_snapshot', 'screenshot', 'set_component', 'lock_preview', 'logout', 'exit']) {
      expect(HUD_HIDDEN_COMMANDS.has(name)).toBe(true)
      const r = parseChatCommand(`/${name} 1 2`)
      expect(r.kind).toBe('system')
      expect((r as { message: string }).message).toContain('/help')
    }
  })

  it('/reload <hash> and /help <command> go to the engine; the bare forms stay local', () => {
    expect(parseChatCommand('/reload bafyabc')).toEqual({ kind: 'console', command: 'reload', hasArgs: true, line: '/reload bafyabc' })
    expect(parseChatCommand('/help teleport')).toEqual({ kind: 'console', command: 'help', hasArgs: true, line: '/help /teleport' })
    expect(parseChatCommand('/help /teleport')).toEqual({ kind: 'console', command: 'help', hasArgs: true, line: '/help /teleport' })
    expect(parseChatCommand('/help').kind).toBe('system')
  })
})

describe('resolveGoto', () => {
  const GENESIS = 'https://realm.example/main'
  const inGenesis: GotoHere = { kind: 'genesis', realm: null, parcel: { x: 5, y: 6 }, heading: null }
  const inWorld: GotoHere = { kind: 'world', realm: 'boedo.dcl.eth', parcel: { x: 1, y: 2 }, heading: null }
  // a scene preview, a realm of its own, or one the engine has not reported yet
  const elsewhere: GotoHere = { kind: 'other', realm: null, parcel: { x: 0, y: 0 }, heading: null }
  const resolve = (text: string, here: GotoHere, local = false) => resolveGoto(parseGotoTarget(text), local, here, GENESIS)
  const usage = { kind: 'reply', message: expect.stringContaining('🔴 Invalid parameters, usage:\n/goto ') }

  it('a parcel is one of the realm the player is in: a teleport in Genesis City and in a world', () => {
    expect(resolve('10,20', inGenesis)).toEqual({ kind: 'teleport', x: 10, y: 20, spawnPoint: undefined, reply: '🟢 You teleported to 10,20.' })
    expect(resolve('10,20/lobby', inGenesis)).toMatchObject({ kind: 'teleport', x: 10, y: 20, spawnPoint: 'lobby' })
    // a player in a world stays in it, as with /goto-local
    expect(resolve('10,20', inWorld)).toEqual({ kind: 'teleport', x: 10, y: 20, spawnPoint: undefined, reply: '🟢 You teleported to 10,20.' })
    expect(resolve('10,20/lobby', inWorld)).toEqual(resolve('10,20/lobby', inWorld, true))
    // Genesis City from a world is asked for by its name
    expect(resolve('genesis/10,20', inWorld)).toMatchObject({ kind: 'travel', realm: GENESIS, parcel: { x: 10, y: 20 } })
    expect(resolve('152,0', inGenesis)).toEqual({ kind: 'reply', message: '🔴 Error: Parcel (152, 0) is outside of the bounds.' })
    // held to the bounds of the realm it is in: Genesis City's extension is outside a world
    expect(resolve('160,155', inGenesis).kind).toBe('teleport')
    expect(resolve('160,155', inWorld)).toEqual({ kind: 'reply', message: '🔴 Error: Parcel (160, 155) is outside of the bounds.' })
    expect(resolve('-150,150', inWorld).kind).toBe('teleport')
  })

  it('in any other realm a parcel is one of that realm, whatever its bounds', () => {
    // a scene preview refuses realm changes: /goto moves inside it
    expect(resolve('2,3', elsewhere)).toEqual({ kind: 'teleport', x: 2, y: 3, spawnPoint: undefined, reply: '🟢 You teleported to 2,3.' })
    expect(resolve('2,3', elsewhere, true)).toMatchObject({ kind: 'teleport', x: 2, y: 3 })
    expect(resolve('500,500', elsewhere)).toMatchObject({ kind: 'teleport', x: 500, y: 500 })
    // a world or Genesis City is still a realm change away
    expect(resolve('boedo', elsewhere)).toMatchObject({ kind: 'travel', realm: 'boedo.dcl.eth' })
    expect(resolve('genesis', elsewhere)).toMatchObject({ kind: 'travel', realm: GENESIS })
  })

  it('random and crowd are Genesis City places from any realm', () => {
    // as the session hands them over: with the parcel it picked
    const random = { ...parseGotoTarget('random'), parcel: { x: 7, y: 8 } }
    const crowd = { ...parseGotoTarget('crowd'), parcel: { x: 9, y: 10 } }
    expect(resolveGoto(random, false, inGenesis, GENESIS)).toEqual({ kind: 'teleport', x: 7, y: 8, spawnPoint: undefined, reply: '🟢 You teleported to 7,8.' })
    for (const here of [inWorld, elsewhere]) {
      expect(resolveGoto(random, false, here, GENESIS)).toEqual({ kind: 'travel', realm: GENESIS, parcel: { x: 7, y: 8 }, spawnPoint: undefined, reply: '🟢 You teleported to 7,8.' })
      expect(resolveGoto(crowd, false, here, GENESIS)).toMatchObject({ kind: 'travel', realm: GENESIS, parcel: { x: 9, y: 10 } })
    }
  })

  it('a local parcel stays in the realm, within its own bounds', () => {
    expect(resolve('10,20', inWorld, true)).toMatchObject({ kind: 'teleport', x: 10, y: 20 })
    expect(resolve('160,155', inWorld, true)).toEqual({ kind: 'reply', message: '🔴 Error: Parcel (160, 155) is outside of the bounds.' })
    expect(resolve('160,155', inGenesis, true)).toMatchObject({ kind: 'teleport', x: 160, y: 155 })
  })

  it('a spawn point alone is one of the scene the player stands in', () => {
    const roof = { ...parseGotoTarget('1,1'), parcel: null, spawnPoint: 'roof' }
    expect(resolveGoto(roof, true, inWorld, GENESIS)).toEqual({ kind: 'teleport', x: 1, y: 2, spawnPoint: 'roof', reply: '🟢 You teleported to 1,2.' })
  })

  it('a world is a realm change, named as it was typed', () => {
    expect(resolve('Mana', inGenesis)).toEqual({ kind: 'travel', realm: 'mana.dcl.eth', parcel: undefined, spawnPoint: undefined, reply: '🟢 Welcome to the Mana world!', world: 'Mana' })
    expect(resolve('mana/3,4/door', inGenesis)).toMatchObject({ kind: 'travel', realm: 'mana.dcl.eth', parcel: { x: 3, y: 4 }, spawnPoint: 'door' })
    expect(resolve('name.eth', inWorld)).toMatchObject({ kind: 'travel', realm: 'name.eth' })
    // the world called main, by its full name
    expect(resolve('main.dcl.eth', inGenesis)).toMatchObject({ kind: 'travel', realm: 'main.dcl.eth', world: 'main.dcl.eth' })
  })

  it('genesis and main, in any case, are Genesis City', () => {
    // no world's name to blame for a failure; with no parcel named, Genesis Plaza
    expect(resolve('genesis', inWorld)).toEqual({ kind: 'travel', realm: GENESIS, parcel: { x: 0, y: 0 }, spawnPoint: undefined, reply: '🟢 Welcome to Genesis City!', world: undefined })
    expect(resolve('Genesis', inWorld)).toMatchObject({ kind: 'travel', realm: GENESIS, reply: '🟢 Welcome to Genesis City!' })
    expect(resolve('GENESIS', inGenesis)).toEqual({ kind: 'reply', message: '🟡 You are already in GENESIS!' })
    expect(resolve('main', inWorld)).toEqual({ kind: 'travel', realm: GENESIS, parcel: { x: 0, y: 0 }, spawnPoint: undefined, reply: '🟢 Welcome to Genesis City!', world: undefined })
    expect(resolve('Main/100,100', inWorld)).toMatchObject({ kind: 'travel', realm: GENESIS, parcel: { x: 100, y: 100 }, world: undefined })
    // a parcel of the city, held to its bounds, and a teleport from inside it
    expect(resolve('main/200,200', inWorld)).toEqual({ kind: 'reply', message: '🔴 Error: Parcel (200, 200) is outside of the bounds.' })
    expect(resolve('main/7,8/door', inGenesis)).toMatchObject({ kind: 'teleport', x: 7, y: 8, spawnPoint: 'door' })
    expect(resolve('main', inGenesis)).toEqual({ kind: 'reply', message: '🟡 You are already in main!' })
  })

  it("a realm's address goes as it is; any other name has to be a world's", () => {
    expect(resolve('https://realm.example/other', inWorld)).toEqual({
      kind: 'travel',
      realm: 'https://realm.example/other',
      parcel: undefined,
      spawnPoint: undefined,
      reply: '🟢 Welcome to https://realm.example/other!',
      world: undefined
    })
    expect(resolve('http://localhost:8000', inGenesis)).toMatchObject({ kind: 'travel', realm: 'http://localhost:8000' })
    // read whole: what follows an address is part of it, no parcel or spawn point
    expect(resolve('https://realm.example/other/999,999', inGenesis)).toMatchObject({ kind: 'travel', realm: 'https://realm.example/other/999,999', parcel: undefined })
    expect(resolve('https://realm.example/other/door', inGenesis)).toMatchObject({ kind: 'travel', realm: 'https://realm.example/other/door', spawnPoint: undefined })
    // a world whose name only starts like an address
    expect(resolve('httpsdog', inGenesis)).toMatchObject({ kind: 'travel', realm: 'httpsdog.dcl.eth' })
    for (const bad of ['../x', 'a?b', 'my-world', 'your.world.eth', 'localhost:8000', 'boedo/1,2,3']) {
      expect(resolve(bad, inGenesis)).toEqual(usage)
    }
  })

  it('a parcel after a world or genesis is held to its bounds, wherever the player is', () => {
    const out = (x: number, y: number) => ({ kind: 'reply', message: `🔴 Error: Parcel (${x}, ${y}) is outside of the bounds.` })
    for (const here of [inGenesis, inWorld, elsewhere]) {
      expect(resolve('genesis/152,0', here)).toEqual(out(152, 0))
      expect(resolve('mana/999,999', here)).toEqual(out(999, 999))
      expect(resolve('mana/0,-151/door', here)).toEqual(out(0, -151))
    }
    // the world the player is in, and Genesis City from inside it
    expect(resolve('boedo/151,0', inWorld)).toEqual(out(151, 0))
    expect(resolve('genesis/0,160', inGenesis)).toEqual(out(0, 160))
    // inside: Genesis City's extension, a world's edge
    expect(resolve('genesis/160,155', inWorld)).toMatchObject({ kind: 'travel', parcel: { x: 160, y: 155 } })
    expect(resolve('mana/-150,150', inGenesis)).toMatchObject({ kind: 'travel', parcel: { x: -150, y: 150 } })
  })

  it('the world the player is in is a teleport inside it, or nothing to do', () => {
    expect(resolve('boedo', inWorld)).toEqual({ kind: 'reply', message: '🟡 You are already in boedo!' })
    expect(resolve('Boedo.dcl.eth', inWorld)).toEqual({ kind: 'reply', message: '🟡 You are already in Boedo.dcl.eth!' })
    expect(resolve('genesis', inGenesis)).toEqual({ kind: 'reply', message: '🟡 You are already in genesis!' })
    expect(resolve('boedo/3,4', inWorld)).toMatchObject({ kind: 'teleport', x: 3, y: 4 })
    expect(resolve('boedo/roof', inWorld)).toEqual({ kind: 'teleport', x: 1, y: 2, spawnPoint: 'roof', reply: '🟢 You teleported to 1,2.' })
    // by its address on a worlds server too
    expect(resolve('https://worlds.example/world/Boedo.dcl.eth', inWorld)).toEqual({ kind: 'reply', message: '🟡 You are already in https://worlds.example/world/Boedo.dcl.eth!' })
    expect(resolve('https://worlds.example/world/mana.dcl.eth', inWorld).kind).toBe('travel')
    // Genesis City is no world, whatever a world is called
    expect(resolve('boedo', inGenesis).kind).toBe('travel')
  })

  it('during a travel, a place inside the realm travelled to is asked of the travel again', () => {
    const toWorld: GotoHere = { ...inWorld, heading: 'boedo.dcl.eth' }
    const toGenesis: GotoHere = { ...inGenesis, heading: GENESIS }
    const toAddress: GotoHere = { ...elsewhere, heading: 'https://realm.example/other' }
    // the realm by its name: its travel with the landing, answered as the travel is
    expect(resolve('boedo/3,4/roof', toWorld)).toEqual({ kind: 'travel', realm: 'boedo.dcl.eth', parcel: { x: 3, y: 4 }, spawnPoint: 'roof', reply: '🟢 Welcome to the boedo world!', world: 'boedo' })
    expect(resolve('boedo/roof', toWorld)).toMatchObject({ kind: 'travel', realm: 'boedo.dcl.eth', parcel: undefined, spawnPoint: 'roof' })
    expect(resolve('genesis/7,8', toGenesis)).toMatchObject({ kind: 'travel', realm: GENESIS, parcel: { x: 7, y: 8 }, reply: '🟢 Welcome to Genesis City!' })
    // a parcel of the realm: Genesis City's own, a local one, one of a realm of no known kind
    expect(resolve('7,8', toGenesis)).toEqual({ kind: 'travel', realm: GENESIS, parcel: { x: 7, y: 8 }, spawnPoint: undefined, reply: '🟢 You teleported to 7,8.' })
    expect(resolve('3,4', toWorld, true)).toEqual({ kind: 'travel', realm: 'boedo.dcl.eth', parcel: { x: 3, y: 4 }, spawnPoint: undefined, reply: '🟢 You teleported to 3,4.' })
    expect(resolve('2,3', toAddress)).toMatchObject({ kind: 'travel', realm: 'https://realm.example/other', parcel: { x: 2, y: 3 } })
    // held to the bounds of the realm travelled to
    expect(resolve('160,155', toWorld, true)).toEqual({ kind: 'reply', message: '🔴 Error: Parcel (160, 155) is outside of the bounds.' })
    // the realm's name alone is nothing to ask again, and a parcel alone is one of the world travelled to
    expect(resolve('boedo', toWorld)).toEqual({ kind: 'reply', message: '🟡 You are already in boedo!' })
    expect(resolve('10,20', toWorld)).toMatchObject({ kind: 'travel', realm: 'boedo.dcl.eth', parcel: { x: 10, y: 20 } })
  })
})

describe('formatConsoleReply', () => {
  it('drops HUD-hidden commands from the bare help listing', () => {
    const out = ['Available commands:', '  /teleport      - set location', '  /crdt_snapshot - Return the full live CRDT state', '  /emote         - emote'].join('\n')
    const shown = formatConsoleReply('help', [], out)
    expect(shown).toContain('/teleport')
    expect(shown).toContain('/emote')
    expect(shown).not.toContain('/crdt_snapshot')
    // Per-command help is passed through untouched.
    expect(formatConsoleReply('help', ['/teleport'], 'set location\n\nUsage: /teleport <X> <Y>')).toContain('Usage:')
  })

  it("trims the engine's unknown-command rejection to one sentence", () => {
    const rejection = 'Command not recognized: `/dance`. Recognized commands: ["/clear", "/emote", "/exit"]'
    expect(formatConsoleReply('dance', [], rejection)).toBe('Command not recognized: `/dance`. Type /commands for the list.')
  })

  it('reports empty output explicitly', () => {
    expect(formatConsoleReply('fps', ['60'], '  ')).toBe('(no output for /fps)')
    expect(formatConsoleReply('fps', ['60'], 'fps set to 60\n')).toBe('fps set to 60')
  })
})
