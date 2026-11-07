import { Epsilon, Quaternion, Vector3 } from '../src'
import { Matrix } from '../src/Matrix'

const results = {
  identity:
    '([1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0])'
}

const normalize = (v: string) => (v === '-0.0' ? '0.0' : v)
const v = (matrix: Matrix.ReadonlyMatrix, index: number) =>
  normalize(matrix._m[index].toFixed(1).substr(0, 6))

function matrixToString(m: Matrix.ReadonlyMatrix) {
  let res = ''
  res += `([${v(m, 0)}, ${v(m, 1)}, ${v(m, 2)}, ${v(m, 3)}], `
  res += `[${v(m, 4)}, ${v(m, 5)}, ${v(m, 6)}, ${v(m, 7)}], `
  res += `[${v(m, 8)}, ${v(m, 9)}, ${v(m, 10)}, ${v(m, 11)}], `
  res += `[${v(m, 12)}, ${v(m, 13)}, ${v(m, 14)}, ${v(m, 15)}])`
  return res
}

describe('ECS Matrix - Next tests', () => {
  it('Matrix.create One', () => {
    expect(matrixToString(Matrix.Identity())).toEqual(results.identity)
  })

  let tOut: Vector3
  let sOut: Vector3
  let rOut: Quaternion

  beforeEach(() => {
    tOut = Vector3.create(NaN, NaN, NaN)
    sOut = Vector3.create(NaN, NaN, NaN)
    rOut = Quaternion.create(NaN, NaN, NaN, NaN)
  })

  it('Matrix.decompose common case', () => {
    const t = Vector3.create(1, 2, 3)
    const r = Quaternion.fromEulerDegrees(15, 25, 35)
    const s = Vector3.create(2, 2, 2)
    const m = Matrix.compose(s, r, t)
    expect(Matrix.decompose(m, sOut, rOut, tOut)).toBe(true)
    expect(Vector3.equalsWithEpsilon(t, tOut)).toBe(true)
    expect(Vector3.equalsWithEpsilon(s, sOut)).toBe(true)
    expect(Math.abs(Quaternion.angle(r, rOut)) < Epsilon).toBe(true)
  })

  it('Matrix.decompose identity case', () => {
    expect(Matrix.decompose(Matrix.Identity(), sOut, rOut, tOut)).toBe(true)
    expect(Vector3.equalsWithEpsilon(tOut, Vector3.Zero())).toBe(true)
    expect(Vector3.equalsWithEpsilon(sOut, Vector3.One())).toBe(true)
    expect(
      Math.abs(Quaternion.angle(rOut, Quaternion.fromEulerDegrees(0, 0, 0))) <
        Epsilon
    ).toBe(true)
  })
})
