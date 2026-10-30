// explorer-automation `BackpackWearableVisibilityTests` (2): the avatar slots hold base wearables
// and show them, and equipping a hair shows in its slot. The Bevy Backpack's slots are the category
// tiles (`Hair`, `Eyes`, `Eyebrows`, `Mouth`, ...), each showing the equipped item's thumbnail.
//
// | Unity test                               | Here                                                                    | Class   |
// | TestBaseWearableSlotThumbnails           | the four tiles show; the engine renders a base wearable in each         | Partial |
// | TestEquipBaseWearableAndVerifyVisibility | equip a hair; the engine selects it and renders it in the Hair category | Partial |
//
// Partial, both: Unity reads the slot view's `SlotWearableUrn` and whether its thumbnail sprite
// loaded. A tile's urn and its `<img alt="">` thumbnail are not in the HUD's accessibility tree, so
// the engine answers instead: `/avatar_state` lists what it renders by category (`rendered.wearables`),
// and "the thumbnail is visible" becomes "the build is processed, did not fail, and has the category".
// Unity needs one of four thumbnails; here all four categories must render. The slot tiles are
// still checked to be on screen. Both assume an account that wears base wearables in these
// categories, as Unity's CI account does.
//
// Partial, TestEquipBaseWearableAndVerifyVisibility: the card's equip is a draft until the Backpack
// closes (backpack.ts), so the engine is read after the close. The hair the avatar wore is put
// back afterwards, whatever the outcome.
import assert from 'node:assert/strict'
import { spec } from '../driver/index.ts'
import {
  EQUIP_TIMEOUT_MS,
  THUMBNAIL_TIMEOUT_MS,
  commitBackpack,
  equipCard,
  gained,
  openBackpack,
  pickHair,
  renderedUrn,
  restoreHair,
  sameUrn,
  waitForPlayer
} from './backpack.ts'
import { closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const BASE_WEARABLE_PREFIX = 'urn:decentraland:off-chain:base-avatars:'

/** Tile name and the engine's category slot for it. */
const SLOTS = [
  { tile: 'Hair', category: 'hair' },
  { tile: 'Eyes', category: 'eyes' },
  { tile: 'Eyebrows', category: 'eyebrows' },
  { tile: 'Mouth', category: 'mouth' }
] as const

scenario(
  'the base avatar slots are on screen and the engine renders a base wearable in each',
  async ({ explorer }) => {
    await openWithKey(explorer, 'backpack')
    await explorer.hud.click({ name: 'Wearables', role: 'tab' })
    await explorer.hud.click({ name: 'Categories', role: 'tab' })
    for (const { tile } of SLOTS) {
      assert.ok(await explorer.hud.isVisible({ name: tile, role: 'button', index: 0 }), `the ${tile} slot should be on screen`)
    }
    const avatar = await waitForPlayer(
      explorer,
      (candidate) => candidate.rendered?.processed === true && !candidate.rendered.failed && SLOTS.every(({ category }) => renderedUrn(candidate, category) !== undefined),
      THUMBNAIL_TIMEOUT_MS,
      'the engine to render the Hair, Eyes, Eyebrows and Mouth categories'
    )
    for (const { tile, category } of SLOTS) {
      const urn = renderedUrn(avatar, category)
      assert.ok(urn?.startsWith(BASE_WEARABLE_PREFIX), `${tile} should render a base avatar wearable, not '${urn}'`)
    }
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 120_000 }
)

scenario(
  'equipping a base hair is selected by the engine and rendered in the Hair category',
  async ({ explorer, restore }) => {
    await openBackpack(explorer, 'wearables')
    const pick = await pickHair(explorer)
    restore(() => restoreHair(explorer, pick))
    const { start } = pick
    const name = pick.card.name
    await equipCard(explorer, pick.card)
    await commitBackpack(explorer, start, `equipping "${name}"`)
    const selected = await waitForPlayer(
      explorer,
      (avatar) => gained(start.before.selected.wearables, avatar.selected.wearables).length > 0,
      EQUIP_TIMEOUT_MS,
      `the avatar to select a wearable it did not before ("${name}")`
    )
    const added = gained(start.before.selected.wearables, selected.selected.wearables)
    assert.ok(added.length > 0, `Grid item "${name}" should be equipped after pressing its EQUIP button`)
    const rendered = await waitForPlayer(
      explorer,
      (avatar) => {
        const hair = renderedUrn(avatar, 'hair')
        return avatar.rendered?.processed === true && !avatar.rendered.failed && hair !== undefined && added.some((urn) => sameUrn(urn, hair))
      },
      THUMBNAIL_TIMEOUT_MS,
      `the engine to render "${name}" in the Hair category`
    )
    assert.ok(renderedUrn(rendered, 'hair'), 'The Hair slot should show the equipped hair after equipping a base wearable')
  },
  { timeoutMs: 300_000 }
)
