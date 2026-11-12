// The QA Automation Suite's EMOTES cases beyond explorer-automation's EmoteWheelTests and
// BackpackEmotesTests (emote-wheel.test.ts, backpack-emotes.test.ts): the wheel showing what the
// Backpack assigned, and a quick emote from B and a digit. The Backpack lists its slots as
// `<slot> <emote or None>` (backpack.ts emoteSlots); the wheel names them `Emote slot <n>` with
// `: <emote>` when one is set (emote-wheel.test.ts), and its hint reads "Press [B+num] to run an emote".
//
// | Suite case   | Here                                                                                | Class   |
// | Wheel        | B opens the wheel; each of its ten slots names the emote the Backpack's slot holds   | Port    |
// | Play         | B, then a slot's digit, plays that slot's emote and closes the wheel                 | Port    |
// | Animations   | none: the Bevy Backpack plays nothing on the preview when an emote is selected        | Drop    |
//
// Drop, Animations: by design the Backpack does not animate the preview avatar with the selected
// emote; the wheel's own click is emote-wheel.test.ts.
import assert from 'node:assert/strict'
import { spec, until } from '../driver/index.ts'
import { emoteSlots, openBackpack } from './backpack.ts'
import { closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const WHEEL = { name: 'Close emotes', role: 'button' } as const
const WHEEL_SLOT = /^Emote slot (\d)(?:: (.+))?$/
const CONTROL_TIMEOUT_MS = 20_000

const normalized = (name: string | undefined): string => (name ?? '').replace(/\s+/g, ' ').trim().toLowerCase()

/** Two urns name the same emote: the same text, or a base emote's name against its full urn. */
function sameEmote(a: string, b: string): boolean {
  const first = a.toLowerCase()
  const second = b.toLowerCase()
  if (first === second) return true
  const [name, urn] = first.includes(':') ? [second, first] : [first, second]
  return !name.includes(':') && urn.endsWith(`:${name}`)
}

scenario(
  "the emote wheel's slots name the emotes the Backpack assigned to them",
  async ({ explorer }) => {
    await openBackpack(explorer, 'emotes')
    const assigned = new Map((await emoteSlots(explorer)).map((slot) => [slot.slot, slot.emote === 'None' ? '' : normalized(slot.emote)]))
    assert.equal(assigned.size, 10, 'the Backpack should list ten emote slots')
    await closePage(explorer, 'backpack')

    await explorer.key('KeyB')
    await explorer.hud.wait(WHEEL, { timeoutMs: CONTROL_TIMEOUT_MS })
    const wheel = new Map<number, string>()
    for (const { name } of await explorer.hud.list('button')) {
      const match = WHEEL_SLOT.exec(name)
      if (match) wheel.set(Number(match[1]), normalized(match[2]))
    }
    const differing = [...assigned].filter(([slot, emote]) => wheel.get(slot) !== emote).map(([slot, emote]) => `${slot}: Backpack "${emote}", wheel "${wheel.get(slot) ?? 'missing'}"`)
    assert.deepEqual(differing, [], `every wheel slot should name the Backpack's emote: ${differing.join('; ')}`)
    await explorer.key('KeyB')
    await explorer.hud.waitHidden(WHEEL, CONTROL_TIMEOUT_MS)
  },
  { timeoutMs: 180_000 }
)

scenario(
  "B, then a slot's digit, plays that slot's emote and closes the wheel",
  async ({ explorer, restore, t }) => {
    restore(async () => {
      if (await explorer.hud.isVisible(WHEEL)) {
        await explorer.key('KeyB')
        await explorer.hud.waitHidden(WHEEL, CONTROL_TIMEOUT_MS)
      }
    })
    await explorer.key('KeyB')
    await explorer.hud.wait(WHEEL, { timeoutMs: CONTROL_TIMEOUT_MS })
    const occupied: number[] = []
    for (const { name } of await explorer.hud.list('button')) {
      const match = WHEEL_SLOT.exec(name)
      if (match?.[2]) occupied.push(Number(match[1]))
    }
    // Digits 1 to 4 are also the scenes' Action 3 to 6: a higher slot keeps the press the wheel's alone.
    const slot = occupied.find((n) => n === 0 || n > 4) ?? occupied[0]
    assert.ok(slot !== undefined, 'no emote wheel slot has an emote: nothing to play')
    const expected = (await explorer.myAvatar()).selected.emotes[slot]
    assert.ok(expected, `the player's emote ${slot} should name the emote the wheel shows`)
    t.diagnostic(`slot ${slot}: ${expected}`)

    await explorer.key(`Digit${slot}`)
    await until(
      async () => (await explorer.myAvatar()).emote,
      (emote) => emote !== null && emote.source === 'triggeredEmote' && !emote.finished && sameEmote(emote.urn, expected),
      { timeoutMs: CONTROL_TIMEOUT_MS, intervalMs: 100, what: `the avatar to play ${expected} after B and Digit${slot}` }
    )
    await explorer.hud.waitHidden(WHEEL, CONTROL_TIMEOUT_MS)
  },
  { timeoutMs: 120_000 }
)
