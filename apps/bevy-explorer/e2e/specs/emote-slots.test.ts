// The QA Automation Suite's EMOTES cases beyond explorer-automation's EmoteWheelTests and
// BackpackEmotesTests (emote-wheel.test.ts, backpack-emotes.test.ts): the wheel showing what the
// Backpack assigned, a quick emote from its key, and an emote card animating the preview. The
// Backpack lists its slots as `<slot> <emote or None>` (backpack.ts emoteSlots); the wheel names them
// `Emote slot <n>` with `: <emote>` when one is set (emote-wheel.test.ts). The quick emotes are the
// engine's QuickEmote0 to 9 actions, bound to the numpad's digits (crates/common/src/inputs.rs).
//
// | Suite case   | Here                                                                                | Class   |
// | Wheel        | B opens the wheel; each of its ten slots names the emote the Backpack's slot holds   | Port    |
// | Play         | a numpad digit plays the emote of that slot                                          | Gap     |
// | Animations   | clicking an emote card in the Backpack plays it on the preview avatar                | Gap     |
//
// Gap, Play: the suite presses B and then a digit; the Bevy bindings are the numpad's digits alone,
// and the console's `/press_key` knows no numpad code (`unknown key code Numpad1`, CI run
// 43224979097), so the key cannot be pressed. The wheel's own click is emote-wheel.test.ts.
// Gap, Animations: selecting an emote card only selects it; the Backpack plays nothing on the
// preview (the HUD's `previewEmote` is the Shop's alone), so the preview stays idle (same run).
import assert from 'node:assert/strict'
import { spec } from '../driver/index.ts'
import { emoteSlots, openBackpack } from './backpack.ts'
import { closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const WHEEL = { name: 'Close emotes', role: 'button' } as const
const WHEEL_SLOT = /^Emote slot (\d)(?:: (.+))?$/
const CONTROL_TIMEOUT_MS = 20_000

const normalized = (name: string | undefined): string => (name ?? '').replace(/\s+/g, ' ').trim().toLowerCase()

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

scenario('a numpad digit plays the emote of that slot', async () => {}, {
  todo: "the console's /press_key knows no numpad code, and the quick emotes are bound to the numpad alone"
})

scenario('clicking an emote card in the Backpack plays it on the preview avatar', async () => {}, {
  todo: "the Bevy Backpack plays nothing on the preview when an emote card is selected (the HUD's previewEmote is the Shop's alone)"
})
