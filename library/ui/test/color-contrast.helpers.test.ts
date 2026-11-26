import { describe, it, expect } from 'vitest'
import { colorContrast, requiredTextContrast } from './color-contrast.helpers'
describe('when measuring opaque computed text colors', () => {
  it('should preserve exact black/white and equal-color limits', () => {
    expect(colorContrast('rgb(0, 0, 0)', 'rgb(255, 255, 255)')).toBe(21)
    expect(colorContrast('rgb(34, 34, 34)', 'rgb(34, 34, 34)')).toBe(1)
  })
  it('should measure the source primary-pink white pairing without rounding it upward', () => {
    expect(colorContrast('rgb(255, 255, 255)', 'rgb(255, 45, 85)')).toBeCloseTo(3.6467641009, 9)
  })
  it.each([
    'transparent',
    'red',
    'rgba(0,0,0,.5)',
    'rgb(256,0,0)',
    'rgb(-1,0,0)',
    'color(srgb 0 0 0)'
  ])('should reject unsupported or ambiguous paint %s', (value) => {
    expect(() => colorContrast(value, 'rgb(0, 0, 0)')).toThrow()
  })
  it('should distinguish CSS pixels from point-size bold thresholds', () => {
    expect(requiredTextContrast('16px', '700')).toBe(4.5)
    expect(requiredTextContrast('18px', '700')).toBe(4.5)
    expect(requiredTextContrast('18.666666666666668px', '700')).toBe(3)
    expect(requiredTextContrast('24px', '400')).toBe(3)
    expect(requiredTextContrast('23px', '400')).toBe(4.5)
  })
  it.each([
    ['', '700'],
    ['0px', '700'],
    ['16px', 'unknown']
  ])('should reject missing metric %s %s', (size, weight) => {
    expect(() => requiredTextContrast(size, weight)).toThrow()
  })
})
