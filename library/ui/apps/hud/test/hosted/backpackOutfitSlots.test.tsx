import { describe, it, expect, beforeEach } from 'vitest'
import { act } from '@testing-library/react'
import { enterAsGuest, renderSession, type Harness } from '../harness'

const grey = { color: { r: 0.5, g: 0.5, b: 0.5 } }
const outfit = { bodyShape: '', eyes: grey, hair: grey, skin: grey, wearables: [], forceRender: [] }

describe('the number of outfit slots', () => {
  let h: Harness

  beforeEach(async () => {
    h = renderSession()
    await enterAsGuest(h)
  })

  describe('when the player has no names and no extra outfit', () => {
    it('should offer the free slots', () => {
      expect(h.session().backpack.outfitSlots).toBe(5)
    })
  })

  describe('when an outfit is saved past the free slots but the names are unknown', () => {
    beforeEach(() => {
      act(() => h.driver.emit({ kind: 'outfits', metadata: { outfits: [{ slot: 6, outfit }], namesForExtraSlots: [] } }))
    })

    it('should still offer the extra slots', () => {
      expect(h.session().backpack.outfitSlots).toBe(10)
    })
  })

  describe('when the stored outfits list names', () => {
    beforeEach(() => {
      act(() => h.driver.emit({ kind: 'outfits', metadata: { outfits: [], namesForExtraSlots: ['mine'] } }))
    })

    it('should offer the extra slots', () => {
      expect(h.session().backpack.outfitSlots).toBe(10)
    })

    describe('and the names lookup says the player owns none', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'ownedNames', names: [] }))
      })

      it('should offer the free slots', () => {
        expect(h.session().backpack.outfitSlots).toBe(5)
      })
    })

    describe('and the names lookup fails', () => {
      beforeEach(() => {
        act(() => h.driver.emit({ kind: 'ownedNames', names: [], failed: true }))
      })

      it('should still offer the extra slots', () => {
        expect(h.session().backpack.outfitSlots).toBe(10)
      })
    })
  })
})
