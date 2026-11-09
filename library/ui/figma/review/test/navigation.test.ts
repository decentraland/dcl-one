import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { variantPlatform, matchesPlatform } from '../src/review/navigation.ts'

describe('when filtering captured platform properties', () => {
  it('should classify explicit Web values case-insensitively', () => {
    assert.equal(variantPlatform({ properties: { Product: 'Web' } }), 'web')
  })
  it('should preserve Explorer without assuming its engine', () => {
    assert.equal(matchesPlatform({ properties: { product: 'explorer' } }, 'bevy'), false)
    assert.equal(matchesPlatform({ properties: { product: 'explorer' } }, 'unity'), false)
  })
  it('should include unclassified designs in All platforms', () => {
    assert.equal(matchesPlatform({ properties: {} }, 'all'), true)
    assert.equal(variantPlatform({ properties: { Device: 'Desktop' } }), 'unclassified')
  })
  it('should not invent assignments from unknown values', () => {
    assert.equal(variantPlatform({ properties: { platform: 'other' } }), 'unclassified')
  })
  it('should filter explicitly captured engine values', () => {
    assert.equal(matchesPlatform({ properties: { platform: 'Bevy' } }, 'bevy'), true)
    assert.equal(matchesPlatform({ properties: { platform: 'Unity' } }, 'unity'), true)
  })
})
