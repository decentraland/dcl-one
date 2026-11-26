import { describe, it, expect } from 'vitest'
import type { Setting } from '@explorer-core/engine/protocol'
import { currentPreset, presetTargets } from '../../bridge-scene/src/domains/graphicsPresets'

const variants = (...names: string[]): Setting['namedVariants'] => names.map((name) => ({ name, description: '' }))
const setting = (name: string, value: number, names: string[] = []): Setting => ({
  name,
  category: 'Graphics',
  description: '',
  minValue: 0,
  maxValue: 300,
  namedVariants: variants(...names),
  value,
  default: value,
  stepSize: 1,
  disabled: false
})
const OFF_ON = ['Off', 'On']
const FXAA = ['Off', 'FXAA (Low)', 'FXAA (High)']
const MSAA = [...FXAA, 'MSAA 2x', 'MSAA 4x', 'MSAA 8x']

// The engine's defaults, the Medium preset, as the web build publishes them: no VSync, no MSAA,
// no SSAO. Jiggle Bones is off by default but on in Medium.
const webDefaults = (): Setting[] => [
  setting('Target Frame Rate', 1, ['30 fps', '60 fps', '120 fps', '144 fps', 'Max']),
  setting('Render Resolution', 100),
  setting('Anti-aliasing', 2, FXAA),
  setting('Bloom', 1, OFF_ON),
  setting('Avatar Outline', 1, OFF_ON),
  setting('Jiggle Bones', 0, OFF_ON),
  setting('Shadow settings', 1, ['Off', 'Low', 'High']),
  setting('Shadow Distance', 20),
  setting('Light Count', 6),
  setting('Shadow Caster Count', 3),
  setting('Lens Flare', 1, OFF_ON),
  setting('Depth of Field', 2, ['Off', 'Low', 'High']),
  setting('Fog', 1, OFF_ON),
  setting('Sky Reflections', 1, ['Low', 'High']),
  setting('Scene Load Distance', 10)
]
const nativeDefaults = (): Setting[] => [
  ...webDefaults().filter((s) => s.name !== 'Anti-aliasing'),
  setting('Anti-aliasing', 2, MSAA),
  setting('VSync', 1, OFF_ON),
  setting('SSAO', 0, ['Off', 'Low', 'High'])
]
const withValue = (list: Setting[], name: string, value: number): Setting[] =>
  list.map((s) => (s.name === name ? { ...s, value } : s))
const target = (list: Setting[], preset: number, name: string): number | undefined =>
  presetTargets(list, preset).find((t) => t.name === name)?.value

describe('graphics presets', () => {
  it('Medium is the engine defaults, with Jiggle Bones on, on the web and natively', () => {
    expect(currentPreset(webDefaults())).toBe(3)
    expect(currentPreset(withValue(webDefaults(), 'Jiggle Bones', 1))).toBe(1)
    expect(currentPreset(withValue(nativeDefaults(), 'Jiggle Bones', 1))).toBe(1)
  })

  it('Medium reads each setting off its published default', () => {
    const list = webDefaults().map((s) => (s.name === 'Shadow Distance' ? { ...s, default: 35 } : s))
    expect(target(list, 1, 'Shadow Distance')).toBe(35)
  })

  it('every preset keeps depth of field and uses no MSAA', () => {
    for (const preset of [0, 1, 2]) {
      expect(target(nativeDefaults(), preset, 'Depth of Field')).toBe(2)
      expect(target(nativeDefaults(), preset, 'Anti-aliasing')).toBeLessThan(3)
    }
    expect(target(nativeDefaults(), 2, 'Anti-aliasing')).toBe(2)
  })

  it('every preset uses the default bloom', () => {
    for (const preset of [0, 1, 2]) {
      expect(target(nativeDefaults(), preset, 'Bloom')).toBe(1)
    }
  })

  it('Low turns off the costly features', () => {
    const low = Object.fromEntries(presetTargets(nativeDefaults(), 0).map((t) => [t.name, t.value]))
    expect(low).toMatchObject({
      'Target Frame Rate': 0,
      VSync: 0,
      'Render Resolution': 50,
      'Anti-aliasing': 0,
      'Shadow settings': 0,
      'Light Count': 0,
      'Lens Flare': 0
    })
  })

  it('leaves load distance to the player, and skips what the engine does not publish', () => {
    const names = presetTargets(nativeDefaults(), 2).map((t) => t.name)
    expect(names).not.toContain('Scene Load Distance')
  })

  it('reads as Custom once a preset setting differs', () => {
    const medium = withValue(webDefaults(), 'Jiggle Bones', 1)
    expect(currentPreset(withValue(medium, 'Shadow Distance', 80))).toBe(3)
  })
})
