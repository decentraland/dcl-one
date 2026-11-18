// The QA Automation Suite's "Randomize outfit" and "Randomize undo / redo" cases (beyond
// explorer-automation): the RANDOMIZE button under the Backpack's avatar preview and its Undo outfit
// and Redo outfit buttons (BackpackPage.tsx RandomizeBar; the bridge scene's randomizer domain). A
// roll edits the Backpack's draft look, like an equip: the preview avatar wears it, and the engine's
// player takes it only when the Backpack closes. The history covers one Backpack session: closing
// resets it (useEngineSession's resetOutfitHistory).
//
// | Suite case            | Here                                                                         | Class   |
// | Randomize outfit      | RANDOMIZE dresses the preview in another look; the player keeps theirs until the close | Partial |
// | Randomize undo / redo | two rolls; Undo walks back to each, Redo forward again; the buttons enable as the history allows | Partial |
//
// Partial, both: the preview avatar is read through `/avatar_state all` (the report tagged
// `preview`), as the grid has no addressable equipped state. The suite closes the Backpack on the
// random look; here every scenario walks Undo back to the look it started from before closing, so
// the shared account is never left on a random look and the close deploys nothing (the deploy on
// close is what the equip scenarios cover). "Owned wearables per category, smart wearables skipped"
// is the randomizer's own unit tests' (react-web bridgeRandomizer.test.ts), not observable here.
import assert from 'node:assert/strict'
import { spec, until, type AvatarReport, type Explorer } from '../driver/index.ts'
import { openBackpack } from './backpack.ts'
import { PAGES, closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const RANDOMIZE = { name: 'RANDOMIZE', role: 'button' } as const
const UNDO = { name: 'Undo outfit', role: 'button' } as const
const REDO = { name: 'Redo outfit', role: 'button' } as const
/** A roll resolves the picked items over the network before the preview wears them. */
const ROLL_TIMEOUT_MS = 30_000
/** RANDOMIZE stays locked until the avatar's body shape is known. */
const READY_TIMEOUT_MS = 40_000
const SCENARIO_TIMEOUT_MS = 240_000

/** The Backpack's preview avatar. */
async function preview(explorer: Explorer): Promise<AvatarReport> {
  const found = (await explorer.avatarState('all')).find((report) => report.kind === 'preview')
  if (!found) throw new Error('no preview avatar in /avatar_state all while the Backpack is open')
  return found
}

/** A look as a comparable key: the body shape and the selected wearables, case- and order-free. */
function lookOf(avatar: AvatarReport): string {
  const wearables = avatar.selected.wearables.map((urn) => urn.toLowerCase()).sort()
  return [avatar.selected.bodyShape?.toLowerCase() ?? '', ...wearables].join('|')
}

async function waitPreviewLook(explorer: Explorer, done: (look: string) => boolean, what: string): Promise<string> {
  return lookOf(await until(() => preview(explorer), (avatar) => done(lookOf(avatar)), { timeoutMs: ROLL_TIMEOUT_MS, intervalMs: 250, what }))
}

/** Roll once and wait for the preview to wear a look other than `from`. */
async function roll(explorer: Explorer, from: string): Promise<string> {
  await explorer.hud.click(RANDOMIZE)
  await explorer.hud.wait(UNDO, { state: 'enabled', timeoutMs: ROLL_TIMEOUT_MS })
  return waitPreviewLook(explorer, (look) => look !== from, 'the preview avatar to wear the rolled look')
}

async function press(explorer: Explorer, button: typeof UNDO | typeof REDO, to: string, what: string): Promise<void> {
  await explorer.hud.click(button)
  await waitPreviewLook(explorer, (look) => look === to, what)
}

async function isEnabled(explorer: Explorer, button: typeof UNDO | typeof REDO): Promise<boolean> {
  return !(await explorer.hud.get(button)).disabled
}

/** Open the Backpack and wait for RANDOMIZE to unlock; the restore walks Undo back and closes it. */
async function startRandomizing(explorer: Explorer, restore: (undo: () => Promise<void>) => void): Promise<{ original: string; player: string }> {
  await openBackpack(explorer, 'wearables')
  await explorer.hud.wait(RANDOMIZE, { state: 'enabled', timeoutMs: READY_TIMEOUT_MS })
  const original = lookOf(await preview(explorer))
  const player = lookOf(await explorer.myAvatar())
  restore(async () => {
    if (!(await explorer.hud.isVisible(PAGES.backpack.marker))) return
    // Undo is enabled while a roll is behind the current look; each press is one step back.
    for (let step = 0; step < 10 && (await explorer.hud.isVisible(UNDO)) && (await isEnabled(explorer, UNDO)); step++) {
      await explorer.hud.click(UNDO)
      await explorer.hud.wait(RANDOMIZE, { state: 'enabled', timeoutMs: ROLL_TIMEOUT_MS })
    }
    await closePage(explorer, 'backpack')
  })
  return { original, player }
}

scenario(
  "RANDOMIZE dresses the preview avatar in another look, and the player's avatar keeps theirs until the Backpack closes",
  async ({ explorer, restore }) => {
    const { original, player } = await startRandomizing(explorer, restore)
    assert.equal(await isEnabled(explorer, UNDO), false, 'Undo outfit should be disabled before any roll')

    const rolled = await roll(explorer, original)
    assert.notEqual(rolled, original, 'the preview should wear a new look after RANDOMIZE')
    assert.equal(await isEnabled(explorer, REDO), false, 'Redo outfit should be disabled right after a roll')
    assert.equal(lookOf(await explorer.myAvatar()), player, "the player's avatar should keep its look while the roll is a draft")

    await press(explorer, UNDO, original, 'the preview avatar to wear the starting look again')
    await closePage(explorer, 'backpack')
    assert.equal(lookOf(await explorer.myAvatar()), player, "the player's avatar should keep its look once the roll is undone and the Backpack closed")
  },
  { timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  'Undo walks back through two rolls and Redo re-applies them',
  async ({ explorer, restore }) => {
    const { original } = await startRandomizing(explorer, restore)
    const first = await roll(explorer, original)
    const second = await roll(explorer, first)

    await press(explorer, UNDO, first, 'Undo to bring back the first roll')
    assert.equal(await isEnabled(explorer, REDO), true, 'Redo outfit should be enabled after an Undo')
    await press(explorer, UNDO, original, 'a second Undo to bring back the starting look')
    assert.equal(await isEnabled(explorer, UNDO), false, 'Undo outfit should be disabled once back at the starting look')

    await press(explorer, REDO, first, 'Redo to re-apply the first roll')
    await press(explorer, REDO, second, 'a second Redo to re-apply the second roll')
    assert.equal(await isEnabled(explorer, REDO), false, 'Redo outfit should be disabled at the newest roll')

    await press(explorer, UNDO, first, 'Undo back toward the starting look')
    await press(explorer, UNDO, original, 'Undo back to the starting look')
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: SCENARIO_TIMEOUT_MS }
)
