import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BackpackPage } from '../../features/backpack/BackpackPage'
import type { Emote, Outfit, OutfitSlot, Wearable } from '@explorer-core/engine/protocol'
import type { BackpackState } from '../../features/session/useEngineSession'
import { SessionProvider } from '../../features/session/SessionContext'
import { fakeProfileState, fakeSession } from '../harness'

const wearable = (over: Partial<Wearable> = {}): Wearable => ({
  urn: 'urn:hat',
  name: 'Cool Hat',
  rarity: 'rare',
  category: 'hat',
  equipped: false,
  ...over
})

const outfitSlot = (slot: number, wearables: string[]): OutfitSlot => {
  const grey = { r: 0.5, g: 0.5, b: 0.5, a: 1 }
  const outfit: Outfit = { bodyShape: '', eyes: { color: grey }, hair: { color: grey }, skin: { color: grey }, wearables, forceRender: [] }
  return { slot, outfit }
}

const WAVE: Emote = { slot: 1, urn: 'urn:wave', name: 'Wave' }

function renderBackpack(over: Partial<BackpackState> = {}, modal = false, emoteList: Emote[] = [WAVE]): BackpackState {
  const backpack: BackpackState = { ...fakeSession().backpack, open: true, list: [wearable()], equip: vi.fn(), preview: vi.fn(), ...over }
  const emotes = { ...fakeSession().emotes, list: emoteList }
  render(
    <SessionProvider value={fakeSession()}>
      <BackpackPage
        backpack={backpack}
        emotes={emotes}
        profile={fakeProfileState()}
        onNavigate={vi.fn()}
        setEngineViewport={vi.fn()}
        modal={modal}
      />
    </SessionProvider>
  )
  return backpack
}

describe('backpack page clicks', () => {
  it('clicking a wearable card selects it (shows detail; does not equip or preview)', async () => {
    const backpack = renderBackpack()
    expect(screen.getByText('No item selected')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Cool Hat' }))
    // The detail panel now shows the item (its name renders as visible text there); the avatar is
    // left untouched — nothing is previewed or equipped.
    expect(screen.getByText('Cool Hat')).toBeInTheDocument()
    expect(vi.mocked(backpack.preview)).not.toHaveBeenCalled()
    expect(vi.mocked(backpack.equip)).not.toHaveBeenCalled()
  })

  it("the detail panel shows the selected wearable's description", async () => {
    renderBackpack({ list: [wearable({ description: '  Keeps the sun off.  ' })] })
    await userEvent.click(screen.getByRole('button', { name: 'Cool Hat' }))
    expect(screen.getByText('Keeps the sun off.')).toBeInTheDocument()
    expect(screen.queryByText(/does not have a description set/)).toBeNull()
  })

  it.each([undefined, '', '   '])('the detail panel says a wearable has no description when it is %j', async (description) => {
    renderBackpack({ list: [wearable({ description })] })
    await userEvent.click(screen.getByRole('button', { name: 'Cool Hat' }))
    expect(screen.getByText('This wearable does not have a description set.')).toBeInTheDocument()
  })

  it('in the modal (the lobby) a click shows no detail panel, and a double-click still equips', async () => {
    const backpack = renderBackpack({}, true)
    expect(screen.queryByText('No item selected')).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Cool Hat' }))
    expect(screen.queryByRole('complementary')).toBeNull()
    expect(screen.queryByText('No item selected')).toBeNull()
    expect(screen.queryByText('Cool Hat')).toBeNull()
    expect(vi.mocked(backpack.preview)).not.toHaveBeenCalled()
    expect(vi.mocked(backpack.equip)).not.toHaveBeenCalled()
    await userEvent.dblClick(screen.getByRole('button', { name: 'Cool Hat' }))
    expect(vi.mocked(backpack.equip)).toHaveBeenCalledWith(['urn:hat'])
  })

  it('double-clicking a wearable card equips it', async () => {
    const backpack = renderBackpack()
    await userEvent.dblClick(screen.getByRole('button', { name: 'Cool Hat' }))
    expect(vi.mocked(backpack.equip)).toHaveBeenCalledWith(['urn:hat'])
  })

  it('EQUIP persists the set and drops the preview', async () => {
    const backpack = renderBackpack()
    await userEvent.click(screen.getByRole('button', { name: 'EQUIP' }))
    expect(vi.mocked(backpack.equip)).toHaveBeenCalledWith(['urn:hat'])
    expect(vi.mocked(backpack.preview)).toHaveBeenCalledWith(null)
  })

  it('clicking the selected category again clears the filter back to all', async () => {
    const query = vi.fn()
    renderBackpack({ query })
    const hatCategory = screen.getByRole('button', { name: 'Hat' })
    await userEvent.click(hatCategory)
    expect(query).toHaveBeenLastCalledWith(expect.objectContaining({ category: 'hat' }))
    await userEvent.click(hatCategory)
    expect(query).toHaveBeenLastCalledWith(expect.objectContaining({ category: undefined }))
  })

  it('an equipped removable category slot has a hover unequip button that unequips it', async () => {
    const backpack = renderBackpack({ list: [], equipped: [wearable({ category: 'hat', equipped: true })] })
    await userEvent.click(screen.getByRole('button', { name: 'Unequip Hat' }))
    expect(vi.mocked(backpack.equip)).toHaveBeenCalledWith([])
  })

  it('a required category (eyes) shows no unequip button even when equipped', () => {
    renderBackpack({ list: [], equipped: [wearable({ urn: 'urn:eyes', category: 'eyes', equipped: true })] })
    expect(screen.queryByRole('button', { name: 'Unequip Eyes' })).toBeNull()
  })

  it('outfits: the outfit matching the current look is marked equipped', async () => {
    renderBackpack({
      list: [],
      equipped: [wearable({ urn: 'urn:hat', category: 'hat' }), wearable({ urn: 'urn:shirt', category: 'upper_body' })],
      outfits: [outfitSlot(0, ['urn:hat', 'urn:shirt']), outfitSlot(1, ['urn:other'])]
    })
    await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
    // Only slot 0 (Outfit 1) equals the equipped set → exactly one equipped slot.
    expect(document.querySelectorAll('[class*="outfitEquipped"]')).toHaveLength(1)
  })

  it('outfits: a click shows the outfit on the avatar without equipping it', async () => {
    const backpack = renderBackpack({
      list: [],
      equipped: [],
      outfits: [outfitSlot(0, ['urn:hat', 'urn:shirt', 'urn:pants'])]
    })
    await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
    await userEvent.click(screen.getByRole('button', { name: 'Outfit 1' }))
    expect(vi.mocked(backpack.previewOutfit)).toHaveBeenCalledWith(0)
    expect(vi.mocked(backpack.equipOutfit)).not.toHaveBeenCalled()
  })

  it('outfits: double-clicking an outfit does not equip it', async () => {
    const backpack = renderBackpack({ list: [], equipped: [], outfits: [outfitSlot(0, ['urn:hat'])] })
    await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
    await userEvent.dblClick(screen.getByRole('button', { name: 'Outfit 1' }))
    expect(vi.mocked(backpack.previewOutfit)).toHaveBeenCalledWith(0)
    expect(vi.mocked(backpack.equipOutfit)).not.toHaveBeenCalled()
  })

  it('outfits: EQUIP equips the outfit', async () => {
    const backpack = renderBackpack({ list: [], equipped: [], outfits: [outfitSlot(1, ['urn:hat'])] })
    await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
    await userEvent.click(screen.getByRole('button', { name: 'EQUIP' }))
    expect(vi.mocked(backpack.equipOutfit)).toHaveBeenCalledWith(1)
    expect(vi.mocked(backpack.previewOutfit)).not.toHaveBeenCalled()
  })

  it('outfits: leaving the Saved Outfits puts back the look from before a preview', async () => {
    const backpack = renderBackpack({ list: [], equipped: [], outfits: [outfitSlot(0, ['urn:hat'])] })
    expect(vi.mocked(backpack.restoreOutfitPreview)).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
    expect(vi.mocked(backpack.restoreOutfitPreview)).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('tab', { name: 'Emotes' }))
    expect(vi.mocked(backpack.restoreOutfitPreview)).toHaveBeenCalledTimes(1)
  })

  it('switching to the Emotes tab shows the equipped emotes', async () => {
    renderBackpack()
    await userEvent.click(screen.getByRole('tab', { name: 'Emotes' }))
    expect(screen.getByText('Wave')).toBeInTheDocument()
  })

  it("the detail panel shows an emote's description, and says so when it has none", async () => {
    renderBackpack({}, false, [WAVE, { urn: 'urn:clap', name: 'Clap', description: 'A slow, sincere clap.' }])
    await userEvent.click(screen.getByRole('tab', { name: 'Emotes' }))
    await userEvent.click(screen.getByRole('button', { name: 'Clap' }))
    expect(screen.getByText('A slow, sincere clap.')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Wave' }))
    expect(screen.getByText('This emote does not have a description set.')).toBeInTheDocument()
    expect(screen.queryByText('A slow, sincere clap.')).toBeNull()
  })

  it('opens directly on the Emotes tab when initialTab="emotes" (the wheel\'s "Customise [E]")', () => {
    const backpack: BackpackState = { ...fakeSession().backpack, open: true, list: [wearable()], equip: vi.fn(), preview: vi.fn() }
    const emotes = { ...fakeSession().emotes, list: [{ slot: 1, urn: 'urn:wave', name: 'Wave' }] }
    render(
      <BackpackPage
        backpack={backpack}
        emotes={emotes}
        profile={fakeProfileState()}
        onNavigate={vi.fn()}
        setEngineViewport={vi.fn()}
        initialTab="emotes"
      />
    )
    // Emotes content is shown with no tab click; the wearable grid (Cool Hat) is not.
    expect(screen.getByText('Wave')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Cool Hat' })).not.toBeInTheDocument()
  })
})
