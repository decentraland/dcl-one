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
  stepSize: 1
})
const OFF_ON = ['Off', 'On']
const FXAA = ['Off', 'FXAA (Low)', 'FXAA (High)']
const MSAA = [...FXAA, 'MSAA 2x', 'MSAA 4x', 'MSAA 8x']

// The engine's defaults, the Medium preset, as the web build publishes them: no VSync, no MSAA,
// no SSAO. Lens Flare is not published yet, so the presets skip it.
const webDefaults = (): Setting[] => [
  setting('Target Frame Rate', 4, ['10 fps', '15 fps', '20 fps', '30 fps', '60 fps', '120 fps', '144 fps', 'Uncapped']),
  setting('Render Resolution', 100),
  setting('Anti-aliasing', 2, FXAA),
  setting('Bloom', 2, ['Off', 'Low', 'High']),
  setting('Avatar Outline', 1, OFF_ON),
  setting('Shadow settings', 1, ['Off', 'Low', 'High']),
  setting('Shadow Distance', 60),
  setting('Light Count', 6),
  setting('Shadow Caster Count', 3),
  setting('Depth of Field', 0, ['Off', 'Low', 'High']),
  setting('Fog', 1, OFF_ON),
  setting('Sky Reflections', 1, ['Low', 'High']),
  setting('Scene Load Distance', 10)
]
const nativeDefaults = (): Setting[] => [
  ...webDefaults().filter((s) => s.name !== 'Anti-aliasing'),
  setting('Anti-aliasing', 3, MSAA),
  setting('VSync', 1, OFF_ON),
  setting('SSAO', 0, ['Off', 'Low', 'High'])
]
const target = (list: Setting[], preset: number, name: string): number | undefined =>
  presetTargets(list, preset).find((t) => t.name === name)?.value

describe('graphics presets', () => {
  it('the engine defaults are Medium, on the web and natively', () => {
    expect(currentPreset(webDefaults())).toBe(1)
    expect(currentPreset(nativeDefaults())).toBe(1)
  })

  it('Medium takes FXAA (High) where the engine publishes no MSAA, and MSAA 2x where it does', () => {
    expect(target(webDefaults(), 1, 'Anti-aliasing')).toBe(2)
    expect(target(nativeDefaults(), 1, 'Anti-aliasing')).toBe(3)
    expect(target(nativeDefaults(), 2, 'Anti-aliasing')).toBe(5)
  })

  it('Low turns off the costly features', () => {
    const low = Object.fromEntries(presetTargets(nativeDefaults(), 0).map((t) => [t.name, t.value]))
    expect(low).toMatchObject({
      'Target Frame Rate': 3,
      VSync: 0,
      'Render Resolution': 50,
      'Anti-aliasing': 0,
      Bloom: 0,
      'Shadow settings': 0,
      'Light Count': 0
    })
  })

  it('leaves load distance to the player, and skips what the engine does not publish', () => {
    const names = presetTargets(nativeDefaults(), 2).map((t) => t.name)
    expect(names).not.toContain('Scene Load Distance')
    expect(names).not.toContain('Lens Flare')
  })

  it('reads as Custom once a preset setting differs', () => {
    const list = webDefaults().map((s) => (s.name === 'Shadow Distance' ? { ...s, value: 80 } : s))
    expect(currentPreset(list)).toBe(3)
  })
})
