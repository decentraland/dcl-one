// The QA Automation Suite's EMOTES cases beyond explorer-automation's EmoteWheelTests and
// BackpackEmotesTests (emote-wheel.test.ts, backpack-emotes.test.ts): the wheel showing what the
// Backpack assigned, a quick emote from its key, and an emote card animating the preview. The
// Backpack lists its slots as `<slot> <emote or None>` (backpack.ts emoteSlots); the wheel names them
// `Emote slot <n>` with `: <emote>` when one is set (emote-wheel.test.ts). The quick emotes are the
// engine's QuickEmote0 to 9 actions, bound to the numpad's digits (crates/common/src/inputs.rs).
//
// | Suite case   | Here                                                                                | Class   |
// | Wheel        | B opens the wheel; each of its ten slots names the emote the Backpack's slot holds   | Port    |
// | Play         | a numpad digit plays the emote of that slot, read through /avatar_state              | Partial |
// | Animations   | clicking an emote card in the Backpack plays an emote on the preview avatar          | Partial |
//
// Partial, Play: the suite presses B and then a digit; the Bevy bindings are the numpad's digits
// alone (the wheel's own click is emote-wheel.test.ts). The slot is the first one the profile fills.
// Partial, Animations: "the selected emote autoplays" is read on the preview avatar
// (`/avatar_state all`, the report tagged `preview`), as an emote that is not the idle or locomotion
// one (`velocitySelected`); the urn is reported, not compared with the
// card, whose name is the HUD's title-cased form of it.
import assert from 'node:assert/strict'
import { spec, until, type AvatarReport, type Explorer } from '../driver/index.ts'
import { emoteCards, emoteSlots, openBackpack, pointAtCard, waitForCards } from './backpack.ts'
import { closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const WHEEL = { name: 'Close emotes', role: 'button' } as const
const WHEEL_SLOT = /^Emote slot (\d)(?:: (.+))?$/
const CONTROL_TIMEOUT_MS = 20_000
const PLAY_TIMEOUT_MS = 20_000

const normalized = (name: string | undefined): string => (name ?? '').replace(/\s+/g, ' ').trim().toLowerCase()

/** Two urns name the same emote: the same text, or a base emote's name against its full urn. */
function sameEmote(a: string, b: string): boolean {
  const first = a.toLowerCase()
  const second = b.toLowerCase()
  if (first === second) return true
  const [name, urn] = first.includes(':') ? [second, first] : [first, second]
  return !name.includes(':') && urn.endsWith(`:${name}`)
}

async function previewAvatar(explorer: Explorer): Promise<AvatarReport | undefined> {
  return (await explorer.avatarState('all')).find((report) => report.kind === 'preview')
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

scenario('a numpad digit plays the emote of that slot', async ({ explorer }) => {
  const emotes = (await explorer.myAvatar()).selected.emotes
  // The slots in the wheel's order, 1 to 9 then 0.
  const slot = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0].find((index) => (emotes[index] ?? '') !== '')
  assert.ok(slot !== undefined, 'the profile should hold an emote in at least one slot')
  const expected = emotes[slot]!

  await explorer.key(`Numpad${slot}`)
  await until(
    async () => (await explorer.myAvatar()).emote,
    (emote) => emote !== null && emote.source === 'triggeredEmote' && sameEmote(emote.urn, expected),
    { timeoutMs: PLAY_TIMEOUT_MS, intervalMs: 100, what: `Numpad${slot} to play ${expected}` }
  )
})

scenario(
  'clicking an emote card in the Backpack plays an emote on the preview avatar',
  async ({ explorer, t }) => {
    await openBackpack(explorer, 'emotes')
    const [card] = await waitForCards(explorer, emoteCards)
    assert.ok(card, 'the Emotes tab should list at least one emote')
    await pointAtCard(explorer, card.selector)
    const playing = await until(() => previewAvatar(explorer), (preview) => preview?.emote != null && preview.emote.source !== 'velocitySelected' && !preview.emote.finished, {
      timeoutMs: PLAY_TIMEOUT_MS,
      intervalMs: 100,
      what: `the preview avatar to play "${card.name}"`
    })
    t.diagnostic(`"${card.name}" plays ${playing!.emote!.urn} on the preview`)
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 180_000 }
)
