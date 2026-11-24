import { describe, it, beforeEach } from 'node:test'
import assert from 'node:assert/strict'
import { propertyOptions, selectVariant } from '../src/review/selection.ts'

describe('when selecting properties from captured combinations', () => {
  let variants: { properties: Record<string, string> }[]
  beforeEach(() => {
    variants = [
      { properties: { Variant: 'Primary', State: 'Default' } },
      { properties: { Variant: 'Primary', State: 'Hover' } },
      { properties: { Variant: 'Secondary', State: 'Default' } }
    ]
  })
  it('should expose one selector per source property', () => {
    assert.deepEqual(propertyOptions(variants), [
      ['Variant', ['Primary', 'Secondary']],
      ['State', ['Default', 'Hover']]
    ])
  })
  describe('and the requested combination exists', () => {
    it('should preserve the other property', () => {
      assert.equal(selectVariant(variants, 0, 'State', 'Hover'), 1)
    })
  })
  describe('and the previous state is unavailable for the requested value', () => {
    it('should select a real combination', () => {
      assert.equal(selectVariant(variants, 1, 'Variant', 'Secondary'), 2)
    })
  })
  describe('and the requested value is unknown', () => {
    it('should retain the current selection', () => {
      assert.equal(selectVariant(variants, 1, 'State', 'Missing'), 1)
    })
  })
})

describe('when a component has no variant properties', () => {
  let variants: { properties: Record<string, string> }[]
  beforeEach(() => {
    variants = [{ properties: {} }]
  })
  it('should expose no selectors', () => {
    assert.deepEqual(propertyOptions(variants), [])
  })
})
