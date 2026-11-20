// Chat slash-commands — parity with bevy-ui-scene's `sendChatMessage` (ChatsAndLogs.tsx) and
// unity-explorer's IChatCommand set. Pure parser: turns a raw input line into an action the session
// dispatches (teleport / changeRealm / reload / console) or a system message to echo in chat.
//
// `/goto`, `/goto-local` and `/world` take the destinations of ../lobby/goto.ts's grammar and
// reuse the teleport/changeRealm bridge plumbing; `/reload` and `/commands` go through thin bridge handlers (reloadScene / consoleCommand). `/help` is a
// client-rendered system message; `/commands` surfaces the engine's own console command list.
// Any other `/word args` is passed through to the engine console (`consoleCommand`), except the
// HUD_HIDDEN_COMMANDS below; the engine itself only registers dev/cheat commands in preview mode.

import { type GotoTarget, WORLD_PARCEL_LIMIT, isGenesisParcel, isWorldName, parseGotoTarget, worldName } from '../lobby/goto'

export type ChatCommand =
  /** Not a command (no leading `/`) — send as a normal chat message. */
  | { kind: 'send'; text: string }
  /** `/goto <destination>`, `/goto-local <destination>` or `/world <world>`: `local` stays in the
   *  realm the player is in. `resolveGoto` turns it into the request to send. */
  | { kind: 'goto'; target: GotoTarget; local: boolean }
  /** `/reload` — reload the current scene. */
  | { kind: 'reload' }
  /** `/commands` — list the engine console commands. */
  | { kind: 'commands' }
  | { kind: 'debug'; target: string }
  /** Any other `/command args` — run it on the engine console (no leading slash, args pre-split). */
  | { kind: 'console'; command: string; args: string[] }
  /** `/help` (and invalid usage) — echo this text as a system message. */
  | { kind: 'system'; message: string }

// Each command's usage, as /help lists it and an invalid destination answers.
const GOTO_USAGE = '<b>/goto <i><x,y | x,y/spawn | random | crowd | world | world/x,y | world/spawn | world/x,y/spawn></i></b>\n  Teleport inside of Genesis or World'
const GOTO_LOCAL_USAGE = '<b>/goto-local <i><x,y | x,y/spawn | spawn></i></b>\n  Teleport inside of the current realm'
const WORLD_USAGE = '<b>/world <i><world></i></b>\n  Jump to a world (alias of /goto world)'
const usageReply = (usage: string): string => `🔴 Invalid parameters, usage:\n${usage}`
const invalid = (usage: string): ChatCommand => ({ kind: 'system', message: usageReply(usage) })

/** The `/help` body: the client (DCL) commands. Chat renders its `<b>` and `<i>`. */
export const HELP_TEXT = [
  'Available commands:',
  '<b>/help</b>\n  Show this help',
  GOTO_USAGE,
  GOTO_LOCAL_USAGE,
  WORLD_USAGE,
  '<b>/reload</b>\n  Reload the current scene',
  '<b>/commands</b>\n  List the engine console commands',
  '<b>/debug <i>[section | help]</i></b>\n  Open the debug panel, find a section, or list its sections',
  '<b>/<i><command> [args]</i></b>\n  Run any engine console command (see /commands; /help <command> for its usage)',
].join('\n')

/** Engine console commands chat never runs nor lists: other HUDs' and harnesses' tooling, and
 *  spawn/kill (`/spawn <url> true` starts a scene with super-user rights). */
export const HUD_HIDDEN_COMMANDS: ReadonlySet<string> = new Set([
  // scene inspector — read
  'set_scene',
  'scene_stats',
  'scene_target',
  'scene_logs',
  'scene_entities',
  'entity_components',
  'inspect_component',
  'scene_tree',
  'crdt_snapshot',
  'crdt_initial',
  'component_names',
  'component_default',
  'component_schema',
  'screenshot',
  // scene inspector — write / assets
  'set_component',
  'set_component_raw',
  'new_entity',
  'save_composite',
  'delete_component',
  'delete_entity',
  'freeze_scene',
  'unfreeze_scene',
  'tick_scene',
  'highlight',
  'scene_content',
  'asset_catalog',
  'init_asset',
  // preview-scene tooling
  'lock_preview',
  'unlock_preview',
  'show_ui',
  // session / agent harness
  'login_guest',
  'login_previous',
  'login_identity',
  'logout',
  'chat',
  // portable scenes (a spawn can be privileged)
  'spawn',
  'kill',
  // console no-op / native quit
  'clear',
  'exit',
])

/** Split a console argument string on whitespace, honouring "double" and 'single' quotes. */
export function splitArgs(rest: string): string[] {
  const out: string[] = []
  const re = /"([^"]*)"|'([^']*)'|(\S+)/g
  let m: RegExpExecArray | null
  while ((m = re.exec(rest)) != null) out.push(m[1] ?? m[2] ?? m[3])
  return out
}

/** Render an engine console reply as a chat system line. A bare `help` (`/commands`) drops the
 *  HUD-hidden commands; the engine's unknown-command rejection is cut to its first sentence (its
 *  "Recognized commands: [...]" tail is the whole registry). */
export function formatConsoleReply(command: string, args: string[], output: string): string {
  if (command === 'help' && args.length === 0) {
    return output
      .split('\n')
      .filter((line) => {
        const m = line.match(/^\s*\/(\S+)/)
        return m == null || !HUD_HIDDEN_COMMANDS.has(m[1])
      })
      .join('\n')
      .trim()
  }
  const unknown = output.match(/^Command not recognized: `[^`]*`/)
  if (unknown != null) return `${unknown[0]}. Type /commands for the list.`
  return output.trim() || `(no output for /${command})`
}

// A command takes one destination: a space after a comma is dropped ("10, 20"), and what is left
// holds no other space.
function destination(rest: string): string | null {
  const arg = rest.replace(/,\s+/g, ',')
  return arg === '' || /\s/.test(arg) ? null : arg
}

function parseGoto(rest: string): ChatCommand {
  const arg = destination(rest)
  return arg == null ? invalid(GOTO_USAGE) : { kind: 'goto', target: parseGotoTarget(arg), local: false }
}

// A parcel, a parcel and a spawn point, or a spawn point of the scene the player stands in (which
// the grammar reads as a world name).
function parseGotoLocal(rest: string): ChatCommand {
  const arg = destination(rest)
  if (arg == null) return invalid(GOTO_LOCAL_USAGE)
  const target = parseGotoTarget(arg)
  if (target.random || target.crowd) return invalid(GOTO_LOCAL_USAGE)
  if (target.world == null) return { kind: 'goto', target, local: true }
  if (target.parcel != null || target.spawnPoint != null || /[,/]/.test(target.world)) return invalid(GOTO_LOCAL_USAGE)
  return { kind: 'goto', target: { ...target, world: null, spawnPoint: target.world }, local: true }
}

// A world's name alone, whatever it spells ("random" is a world here).
function parseWorld(rest: string): ChatCommand {
  const arg = destination(rest)
  if (arg == null || arg.includes(',') || arg.includes('/')) return invalid(WORLD_USAGE)
  return { kind: 'goto', target: { world: arg, parcel: null, spawnPoint: null, random: false, crowd: false }, local: false }
}

/** Where the player is, for `resolveGoto`. */
export interface GotoHere {
  /** The realm's kind: Genesis City, a world, or any other (a scene preview, a realm of its own,
   *  or not known yet), which has no Genesis City to go back to and no known bounds. */
  kind: 'genesis' | 'world' | 'other'
  /** The world's name (`name.dcl.eth`, lowercase) in a world; null anywhere else. */
  realm: string | null
  parcel: { x: number; y: number }
}

/** What a `/goto` asks of the engine, or the line to answer with instead. `reply` is the answer
 *  once the request succeeds; `world` is the world as the player typed it. */
export type GotoPlan =
  | { kind: 'reply'; message: string }
  /** A teleport inside the realm the player is in. */
  | { kind: 'teleport'; x: number; y: number; spawnPoint?: string; reply: string }
  /** A realm change, to `parcel` or to the realm's own spawn. */
  | { kind: 'travel'; realm: string; parcel?: { x: number; y: number }; spawnPoint?: string; reply: string; world?: string }

const outside = (p: { x: number; y: number }): GotoPlan => ({ kind: 'reply', message: `🔴 Error: Parcel (${p.x}, ${p.y}) is outside of the bounds.` })
const teleported = (p: { x: number; y: number }): string => `🟢 You teleported to ${p.x},${p.y}.`
const REALM_URL = /^https?:\/\//i

/**
 * A parcel alone is a Genesis City parcel, reached by a realm change from a world, unless `local`.
 * In a realm of any other kind it is a parcel of that realm: a scene preview refuses realm changes,
 * and its parcels are the creator's own. A world the player is already in is a teleport inside it.
 * `genesisRealm` is the Genesis City realm's address. `random` and `crowd` are resolved to a
 * parcel by the caller.
 */
export function resolveGoto(target: GotoTarget, local: boolean, here: GotoHere, genesisRealm: string): GotoPlan {
  const spawnPoint = target.spawnPoint ?? undefined
  if (target.world == null) {
    const parcel = target.parcel ?? here.parcel
    const toGenesis = !local && here.kind === 'world'
    const inside =
      here.kind === 'genesis' || toGenesis
        ? isGenesisParcel(parcel.x, parcel.y)
        : here.kind === 'other' || (Math.abs(parcel.x) <= WORLD_PARCEL_LIMIT && Math.abs(parcel.y) <= WORLD_PARCEL_LIMIT)
    if (target.parcel != null && !inside) return outside(parcel)
    if (toGenesis) return { kind: 'travel', realm: genesisRealm, parcel, spawnPoint, reply: teleported(parcel) }
    return { kind: 'teleport', ...parcel, spawnPoint, reply: teleported(parcel) }
  }

  const name = target.world
  const toGenesis = name.toLowerCase() === 'genesis'
  const url = REALM_URL.test(name)
  // a realm's address goes as it is; anything else has to be a world's name
  if (!toGenesis && !url && !isWorldName(name)) return { kind: 'reply', message: usageReply(GOTO_USAGE) }
  const realm = toGenesis ? genesisRealm : url ? name : worldName(name)
  const there = toGenesis ? here.kind === 'genesis' : here.kind === 'world' && here.realm === realm.toLowerCase()
  if (there) {
    if (target.parcel == null && spawnPoint == null) return { kind: 'reply', message: `🟡 You are already in ${name}!` }
    const parcel = target.parcel ?? here.parcel
    return { kind: 'teleport', ...parcel, spawnPoint, reply: teleported(parcel) }
  }
  return { kind: 'travel', realm, parcel: target.parcel ?? undefined, spawnPoint, reply: `🟢 Welcome to the ${name} world!`, world: name }
}

/** Parse a raw chat input into an action. Non-`/` lines pass through as `send`. */
export function parseChatCommand(input: string): ChatCommand {
  const text = input.trim()
  if (!text.startsWith('/')) return { kind: 'send', text }

  const [word, ...restParts] = text.split(/\s+/)
  const rest = text.slice(word.length).trim()
  const name = word.slice(1).toLowerCase()
  switch (name) {
    case 'help':
      // `/help <command>` → the engine's per-command usage (its registry keys carry the slash).
      if (restParts.length > 0) return { kind: 'console', command: 'help', args: [`/${restParts[0].replace(/^\//, '')}`] }
      return { kind: 'system', message: HELP_TEXT }
    case 'debug': return { kind: 'debug', target: rest.trim() }
    case 'commands':
      return { kind: 'commands' }
    case 'reload':
      // Bare `/reload` targets the player's scene (the engine's bare `/reload` reloads every scene,
      // this HUD's bridge included); with a hash it is the engine command.
      if (restParts.length > 0) return { kind: 'console', command: 'reload', args: splitArgs(rest) }
      return { kind: 'reload' }
    case 'goto':
      return parseGoto(rest)
    case 'goto-local':
      return parseGotoLocal(rest)
    case 'world':
      return parseWorld(rest)
    default:
      if (name === '' || HUD_HIDDEN_COMMANDS.has(name)) {
        return { kind: 'system', message: `Unknown command ${word}. Type /help for the list.` }
      }
      return { kind: 'console', command: name, args: splitArgs(rest) }
  }
}
