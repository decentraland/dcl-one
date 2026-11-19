import { expectConsoleOutput } from '@explorer-core/test/console'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { BackpackPage } from '../../features/backpack/BackpackPage'
import { OUTFIT_FAILED_MESSAGE } from '../../features/session/useEngineSession'
import { enterAsGuest, fakeProfileState, fakeSession, renderSession, type Harness } from '../harness'

describe('a refused outfit save or delete', () => {
  describe('when the bridge reports a refused save', () => {
    let h: Harness

    beforeEach(async () => {
      expectConsoleOutput('error', [['[outfits] save of slot 1 failed:', 'Error: failed to deploy']])
      h = renderSession()
      await enterAsGuest(h)
      act(() => h.driver.emit({ kind: 'outfitSaveFailed', slot: 1, action: 'save', message: 'Error: failed to deploy' }))
    })

    it('should say the outfit was not saved', () => {
      expect(h.session().backpack.outfitError).toBe(OUTFIT_FAILED_MESSAGE.save)
    })

    describe('and the outfits arrive again', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'outfits', metadata: { outfits: [], namesForExtraSlots: [] } }))
      })

      it('should clear the error', () => {
        expect(h.session().backpack.outfitError).toBeNull()
      })
    })
  })

  describe('when the bridge reports a refused delete', () => {
    let h: Harness

    beforeEach(async () => {
      expectConsoleOutput('error', [['[outfits] delete of slot 0 failed:', 'Error: failed to deploy']])
      h = renderSession()
      await enterAsGuest(h)
      act(() => h.driver.emit({ kind: 'outfitSaveFailed', slot: 0, action: 'delete', message: 'Error: failed to deploy' }))
    })

    it('should say the outfit was not deleted', () => {
      expect(h.session().backpack.outfitError).toBe(OUTFIT_FAILED_MESSAGE.delete)
    })
  })

  describe('when outfits sit in extra slots and the player owns no name', () => {
    const grey = { color: { r: 0.5, g: 0.5, b: 0.5, a: 1 } }
    const outfit = { bodyShape: '', eyes: grey, hair: grey, skin: grey, wearables: [], forceRender: [] }
    let h: Harness

    beforeEach(async () => {
      h = renderSession()
      await enterAsGuest(h)
      act(() => h.driver.emit({ kind: 'outfits', metadata: { outfits: [{ slot: 6, outfit }, { slot: 8, outfit }], namesForExtraSlots: [] } }))
      act(() => h.driver.emit({ kind: 'ownedNames', names: [] }))
    })

    it('should warn the player before deleting second-row outfits', async () => {
      const s = h.session()
      render(<BackpackPage backpack={{ ...s.backpack, open: true }} emotes={s.emotes} profile={fakeProfileState()} onNavigate={vi.fn()} setEngineViewport={vi.fn()} />)
      await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))

      expect(screen.getByText('Without a NAME, deleting any outfit in the second row clears the entire row.')).toBeVisible()
    })

    describe('and a save to a free slot is refused', () => {
      beforeEach(() => {
        expectConsoleOutput('error', [['[outfits] save of slot 1 failed:', 'Error: outfit slot 6 needs an owned name']])
        act(() => h.driver.emit({ kind: 'outfitSaveFailed', slot: 1, action: 'save', message: 'Error: outfit slot 6 needs an owned name' }))
      })

      it('should say to delete the outfits in the extra slots first', () => {
        expect(h.session().backpack.outfitError).toBe(OUTFIT_FAILED_MESSAGE.extraSlotWithoutName)
      })
    })

    describe('and deleting that extra outfit is refused', () => {
      beforeEach(() => {
        expectConsoleOutput('error', [['[outfits] delete of slot 6 failed:', 'Error: failed to deploy']])
        act(() => h.driver.emit({ kind: 'outfitSaveFailed', slot: 6, action: 'delete', message: 'Error: failed to deploy' }))
      })

      it('should explain how to retry clearing the second row', () => {
        expect(h.session().backpack.outfitError).toBe(OUTFIT_FAILED_MESSAGE.deleteExtraWithoutName)
      })
    })
  })

  describe('when a save is refused while the Outfits tab is shown', () => {
    // The session's outfit error, raised from the test as the bridge's message would.
    let raise: (message: string) => void

    function Page() {
      const s = fakeSession()
      const [outfitError, setOutfitError] = useState<string | null>(null)
      raise = setOutfitError
      const backpack = { ...s.backpack, open: true, outfitError, dismissOutfitError: () => setOutfitError(null) }
      return <BackpackPage backpack={backpack} emotes={s.emotes} profile={fakeProfileState()} onNavigate={vi.fn()} setEngineViewport={vi.fn()} />
    }

    beforeEach(async () => {
      render(<Page />)
      await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
      act(() => raise(OUTFIT_FAILED_MESSAGE.save))
    })

    it('should alert the player', () => {
      expect(screen.getByRole('alert')).toHaveTextContent(OUTFIT_FAILED_MESSAGE.save)
    })

    describe('and the player leaves the tab', () => {
      beforeEach(async () => {
        await userEvent.click(screen.getByRole('tab', { name: /categories/i }))
        await userEvent.click(screen.getByRole('tab', { name: /saved outfits/i }))
      })

      it('should clear the error', () => {
        expect(screen.queryByRole('alert')).toBeNull()
      })
    })
  })
})
