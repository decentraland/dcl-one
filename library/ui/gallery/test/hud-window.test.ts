// @vitest-environment node
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import {
  HUD_SCALE_SOURCE,
  expectedHudScale,
  expectedWindowHeight,
  readHudScale
} from '../scripts/hud-window.mjs'

describe('when the build check reads what a HUD window must show', () => {
  it('should take the reference canvas and bounds from hudScale.ts itself', () => {
    expect(readHudScale()).toEqual({ width: 1920, height: 1080, min: 0.25, max: 4 })
  })

  it('should name the constant it cannot read instead of checking against a guess', () => {
    const source = readFileSync(HUD_SCALE_SOURCE, 'utf8').replace(/^const MIN = .*$/m, '')
    expect(() => readHudScale(source)).toThrow(
      'cannot read MIN from library/explorer-core/lib/hudScale.ts'
    )
  })

  it.each([
    [1920, 1080, '1.0000'],
    [1280, 720, '0.6667'],
    [375, 480, '0.2500'],
    [16000, 9000, '4.0000']
  ])(
    'should expect the scale hudScale.ts writes for a %ix%i window: %s',
    (width, height, scale) => {
      expect(expectedHudScale(readHudScale(), width, height)).toBe(scale)
    }
  )

  it('should expect the height the window CSS gives, the minimum included', () => {
    expect(expectedWindowHeight(1280, '16 / 9', '480px')).toBe(720)
    expect(expectedWindowHeight(375, '16 / 9', '480px')).toBe(480)
    expect(() => expectedWindowHeight(1280, 'auto', '0px')).toThrow('has no aspect ratio')
  })
})
