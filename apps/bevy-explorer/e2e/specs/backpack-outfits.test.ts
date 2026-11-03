// explorer-automation `BackpackOutfitsTests` (4): the Saved Outfits tab, save, equip, delete. The
// HUD offers five slots (ten with a claimed NAME) numbered from 1, as Unity's OutfitSlot_1 to 5.
// A slot is `Save outfit in slot <n>` when empty and `Outfit <n>` when saved, with `Delete Outfit <n>`
// beside it and an EQUIP button over it while the pointer is on it.
//
// | Unity test                    | Here                                                          | Class   |
// | TestOpenSavedOutfitsTab       | the three tabs show; Saved Outfits shows slot 1               | Port    |
// | TestSaveOutfit                | save the current look into the first empty slot               | Port    |
// | TestEquipFirstSavedOutfit     | change the hair, equip outfit 1, the engine drops the change  | Partial |
// | TestDeleteOutfit              | save a throwaway outfit and delete it, keeping slot 1         | Partial |
//
// Port, TestSaveOutfit: when all five slots are full the scenario deletes slot 5 first, as Unity
// does, but the HUD deletes at once (no confirmation dialog).
//
// Partial, TestEquipFirstSavedOutfit: Unity reads the hair tile's `IsEquipped` flag before and after
// the outfit. The Bevy grid has no flag, and equips (a hair, an outfit) only edit the Backpack's
// draft look, which the engine takes when the Backpack closes. So the scenario closes the Backpack
// after the hair change and again after the outfit, and reads the engine each time: the hair change
// is selected after the first close, and after the second the avatar selects none of the wearables
// the hair change added.
//
// Partial, TestDeleteOutfit: Unity deletes through a confirmation dialog (Yes button, dialog shown
// and hidden). The Bevy delete button acts at once, so the dialog steps are dropped; the slot
// reading empty again is the assertion, and slot 1 is checked to have stayed.
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'
import {
  COMMIT_TIMEOUT_MS,
  EQUIP_TIMEOUT_MS,
  commitBackpack,
  equipFirstUnequippedHair,
  gained,
  hoverCard,
  openBackpack,
  outfitSlots,
  sameUrn,
  showWearableGrid,
  startLook,
  waitForPlayer
} from './backpack.ts'
import { closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const SLOTS = [1, 2, 3, 4, 5]

const savedOutfit = (slot: number): Selector => ({ name: `Outfit ${slot}`, role: 'button' })
const emptySlot = (slot: number): Selector => ({ name: `Save outfit in slot ${slot}`, role: 'button' })
const deleteButton = (slot: number): Selector => ({ name: `Delete Outfit ${slot}`, role: 'button' })

async function saved(explorer: Explorer): Promise<number[]> {
  return (await outfitSlots(explorer)).filter(([slot, full]) => full && SLOTS.includes(slot)).map(([slot]) => slot)
}

async function firstEmptySlot(explorer: Explorer): Promise<number | undefined> {
  const full = await saved(explorer)
  return SLOTS.find((slot) => !full.includes(slot))
}

/** Save the current look into an empty slot and wait for it to read saved. */
async function saveIn(explorer: Explorer, slot: number, timeoutMs = 30_000): Promise<void> {
  await explorer.hud.click(emptySlot(slot))
  await explorer.hud.wait(savedOutfit(slot), { timeoutMs })
}

/** Delete a saved outfit (no confirmation dialog) and wait for the slot to read empty. */
async function deleteIn(explorer: Explorer, slot: number): Promise<void> {
  await explorer.hud.click(deleteButton(slot))
  await explorer.hud.wait(emptySlot(slot), { timeoutMs: 30_000 })
}

async function ensureFirstSlotSaved(explorer: Explorer): Promise<void> {
  if (!(await saved(explorer)).includes(1)) await saveIn(explorer, 1, 40_000)
}

/** Put the pointer on a saved outfit and press the EQUIP that appears over it. */
async function equipOutfit(explorer: Explorer, slot: number): Promise<void> {
  const { action } = await hoverCard(explorer, { name: `Outfit ${slot}`, selector: savedOutfit(slot) }, ['EQUIP'])
  if (!action) throw new Error(`Outfit ${slot} offered no EQUIP button`)
  await explorer.hud.click(action.selector)
}

scenario(
  'the Backpack shows its three tabs and the Saved Outfits slots',
  async ({ explorer }) => {
    await openWithKey(explorer, 'backpack')
    for (const tab of ['Wearables', 'Emotes', 'Saved Outfits']) {
      assert.ok(await explorer.hud.isVisible({ name: tab, role: 'tab' }), `${tab} tab button should be visible`)
    }
    await explorer.hud.click({ name: 'Saved Outfits', role: 'tab' })
    await until(() => outfitSlots(explorer), (slots) => slots.some(([slot]) => slot === 1), { timeoutMs: 20_000, what: 'the first outfit slot to show' })
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 120_000 }
)

scenario(
  'the current look is saved into an empty outfit slot',
  async ({ explorer }) => {
    await openBackpack(explorer, 'outfits')
    let slot = await firstEmptySlot(explorer)
    if (slot === undefined) {
      slot = 5
      await deleteIn(explorer, slot)
    }
    await saveIn(explorer, slot)
    assert.ok(await explorer.hud.isVisible(savedOutfit(slot)), `outfit slot ${slot} should read saved`)
    assert.ok(await explorer.hud.isVisible(deleteButton(slot)), `outfit slot ${slot} should offer its delete button`)
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 180_000 }
)

scenario(
  'equipping the first saved outfit reverts a hair change',
  async ({ explorer }) => {
    await openBackpack(explorer, 'outfits')
    await ensureFirstSlotSaved(explorer)
    await showWearableGrid(explorer)
    const { name, start } = await equipFirstUnequippedHair(explorer)
    await commitBackpack(explorer, start, `changing the hair to "${name}"`)
    const changed = await waitForPlayer(
      explorer,
      (avatar) => gained(start.before.selected.wearables, avatar.selected.wearables).length > 0,
      EQUIP_TIMEOUT_MS,
      `the avatar to select the alternative hair ("${name}")`
    )
    const added = gained(start.before.selected.wearables, changed.selected.wearables)
    assert.ok(added.length > 0, 'Precondition: the alternative hair should be equipped')

    await openBackpack(explorer, 'outfits')
    const second = await startLook(explorer)
    await equipOutfit(explorer, 1)
    await commitBackpack(explorer, second, 'equipping outfit 1')
    const reverted = await waitForPlayer(
      explorer,
      (avatar) => !avatar.selected.wearables.some((urn) => added.some((alternative) => sameUrn(alternative, urn))),
      COMMIT_TIMEOUT_MS,
      'the avatar to stop selecting the alternative hair'
    )
    assert.ok(
      !reverted.selected.wearables.some((urn) => added.some((alternative) => sameUrn(alternative, urn))),
      'Equipping the saved outfit should have reverted the hair change'
    )
  },
  { timeoutMs: 400_000 }
)

scenario(
  'a throwaway outfit is deleted and slot 1 stays',
  async ({ explorer }) => {
    await openBackpack(explorer, 'outfits')
    await ensureFirstSlotSaved(explorer)
    let target = await firstEmptySlot(explorer)
    if (target === undefined) {
      target = 5
    } else {
      await saveIn(explorer, target)
    }
    await deleteIn(explorer, target)
    assert.ok(await explorer.hud.isVisible(emptySlot(target)), `outfit slot ${target} should read empty after its delete`)
    assert.ok(await explorer.hud.isVisible(savedOutfit(1)), 'outfit slot 1 should still be saved')
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 180_000 }
)
