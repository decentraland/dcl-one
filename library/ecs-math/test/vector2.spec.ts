import { Vector2 } from '../src/Vector2'

const results = {
  zeros: '(0.0, 0.0)',
  ones: '(1.0, 1.0)'
}

const normalize = (v: string) => (v === '-0.0' ? '0.0' : v)

function vector2ToString(vec: Vector2.MutableVector2) {
  const x = normalize(vec.x.toFixed(1).slice(0, 6))
  const y = normalize(vec.y.toFixed(1).slice(0, 6))

  return `(${x}, ${y})`
}

describe('ECS Vector2 - Next tests', () => {
  it('Vector2.create One', () => {
    expect(vector2ToString(Vector2.One())).toEqual(results.ones)
  })

  it('Vector3.create zeros', () => {
    expect(vector2ToString(Vector2.Zero())).toEqual(results.zeros)
  })
})
