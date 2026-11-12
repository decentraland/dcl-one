// The QA Automation Suite's OUTFITS cases beyond explorer-automation's BackpackOutfitsTests
// (backpack-outfits.test.ts: the tab, save, equip, delete): a saved outfit outliving the Backpack,
// the SHOP button and the delete's confirmation. A slot is `Save outfit in slot <n>` when empty and
// `Outfit <n>` when saved, with `Delete Outfit <n>` beside it (BackpackPage.tsx OutfitSlotCard).
//
// | Suite case           | Here                                                                         | Class   |
// | Check outfits        | an outfit saved, the Backpack closed and opened again: the slot still reads saved | Partial |
// | Outfits shop         | the Wearables tab shows SHOP, actionable                                      | Partial |
// | Delete outfit (cancel) | cancelling the "Are you sure?" prompt keeps the outfit                     | Gap     |
//
// Partial, Check outfits: the suite goes in world between the close and the reopen, which this is,
// on the same client. Whether the outfit outlives the session (another device or client) is not
// observable from one client: the catalyst does not store Bevy's outfits (Notion A2). The outfit is
// saved into the first empty slot of 1 to 5 and deleted again, whatever the outcome; with all five
// full the scenario is skipped, as no slot can be saved without losing an outfit.
// Partial, Outfits shop: SHOP opens the Marketplace in a new browser tab (window.open), which the
// console cannot follow and which would take the window from the engine; it is checked to be there
// and actionable, not pressed.
// Gap, Delete outfit: the HUD deletes at once, with no confirmation (Notion U5); the delete itself
// is backpack-outfits'.
import assert from 'node:assert/strict'
import { spec, type Explorer, type Selector } from '../driver/index.ts'
import { openBackpack, outfitSlots, waitForOutfitSlots } from './backpack.ts'
import { PAGES, closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const SLOTS = [1, 2, 3, 4, 5]
const SHOP = { name: 'SHOP', role: 'button' } as const
const SAVE_TIMEOUT_MS = 40_000

const savedOutfit = (slot: number): Selector => ({ name: `Outfit ${slot}`, role: 'button' })
const emptySlot = (slot: number): Selector => ({ name: `Save outfit in slot ${slot}`, role: 'button' })
const deleteButton = (slot: number): Selector => ({ name: `Delete Outfit ${slot}`, role: 'button' })

async function firstEmptySlot(explorer: Explorer): Promise<number | undefined> {
  const full = (await outfitSlots(explorer)).filter(([, saved]) => saved).map(([slot]) => slot)
  return SLOTS.find((slot) => !full.includes(slot))
}

async function deleteIn(explorer: Explorer, slot: number): Promise<void> {
  await explorer.hud.click(deleteButton(slot))
  await explorer.hud.wait(emptySlot(slot), { timeoutMs: SAVE_TIMEOUT_MS })
}

scenario(
  'an outfit saved into a slot still reads saved after the Backpack is closed and opened again',
  async ({ explorer, restore, t }) => {
    await openBackpack(explorer, 'outfits')
    const slot = await firstEmptySlot(explorer)
    if (slot === undefined) {
      t.skip('outfit slots 1 to 5 are all saved, so none can take a new outfit without losing one')
      await closePage(explorer, 'backpack')
      return
    }
    // Set while the scenario's outfit is saved: the restore deletes it only then.
    let pending = false
    restore(async () => {
      if (!pending) return
      if (!(await explorer.hud.isVisible(PAGES.backpack.marker))) await openBackpack(explorer, 'outfits')
      await waitForOutfitSlots(explorer)
      if (await explorer.hud.isVisible(savedOutfit(slot))) await deleteIn(explorer, slot)
      await closePage(explorer, 'backpack')
    })

    pending = true
    await explorer.hud.click(emptySlot(slot))
    await explorer.hud.wait(savedOutfit(slot), { timeoutMs: SAVE_TIMEOUT_MS })
    await closePage(explorer, 'backpack')

    await openBackpack(explorer, 'outfits')
    assert.ok(await explorer.hud.isVisible(savedOutfit(slot)), `outfit slot ${slot} should still read saved after the Backpack reopened`)
    await deleteIn(explorer, slot)
    pending = false
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 240_000 }
)

scenario('the Wearables tab offers the SHOP button', async ({ explorer }) => {
  await openBackpack(explorer, 'outfits')
  await explorer.hud.wait(SHOP, { state: 'actionable', timeoutMs: 10_000 })
  await closePage(explorer, 'backpack')
})

scenario('cancelling the delete prompt keeps the outfit in its slot', async () => {}, {
  todo: 'the HUD deletes a saved outfit at once, with no "Are you sure?" prompt to cancel (Notion U5)'
})
