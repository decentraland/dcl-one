import { describe, expect, it } from 'vitest'
import { formatConsoleReply, HUD_HIDDEN_COMMANDS, parseChatCommand, resolveGoto, splitArgs, HELP_TEXT } from './chatCommands'
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
      expect((r as { message: string }).message).toContain('🔴 Invalid parameters, usage:\n<b>/goto-local ')
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
      message: '🔴 Invalid parameters, usage:\n<b>/goto <i><x,y | x,y/spawn | random | crowd | world | world/x,y | world/spawn | world/x,y/spawn></i></b>\n  Teleport inside of Genesis or World'
    })
    expect(parseChatCommand('/world').kind).toBe('system')
    expect(parseChatCommand('/goto 1 2 3').kind).toBe('system')
    expect(parseChatCommand('/goto 10 ,20').kind).toBe('system')
    expect(parseChatCommand('/goto boedo/main lobby').kind).toBe('system')
  })

  it('any other /command passes through to the engine console with split args', () => {
    expect(parseChatCommand('/dance')).toEqual({ kind: 'console', command: 'dance', args: [] })
    expect(parseChatCommand('/Emote 3')).toEqual({ kind: 'console', command: 'emote', args: ['3'] })
    expect(parseChatCommand('/changerealm boedo.dcl.eth "https://x.y/content"')).toEqual({
      kind: 'console',
      command: 'changerealm',
      args: ['boedo.dcl.eth', 'https://x.y/content']
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
    expect(parseChatCommand('/reload bafyabc')).toEqual({ kind: 'console', command: 'reload', args: ['bafyabc'] })
    expect(parseChatCommand('/help teleport')).toEqual({ kind: 'console', command: 'help', args: ['/teleport'] })
    expect(parseChatCommand('/help /teleport')).toEqual({ kind: 'console', command: 'help', args: ['/teleport'] })
    expect(parseChatCommand('/help').kind).toBe('system')
  })

  it('splitArgs honours quotes', () => {
    expect(splitArgs(`a "b c" 'd e' f`)).toEqual(['a', 'b c', 'd e', 'f'])
    expect(splitArgs('')).toEqual([])
  })
})

describe('resolveGoto', () => {
  const GENESIS = 'https://realm.example/main'
  const inGenesis = { genesis: true, realm: GENESIS, parcel: { x: 5, y: 6 } }
  const inWorld = { genesis: false, realm: 'boedo.dcl.eth', parcel: { x: 1, y: 2 } }
  const resolve = (text: string, here: typeof inGenesis, local = false) => resolveGoto(parseGotoTarget(text), local, here, GENESIS)

  it('a parcel is a Genesis City parcel: a teleport there, a realm change from a world', () => {
    expect(resolve('10,20', inGenesis)).toEqual({ kind: 'teleport', x: 10, y: 20, spawnPoint: undefined, reply: '🟢 You teleported to 10,20.' })
    expect(resolve('10,20/lobby', inGenesis)).toMatchObject({ kind: 'teleport', x: 10, y: 20, spawnPoint: 'lobby' })
    expect(resolve('10,20', inWorld)).toEqual({ kind: 'travel', realm: GENESIS, parcel: { x: 10, y: 20 }, spawnPoint: undefined, reply: '🟢 You teleported to 10,20.' })
    expect(resolve('152,0', inGenesis)).toEqual({ kind: 'reply', message: '🔴 Error: Parcel (152, 0) is outside of the bounds.' })
    // judged against Genesis City from a world too
    expect(resolve('160,155', inWorld).kind).toBe('travel')
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
    expect(resolve('main', inGenesis)).toMatchObject({ kind: 'travel', realm: 'main.dcl.eth' })
    expect(resolve('genesis', inWorld)).toMatchObject({ kind: 'travel', realm: GENESIS, reply: '🟢 Welcome to the genesis world!' })
    expect(resolve('https://realm.example/other', inWorld)).toMatchObject({ kind: 'travel', realm: 'https://realm.example/other' })
  })

  it('the world the player is in is a teleport inside it, or nothing to do', () => {
    expect(resolve('boedo', inWorld)).toEqual({ kind: 'reply', message: '🟡 You are already in boedo!' })
    expect(resolve('Boedo.dcl.eth', inWorld)).toEqual({ kind: 'reply', message: '🟡 You are already in Boedo.dcl.eth!' })
    expect(resolve('genesis', inGenesis)).toEqual({ kind: 'reply', message: '🟡 You are already in genesis!' })
    expect(resolve('boedo/3,4', inWorld)).toMatchObject({ kind: 'teleport', x: 3, y: 4 })
    expect(resolve('boedo/roof', inWorld)).toEqual({ kind: 'teleport', x: 1, y: 2, spawnPoint: 'roof', reply: '🟢 You teleported to 1,2.' })
    // not the world of that name: Genesis City's realm is not a world
    expect(resolve('boedo', { ...inGenesis, realm: 'boedo.dcl.eth' }).kind).toBe('travel')
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
