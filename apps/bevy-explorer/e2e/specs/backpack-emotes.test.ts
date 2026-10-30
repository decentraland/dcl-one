// explorer-automation `BackpackEmotesTests` (4): fill the emote slots from the grid, search, unequip
// with a slot's own button, paginate. Unity's slot index i is the wheel slot number i: the HUD
// lists the slots 1 to 9 then 0, each as `<slot> <emote or None>`, and `selected.emotes[i]` in
// `/avatar_state` is the same slot.
//
// | Unity test                         | Here                                                                | Class   |
// | TestUnequipAndEquipAllEmoteSlots   | clear the ten slots, fill them from the first ten cards, check them | Partial |
// | TestSearchAndEquipFistPump         | search for Fist, equip the Fist Pump card into slot 0               | Partial |
// | TestUnequipEmoteWithSlotButton     | equip into slot 4, remove it with the slot's Unequip button         | Partial |
// | TestEmotesPagination               | the pager's arrows change the first card and bring page 1 back      | Partial |
//
// Partial, all four: Unity reads each tile's `EquippedSlot` badge and the slot's `EmoteName` and
// `EmptyEmoteName` labels. The Bevy card has no addressable badge, so "equipped" is two reads: the
// card offers UNEQUIP (not EQUIP) when the pointer is on it, and the slot control is named
// `<slot> <emote name>` instead of `<slot> None`. An equip only edits the Backpack's draft look, so
// the engine is read after the Backpack closes: `selected.emotes` in `/avatar_state`. The profile
// version is not waited on here: when the net look is the one already deployed (a rerun refills the
// same slots) the Backpack deploys nothing and the version stays. The three scenarios that change
// the slots put them back as they found them afterwards, whatever the outcome (backpack.ts).
//
// Partial, TestSearchAndEquipFistPump: the HUD names the base emote `Fistpump` (it title-cases the
// urn's last word: bridge-scene emotes.ts), so the search is for `Fist` and the card is the first
// whose name matches /fist ?pump/i.
//
// Partial, TestEmotesPagination: the pager shows only past 16 emotes (Unity's account has more).
// The HUD lists the ten base emotes plus the owned ones, so a base-only account has no second page;
// the scenario is then skipped with that reason, which the report counts apart from a pass.
import assert from 'node:assert/strict'
import { spec, until } from '../driver/index.ts'
import {
  COMMIT_TIMEOUT_MS,
  EQUIP_TIMEOUT_MS,
  type Card,
  emoteCards,
  emoteSlot,
  equipEmoteIntoSlot,
  flipPage,
  hoverCard,
  normalize,
  openBackpack,
  pagerVisible,
  recordEmoteSlots,
  restoreEmoteSlots,
  sameCards,
  searchFor,
  selectEmoteSlot,
  stableFirstCard,
  unequipAllEmotes,
  waitForAction,
  waitForCards,
  waitForPlayer
} from './backpack.ts'
import { closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const SLOTS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]

scenario(
  'all ten emote slots can be filled from the grid and stay equipped',
  async ({ explorer, restore }) => {
    await openBackpack(explorer, 'emotes')
    const cards = await waitForCards(explorer, emoteCards)
    assert.ok(cards.length >= SLOTS.length, `the account should list at least ${SLOTS.length} emotes on page 1, not ${cards.length}`)
    const original = await recordEmoteSlots(explorer)
    restore(() => restoreEmoteSlots(explorer, original))
    await unequipAllEmotes(explorer)
    for (const slot of SLOTS) await equipEmoteIntoSlot(explorer, slot, cards[slot]!)
    for (const slot of SLOTS) {
      const card = cards[slot]!
      const { action } = await hoverCard(explorer, card)
      assert.equal(action?.label, 'UNEQUIP', `Emote ${slot} ("${card.name}") should still be equipped after all ten slots are filled`)
      assert.equal(normalize((await emoteSlot(explorer, slot)).emote), normalize(card.name), `slot ${slot} should still hold "${card.name}"`)
    }
    await closePage(explorer, 'backpack')
    const after = await waitForPlayer(
      explorer,
      (avatar) => SLOTS.every((slot) => (avatar.selected.emotes[slot] ?? '') !== ''),
      COMMIT_TIMEOUT_MS,
      'the engine to select an emote in every slot'
    )
    for (const slot of SLOTS) assert.notEqual(after.selected.emotes[slot] ?? '', '', `the engine should select an emote in slot ${slot}`)
  },
  { timeoutMs: 400_000 }
)

scenario(
  'searching for Fist and equipping the Fist Pump emote names it in slot 0',
  async ({ explorer, restore }) => {
    const term = 'Fist'
    const fistPump = /fist ?pump/i
    await openBackpack(explorer, 'emotes')
    const original = await recordEmoteSlots(explorer)
    restore(() => restoreEmoteSlots(explorer, original))
    const unfiltered = await emoteCards(explorer)
    await searchFor(explorer, term)
    await until(() => emoteCards(explorer), (cards) => cards.length > 0 && !sameCards(cards, unfiltered), {
      timeoutMs: 40_000,
      intervalMs: 500,
      what: `the emote grid to refilter to "${term}"`
    })
    const card = (await waitForCards(explorer, emoteCards)).find((candidate) => fistPump.test(candidate.name))
    assert.ok(card, "Emote grid should refilter to the 'Fist Pump' search results")
    await unequipAllEmotes(explorer)
    await equipEmoteIntoSlot(explorer, 0, card)
    const slot = await emoteSlot(explorer, 0)
    assert.notEqual(slot.emote, 'None', 'Slot 0 should hold an emote after the equip, but it is still empty')
    assert.match(slot.emote, fistPump, `Slot 0 label '${slot.label}' should name Fist Pump`)
    await closePage(explorer, 'backpack')
    const after = await waitForPlayer(explorer, (avatar) => (avatar.selected.emotes[0] ?? '') !== '', COMMIT_TIMEOUT_MS, 'the engine to select an emote in slot 0')
    assert.notEqual(after.selected.emotes[0] ?? '', '', 'the engine should select an emote in slot 0')
  },
  { timeoutMs: 300_000 }
)

scenario(
  'an emote equipped into slot 4 is removed with the slot own Unequip button',
  async ({ explorer, restore }) => {
    const slot = 4
    await openBackpack(explorer, 'emotes')
    const cards = await waitForCards(explorer, emoteCards)
    const original = await recordEmoteSlots(explorer)
    restore(() => restoreEmoteSlots(explorer, original))
    let found: Card | undefined
    for (const card of cards.slice(0, 12)) {
      if ((await hoverCard(explorer, card)).action?.label === 'EQUIP') {
        found = card
        break
      }
    }
    assert.ok(found, 'at least one of the first twelve emotes should be loaded and not equipped')
    const card = found
    const over = await equipEmoteIntoSlot(explorer, slot, card)
    await waitForAction(explorer, over, 'UNEQUIP', EQUIP_TIMEOUT_MS)
    assert.equal(normalize((await emoteSlot(explorer, slot)).emote), normalize(card.name), `Precondition: "${card.name}" should be equipped before the unequip is exercised`)

    const target = await selectEmoteSlot(explorer, slot)
    assert.ok(target.unequip, `slot ${slot} should offer its own Unequip button`)
    await explorer.hud.click(target.unequip)
    await until(async () => (await hoverCard(explorer, card)).action?.label, (label) => label === 'EQUIP', {
      timeoutMs: 40_000,
      intervalMs: 1_000,
      what: `"${card.name}" to offer EQUIP again`
    })
    await until(async () => (await emoteSlot(explorer, slot)).emote, (emote) => emote === 'None', {
      timeoutMs: 40_000,
      intervalMs: 1_000,
      what: `emote slot ${slot} to read empty`
    })
    await closePage(explorer, 'backpack')
    await waitForPlayer(explorer, (avatar) => (avatar.selected.emotes[slot] ?? '') === '', COMMIT_TIMEOUT_MS, `the engine to select no emote in slot ${slot}`)
  },
  { timeoutMs: 300_000 }
)

scenario(
  'paging the emotes grid forward and back changes the first card and returns to page 1',
  async ({ explorer, t }) => {
    await openBackpack(explorer, 'emotes')
    if (!(await pagerVisible(explorer))) {
      const cards = await emoteCards(explorer)
      await closePage(explorer, 'backpack')
      t.skip(`the account has one page of emotes (${cards.length}), so there is no pager to exercise`)
      return
    }
    const first = await stableFirstCard(explorer, emoteCards)
    const second = await flipPage(explorer, 'Next page', emoteCards, first)
    assert.notEqual(second, first, 'First emote on page 2 should differ from first emote on page 1')
    const back = await flipPage(explorer, 'Previous page', emoteCards, second)
    assert.equal(back, first, "Navigating back should show page 1's first emote again")
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 300_000 }
)
