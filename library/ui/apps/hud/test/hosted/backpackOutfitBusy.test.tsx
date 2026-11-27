import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BackpackPage } from '../../features/backpack/BackpackPage'
import { PopupHost, resetPopups } from '../../design'
import type { BackpackState } from '../../features/session/useEngineSession'
import { enterAsGuest, fakeProfileState, fakeSession, renderSession, type Harness } from '../harness'

const grey = { color: { r: 0.5, g: 0.5, b: 0.5, a: 1 } }
const outfit = { bodyShape: '', eyes: grey, hair: grey, skin: grey, wearables: [], forceRender: [] }
const QUESTION = 'Are you sure you want to delete this Outfit?'

describe('the session while the scene works on a saved outfit', () => {
  let h: Harness

  beforeEach(async () => {
    h = renderSession()
    await enterAsGuest(h)
  })

  describe('when no outfit is being worked on', () => {
    it('should report no operation and no equip', () => {
      expect([h.session().backpack.outfitOperation, h.session().backpack.outfitEquipping]).toEqual([null, null])
    })
  })

  describe('when the scene starts saving a slot', () => {
    beforeEach(() => {
      act(() => h.driver.emit({ kind: 'outfitBusy', slot: 2, action: 'save', busy: true }))
    })

    it('should report the save', () => {
      expect(h.session().backpack.outfitOperation).toEqual({ slot: 2, action: 'save' })
    })

    describe('and the save ends', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'outfitBusy', slot: 2, action: 'save', busy: false }))
      })

      it('should report no operation', () => {
        expect(h.session().backpack.outfitOperation).toBeNull()
      })
    })

    describe('and an equip of another slot starts and ends meanwhile', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'outfitBusy', slot: 0, action: 'equip', busy: true }))
        act(() => h.driver.emit({ kind: 'outfitBusy', slot: 0, action: 'equip', busy: false }))
      })

      it('should still report the save', () => {
        expect(h.session().backpack.outfitOperation).toEqual({ slot: 2, action: 'save' })
      })

      it('should report no equip', () => {
        expect(h.session().backpack.outfitEquipping).toBeNull()
      })
    })

    describe('and the bridge starts again', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'bridgeReady' }))
      })

      it('should report no operation', () => {
        expect(h.session().backpack.outfitOperation).toBeNull()
      })
    })
  })

  describe('when the player asks to save into a slot', () => {
    beforeEach(() => {
      act(() => h.session().backpack.saveOutfit(3))
    })

    it('should report the save before the scene answers', () => {
      expect(h.session().backpack.outfitOperation).toEqual({ slot: 3, action: 'save' })
    })

    it('should ask the scene to save that slot', () => {
      expect(h.driver.sent).toContainEqual({ kind: 'saveOutfit', slot: 3 })
    })

    describe('and the scene ends the save', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'outfitBusy', slot: 3, action: 'save', busy: false }))
      })

      it('should report no operation', () => {
        expect(h.session().backpack.outfitOperation).toBeNull()
      })
    })
  })

  describe('when the player asks to delete a saved outfit', () => {
    beforeEach(() => {
      act(() => h.session().backpack.deleteOutfit(0))
    })

    it('should report the delete before the scene answers', () => {
      expect(h.session().backpack.outfitOperation).toEqual({ slot: 0, action: 'delete' })
    })
  })

  describe('when the player asks to equip a saved outfit', () => {
    beforeEach(() => {
      act(() => h.session().backpack.equipOutfit(1))
    })

    it('should report the slot being equipped before the scene answers', () => {
      expect(h.session().backpack.outfitEquipping).toBe(1)
    })
  })

  describe('when the player asks to see a saved outfit on the avatar', () => {
    beforeEach(() => {
      act(() => h.session().backpack.previewOutfit(1))
    })

    it('should report the slot being loaded before the scene answers', () => {
      expect(h.session().backpack.outfitEquipping).toBe(1)
    })

    it('should ask the scene to show that slot without equipping it', () => {
      expect([h.driver.sentOf('previewOutfit'), h.driver.sentOf('equipOutfit')]).toEqual([[{ kind: 'previewOutfit', slot: 1 }], []])
    })

    describe('and the scene ends the preview', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'outfitBusy', slot: 1, action: 'preview', busy: false }))
      })

      it('should report no slot being loaded', () => {
        expect(h.session().backpack.outfitEquipping).toBeNull()
      })
    })

    describe('and the bridge starts again', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'bridgeReady' }))
      })

      it('should report no slot being loaded', () => {
        expect(h.session().backpack.outfitEquipping).toBeNull()
      })
    })
  })

  describe('when the scene starts showing an outfit on the avatar', () => {
    beforeEach(() => {
      act(() => h.driver.emit({ kind: 'outfitBusy', slot: 2, action: 'preview', busy: true }))
    })

    it('should report the slot being loaded', () => {
      expect(h.session().backpack.outfitEquipping).toBe(2)
    })
  })

  describe('when the Backpack closes', () => {
    beforeEach(() => {
      act(() => h.session().backpack.toggle())
      act(() => h.session().backpack.previewOutfit(0))
      act(() => h.session().backpack.toggle())
    })

    it('should put back the look from before the preview, then deploy', () => {
      const kinds = h.driver.sent.map((m) => m.kind).filter((k) => k === 'restoreOutfitPreview' || k === 'commitAvatar')
      expect(kinds).toEqual(['restoreOutfitPreview', 'commitAvatar'])
    })
  })

  describe('when the scene starts putting an outfit on', () => {
    beforeEach(() => {
      act(() => h.driver.emit({ kind: 'outfitBusy', slot: 1, action: 'equip', busy: true }))
    })

    it('should report the slot being equipped', () => {
      expect(h.session().backpack.outfitEquipping).toBe(1)
    })

    describe('and the equip of an earlier slot ends', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'outfitBusy', slot: 0, action: 'equip', busy: false }))
      })

      it('should still report the slot being equipped', () => {
        expect(h.session().backpack.outfitEquipping).toBe(1)
      })
    })
  })
})

describe('the Saved Outfits slots', () => {
  let deleteOutfit: ReturnType<typeof vi.fn<(slot: number) => void>>
  let saveOutfit: ReturnType<typeof vi.fn<(slot: number) => void>>
  let equipOutfit: ReturnType<typeof vi.fn<(slot: number) => void>>

  // Slots 1 and 2 hold an outfit; 3 to 5 are empty.
  async function showOutfits(over: Partial<BackpackState> = {}): Promise<void> {
    const s = fakeSession()
    const backpack: BackpackState = {
      ...s.backpack,
      open: true,
      outfits: [{ slot: 0, outfit }, { slot: 1, outfit }],
      deleteOutfit,
      saveOutfit,
      equipOutfit,
      ...over
    }
    render(
      <>
        <BackpackPage backpack={backpack} emotes={s.emotes} profile={fakeProfileState()} onNavigate={vi.fn()} setEngineViewport={vi.fn()} />
        <PopupHost />
      </>
    )
    await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
  }

  beforeEach(() => {
    deleteOutfit = vi.fn<(slot: number) => void>()
    saveOutfit = vi.fn<(slot: number) => void>()
    equipOutfit = vi.fn<(slot: number) => void>()
  })

  afterEach(() => {
    act(() => resetPopups())
  })

  describe('when the saved outfits have not arrived', () => {
    beforeEach(async () => {
      await showOutfits({ outfitsLoaded: false })
    })

    it('should offer no slot to save into', () => {
      expect(screen.queryByRole('button', { name: /^Save outfit in slot/ })).toBeNull()
    })

    it('should show no saved outfit', () => {
      expect(screen.queryByRole('button', { name: /^Outfit \d+$/ })).toBeNull()
    })

    it("should fill each slot with the HUD's sweeping placeholder", () => {
      const placeholders = Array.from(document.querySelectorAll('[class*="outfitSkeleton"]'))
      expect(placeholders.length).toBeGreaterThan(0)
      for (const placeholder of placeholders) expect(placeholder.className).toMatch(/sweep/)
    })
  })

  describe('when the player presses the delete button of a saved outfit', () => {
    beforeEach(async () => {
      await showOutfits()
      await userEvent.click(screen.getByRole('button', { name: 'Delete Outfit 1' }))
    })

    it('should ask whether to delete the outfit', () => {
      expect(screen.getByText(QUESTION)).toBeVisible()
    })

    it('should not delete the outfit yet', () => {
      expect(deleteOutfit).not.toHaveBeenCalled()
    })

    it('should keep showing the outfit while the question is open', () => {
      expect(screen.getByRole('button', { name: 'Outfit 1', hidden: true })).toBeInTheDocument()
    })

    it('should not show the slot as being deleted', () => {
      expect(screen.queryByRole('group', { name: 'Deleting outfit 1', hidden: true })).toBeNull()
    })

    describe('and the player cancels', () => {
      beforeEach(async () => {
        await userEvent.click(screen.getByRole('button', { name: 'CANCEL' }))
      })

      it('should not delete the outfit', () => {
        expect(deleteOutfit).not.toHaveBeenCalled()
      })

      it('should show the outfit again with its delete button', () => {
        expect(screen.getByRole('button', { name: 'Delete Outfit 1' })).toBeVisible()
      })

      it('should close the question', () => {
        expect(screen.queryByText(QUESTION)).toBeNull()
      })
    })

    describe('and the player confirms', () => {
      beforeEach(async () => {
        await userEvent.click(screen.getByRole('button', { name: 'YES' }))
      })

      it('should delete that outfit', () => {
        expect(deleteOutfit).toHaveBeenCalledExactlyOnceWith(0)
      })

      it('should close the question', () => {
        expect(screen.queryByText(QUESTION)).toBeNull()
      })
    })
  })

  describe('when the scene is saving an outfit into a slot', () => {
    beforeEach(async () => {
      await showOutfits({ outfitOperation: { slot: 2, action: 'save' } })
    })

    it('should show that slot as being saved', () => {
      expect(within(screen.getByRole('group', { name: 'Saving outfit 3' })).getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    })

    it('should not offer that slot to save into', () => {
      expect(screen.queryByRole('button', { name: 'Save outfit in slot 3' })).toBeNull()
    })

    it('should not let the player save into another empty slot', () => {
      expect(screen.getByRole('button', { name: 'Save outfit in slot 4' })).toBeDisabled()
    })

    it('should not let the player delete a saved outfit', () => {
      expect(screen.getByRole('button', { name: 'Delete Outfit 1' })).toBeDisabled()
    })

    it('should not let the player equip a saved outfit', () => {
      expect(within(screen.getByRole('button', { name: 'Outfit 1' }).parentElement!).getByRole('button', { name: 'EQUIP' })).toBeDisabled()
    })
  })

  describe('when the scene is deleting an outfit', () => {
    beforeEach(async () => {
      await showOutfits({ outfitOperation: { slot: 1, action: 'delete' } })
    })

    it('should show that slot as being deleted', () => {
      expect(screen.getByRole('group', { name: 'Deleting outfit 2' })).toHaveAttribute('aria-busy', 'true')
    })

    it('should not let the player equip another outfit', () => {
      expect(within(screen.getByRole('button', { name: 'Outfit 1' }).parentElement!).getByRole('button', { name: 'EQUIP' })).toBeDisabled()
    })
  })

  describe('when the avatar is loading an outfit, shown or equipped', () => {
    const equipButtonOf = (slot: number): HTMLElement =>
      within(screen.getByRole('button', { name: `Outfit ${slot}`, hidden: true }).parentElement!).getByRole('button', { name: 'EQUIP', hidden: true })

    beforeEach(async () => {
      await showOutfits({ outfitEquipping: 0 })
    })

    it('should show a spinner in that outfit\'s EQUIP button', () => {
      expect(within(equipButtonOf(1)).getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    })

    it('should mark that EQUIP button busy and not let it be pressed again', () => {
      const button = equipButtonOf(1)
      expect([button.getAttribute('aria-busy'), button.hasAttribute('disabled')]).toEqual(['true', true])
    })

    it('should not let the player equip another outfit', () => {
      expect(equipButtonOf(2)).toBeDisabled()
    })

    it('should not let the player show that outfit again', () => {
      expect(screen.getByRole('button', { name: 'Outfit 1', hidden: true })).toBeDisabled()
    })

    it('should not let the player delete that outfit', () => {
      expect(screen.getByRole('button', { name: 'Delete Outfit 1', hidden: true })).toBeDisabled()
    })

    it('should not let the player show another outfit', () => {
      expect(screen.getByRole('button', { name: 'Outfit 2', hidden: true })).toBeDisabled()
    })

    it('should not let the player delete another outfit', () => {
      expect(screen.getByRole('button', { name: 'Delete Outfit 2' })).toBeDisabled()
    })

    it('should not let the player save into an empty slot', () => {
      expect(screen.getByRole('button', { name: 'Save outfit in slot 3' })).toBeDisabled()
    })
  })
})
