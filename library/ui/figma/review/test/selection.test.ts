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
  it('should preserve the other property when the combination exists', () => {
    assert.equal(selectVariant(variants, 0, 'State', 'Hover'), 1)
  })
  it('should select a real combination when the previous state is unavailable', () => {
    assert.equal(selectVariant(variants, 1, 'Variant', 'Secondary'), 2)
  })
  it('should retain the current selection for unknown values', () => {
    assert.equal(selectVariant(variants, 1, 'State', 'Missing'), 1)
  })
  it('should handle components without variant properties', () => {
    assert.deepEqual(propertyOptions([{ properties: {} }]), [])
  })
})
