// The place Donate names for where the player stands (features/donate/place).
import { describe, expect, it } from 'vitest'
import { donatePlaceOf } from '../features/donate/place'

describe('the place Donate names', () => {
  it('is the parcel in Genesis City, and the World by its name elsewhere', () => {
    expect(donatePlaceOf({ genesis: true, parcel: { x: -9, y: 4 }, realm: 'main' })).toEqual({ parcel: { x: -9, y: 4 } })
    expect(donatePlaceOf({ realm: 'Lorux0r.dcl.eth', parcel: { x: 0, y: 0 } })).toEqual({ world: 'lorux0r.dcl.eth' })
    expect(donatePlaceOf({ realm: 'https://worlds.example.org/world/boedo.dcl.eth/', parcel: { x: 0, y: 0 } })).toEqual({ world: 'boedo.dcl.eth' })
  })

  it('is none in a preview, a local realm, or Genesis City between scenes', () => {
    expect(donatePlaceOf({ genesis: true, preview: true, parcel: { x: 0, y: 0 } })).toBeNull()
    expect(donatePlaceOf({ realm: 'http://127.0.0.1:8000', parcel: { x: 0, y: 0 } })).toBeNull()
    expect(donatePlaceOf({ genesis: true })).toBeNull()
  })
})
