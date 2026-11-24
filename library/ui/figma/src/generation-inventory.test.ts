import { beforeEach, describe, expect, it } from 'vitest'
import {
  addInventoryPages,
  generationInventorySchema,
  type GenerationInventory
} from './generation-inventory.js'

describe('when updating generation inventory', () => {
  let inventory: GenerationInventory

  beforeEach(() => {
    inventory = {
      schema: 1,
      review: [{ pageId: 'frozen' }, { pageId: 'modals', componentIds: ['shop'] }],
      promotion: [{ pageId: 'frozen' }]
    }
  })

  describe('and a new ready page is captured with component identities', () => {
    it('should retain unrelated entries and keep exception-only pages out of promotion', () => {
      expect(addInventoryPages(inventory, ['selected'], { selected: ['component'] })).toEqual({
        ...inventory,
        review: [...inventory.review, { pageId: 'selected' }],
        promotion: [...inventory.promotion, { pageId: 'selected' }]
      })
    })

    it('should be idempotent for a repeated capture', () => {
      expect(
        addInventoryPages(
          addInventoryPages(inventory, ['selected'], { selected: ['component'] }),
          ['selected'],
          { selected: ['component'] }
        )
      ).toEqual(addInventoryPages(inventory, ['selected'], { selected: ['component'] }))
    })
  })

  describe('and a new ready page is captured without component identities', () => {
    it('should not admit the page', () => {
      expect(addInventoryPages(inventory, ['selected'])).toEqual(inventory)
    })
  })

  describe('and a former exception page is captured ready', () => {
    it('should admit the page to promotion', () => {
      expect(
        addInventoryPages(inventory, ['modals'], { modals: ['shop', 'other'] }).promotion
      ).toEqual([{ pageId: 'frozen' }, { pageId: 'modals' }])
    })

    it('should retain normal promotion scope on subsequent ready captures', () => {
      const first = addInventoryPages(inventory, ['modals'], { modals: ['shop', 'other'] })
      expect(
        addInventoryPages(first, ['modals'], { modals: ['shop', 'other', 'new'] }).promotion
      ).toEqual([{ pageId: 'frozen' }, { pageId: 'modals' }])
    })
  })

  describe('and capture IDs are unavailable for a former exception page', () => {
    it('should retain a narrow promotion scope', () => {
      expect(addInventoryPages(inventory, ['modals']).promotion).toEqual(inventory.promotion)
    })
  })

  describe('and promotion would exceed Review scope', () => {
    beforeEach(() => {
      inventory.promotion.push({ pageId: 'modals' })
    })

    it('should fail closed', () => {
      expect(() => generationInventorySchema.parse(inventory)).toThrow(
        'Promotion scope exceeds Review scope'
      )
    })
  })

  describe('and the same page appears twice', () => {
    beforeEach(() => {
      inventory.review.push({ pageId: 'frozen' })
    })

    it('should reject ambiguous inventory', () => {
      expect(() => generationInventorySchema.parse(inventory)).toThrow(
        'Duplicate review inventory page'
      )
    })
  })
})
