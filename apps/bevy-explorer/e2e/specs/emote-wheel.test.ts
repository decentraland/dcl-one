// explorer-automation `EmoteWheelTests` (3): the wheel opens from the rail or with B, shows its ten
// slots, and a slot plays its emote and closes the wheel. The Bevy wheel is a set of controls
// named `Emote slot <n>` (`: <emote name>` once a slot holds one; slots 1 to 9, then 0) with a
// `Close emotes` button and `Customise [E]`; its title is text without a role.
//
// | Unity test                                        | Here                                                                             | Class   |
// | TestOpenEmoteWheelFromSidebar                     | the rail's Emotes button opens it; Customise [E] stands in for the Emotes title  | Partial |
// | TestOpenEmoteWheelWithShortcutShowsAllSlots       | B opens it, ten Emote slot controls, B closes it                                 | Port    |
// | TestTriggerEmoteFromWheelClosesWheel              | the wheel closes, and /avatar_state shows the emote playing as a triggered emote | Port    |
//
// The third test asserts more than Unity's, which could not observe the animation.
import assert from 'node:assert/strict'
import { settled, spec, until, type Control, type Explorer } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000

const RAIL_EMOTES = { name: 'Emotes', role: 'button' } as const
const WHEEL = { name: 'Close emotes', role: 'button' } as const
const CUSTOMISE = { name: 'Customise [E]', role: 'button' } as const
const SLOT = /^Emote slot (\d)(?:: (.+))?$/
const SLOT_COUNT = 10

interface WheelSlot {
  slot: number
  /** The emote's name; undefined for an empty slot. */
  emote: string | undefined
}

/** The wheel's slots as the HUD lists them, by number. */
async function wheelSlots(explorer: Explorer): Promise<WheelSlot[]> {
  const buttons = await explorer.hud.list('button')
  const slots: WheelSlot[] = []
  for (const { name } of buttons) {
    const match = SLOT.exec(name)
    if (match) slots.push({ slot: Number(match[1]), emote: match[2] })
  }
  return slots.sort((a, b) => a.slot - b.slot)
}

/** Two urns name the same emote: the same text, or a base emote's name against its full urn. */
function sameEmote(a: string, b: string): boolean {
  const first = a.toLowerCase()
  const second = b.toLowerCase()
  if (first === second) return true
  const [name, urn] = first.includes(':') ? [second, first] : [first, second]
  return !name.includes(':') && urn.endsWith(`:${name}`)
}

function sameBox(a: Control, b: Control): boolean {
  return a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height
}

scenario("the rail's Emotes button opens the wheel and Escape closes it", async ({ explorer }) => {
  await explorer.hud.click(RAIL_EMOTES)
  await explorer.hud.wait(WHEEL, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.ok(await explorer.hud.isVisible(CUSTOMISE), 'Emote wheel should show its Customise control next to its title')
  assert.equal((await explorer.hud.get(RAIL_EMOTES)).checked, true, 'the rail button should show the wheel as open')

  await explorer.escape()
  await explorer.hud.waitHidden(WHEEL, CONTROL_TIMEOUT_MS)
})

scenario('B opens the wheel with all ten slots and a second B closes it', async ({ explorer }) => {
  await explorer.key('KeyB')
  await explorer.hud.wait(WHEEL, { timeoutMs: CONTROL_TIMEOUT_MS })
  const present = new Set((await wheelSlots(explorer)).map((entry) => entry.slot))
  const missing = Array.from({ length: SLOT_COUNT }, (_, slot) => slot).filter((slot) => !present.has(slot))
  assert.deepEqual(missing, [], `All 10 wheel slots should be present, missing: ${missing.join(', ')}`)

  await explorer.key('KeyB')
  await explorer.hud.waitHidden(WHEEL, CONTROL_TIMEOUT_MS)
})

scenario('clicking an emote slot plays the emote and closes the wheel', async ({ explorer }) => {
  await explorer.key('KeyB')
  await explorer.hud.wait(WHEEL, { timeoutMs: CONTROL_TIMEOUT_MS })
  const occupied = (await wheelSlots(explorer)).find((entry) => entry.emote !== undefined)
  assert.ok(occupied, 'No emote wheel slot has an emote loaded: cannot trigger an emote')
  const slot = { name: `Emote slot ${occupied.slot}: ${occupied.emote}`, role: 'button' } as const

  // The urn the slot holds: the profile's emote list is indexed by slot.
  const expected = (await explorer.myAvatar()).selected.emotes[occupied.slot]
  assert.ok(expected, `the player's emote ${occupied.slot} should name the emote the wheel shows, ${occupied.emote}`)

  // The wheel animates in: click once the slot stops moving, where Unity waits 0.5 s.
  await settled(() => explorer.hud.get(slot), { timeoutMs: TEXT_TIMEOUT_MS, intervalMs: 250, what: `${occupied.emote}'s slot to stop moving`, same: sameBox })
  await explorer.hud.click(slot)
  await explorer.hud.waitHidden(WHEEL, CONTROL_TIMEOUT_MS)

  await until(
    async () => (await explorer.myAvatar()).emote,
    (emote) => emote !== null && emote.source === 'triggeredEmote' && !emote.finished && sameEmote(emote.urn, expected),
    { timeoutMs: CONTROL_TIMEOUT_MS, intervalMs: 100, what: `the avatar to play ${expected} as a triggered emote` }
  )
})
