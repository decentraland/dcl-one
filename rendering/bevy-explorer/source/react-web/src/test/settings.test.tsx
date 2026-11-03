import { describe, it, expect } from 'vitest'
import { act } from '@testing-library/react'
import type { Setting } from '../engine/protocol'
import { renderSession, enterAsGuest } from './harness'
import { presetTargets } from '../../bridge-scene/src/domains/settings'

// DOMAIN: settings — fetch ExplorerSettings on open, change a setting.
describe('settings domain', () => {
  const setting: Setting = {
    name: 'master_volume',
    category: 'audio',
    description: 'Master volume',
    minValue: 0,
    maxValue: 100,
    namedVariants: [],
    value: 50,
    default: 80,
    stepSize: 1
  }

  it('fetches settings every time the panel opens: the engine changes some itself', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().settings.toggle())
    expect(h.driver.sentOf('getSettings')).toHaveLength(1)
    act(() => h.session().settings.toggle()) // close
    act(() => h.session().settings.toggle()) // reopen
    expect(h.driver.sentOf('getSettings')).toHaveLength(2)
  })

  it('settings stream populates the list', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'settings', settings: [setting] })
    expect(h.session().settings.list[0].name).toBe('master_volume')
  })

  it('set posts setSetting with name + value', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().settings.set('master_volume', 75))
    expect(h.driver.last('setSetting')).toEqual({ kind: 'setSetting', name: 'master_volume', value: 75 })
  })
})

describe('graphics presets', () => {
  const variants = (...names: string[]): Setting['namedVariants'] => names.map((name) => ({ name, description: '' }))
  const base = { category: 'Graphics', description: '', minValue: 0, maxValue: 10, value: 0, default: 0, stepSize: 1 }
  const web: Setting[] = [
    { ...base, name: 'Anti-aliasing', namedVariants: variants('Off', 'FXAA (Low)', 'FXAA (High)') },
    { ...base, name: 'Shadow Distance', namedVariants: [] },
    { ...base, name: 'Lens Flare', namedVariants: variants('Off', 'On') }
  ]
  const native: Setting[] = [
    {
      ...base,
      name: 'Anti-aliasing',
      namedVariants: variants('Off', 'FXAA (Low)', 'FXAA (High)', 'MSAA 2x', 'MSAA 4x', 'MSAA 8x')
    }
  ]
  const target = (list: Setting[], name: string): number | undefined =>
    presetTargets(list, 1).find((t) => t.name === name)?.value

  it('Medium falls back to FXAA (High) where the engine publishes no MSAA', () => {
    expect(target(web, 'Anti-aliasing')).toBe(2)
    expect(target(web, 'Shadow Distance')).toBe(60)
    expect(target(web, 'Lens Flare')).toBe(1)
  })

  it('Medium picks MSAA 2x when the engine publishes it', () => {
    expect(target(native, 'Anti-aliasing')).toBe(3)
  })
})
