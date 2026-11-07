import { Color4 } from '../src/Color4'

describe('ECS Color 4', () => {
  it('creates color4 without alpha', () => {
    const hex3String = '#123456'
    const color4Hex = Color4.toHexString(Color4.fromHexString(hex3String))
    expect(color4Hex).toBe('#123456FF')
  })

  it('creates color4 with alpha', () => {
    const hex3String = '#123456cd'
    const color4Hex = Color4.toHexString(Color4.fromHexString(hex3String))
    expect(color4Hex).toBe('#123456CD')
  })

  it('creates color4 with invalid', () => {
    const hex3String = '#fc'
    const color4Hex = Color4.toHexString(Color4.fromHexString(hex3String))
    expect(color4Hex).toBe('#000000FF')
  })
})
