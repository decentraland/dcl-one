import { describe, it, expect, vi } from 'vitest'
import type { Setting } from '@explorer-core/engine/protocol'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
const api = vi.hoisted(() => ({
  getSettings: vi.fn(async () => await new Promise<never>(() => undefined)),
  setSetting: vi.fn(),
  getSettingsStream: vi.fn()
}))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { registerSettings } = await import('../../bridge-scene/src/domains/settings')

const microphone = (...names: string[]): Setting => ({
  name: 'Microphone',
  category: 'Audio',
  description: '',
  minValue: 0,
  maxValue: names.length,
  namedVariants: names.map((name) => ({ name, description: '' })),
  value: 0,
  default: 0,
  stepSize: 1,
  disabled: false
})

describe('settings stream (bridge)', () => {
  it("relays each list the engine streams to the page, with the graphics preset", () => {
    const sent: Array<{ kind: string; settings?: Setting[] }> = []
    relay.mockClear()
    registerSettings({ send: (msg) => sent.push(msg as never), on: () => undefined, push: () => undefined })
    expect(relay.mock.calls[0][0]).toBe('settings')
    const engine = relay.mock.calls[0][2] as (settings: Setting[]) => void

    engine([microphone('System Default')])
    engine([microphone('System Default', 'Headset')])

    expect(sent.map((m) => m.kind)).toEqual(['settings', 'settings'])
    const mic = sent[1].settings?.find((s) => s.name === 'Microphone')
    expect(mic?.namedVariants.map((v) => v.name)).toEqual(['System Default', 'Headset'])
    expect(sent[1].settings?.[0].name).toBe('Graphics Preset')
  })
})
