import { describe, it, expect, afterEach } from 'vitest'
import { enterAsGuest, renderSession } from './harness'
import { mapPins } from '@explorer-core/features/map/mapPins'
import type { MapPinInfo } from '@explorer-core/engine/generated'

const pin: MapPinInfo = { id: 'bafypx:512', x: 3, y: 2, title: 'Memory Grid', icon: null }

afterEach(() => mapPins.clear())

// DOMAIN: map pins — the engine's list of portable experiences' map pins, as the maps read it.
describe('map pins domain', () => {
  it('keeps the list the engine sends, and forgets it when the session ends', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'mapPins', pins: [pin] })
    expect(mapPins.getSnapshot().map((p) => [p.id, p.key, p.title, p.icon])).toEqual([['bafypx:512', { x: 3, y: 2 }, 'Memory Grid', null]])

    h.driver.emit({ kind: 'mapPins', pins: [] })
    expect(mapPins.getSnapshot()).toEqual([])

    h.driver.emit({ kind: 'mapPins', pins: [pin] })
    h.unmount()
    expect(mapPins.getSnapshot()).toEqual([])
  })
})
