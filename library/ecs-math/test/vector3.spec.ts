import { FloatArray } from '../src'
import { Matrix } from '../src/Matrix'
import { Quaternion } from '../src/Quaternion'
import { Vector3 } from '../src/Vector3'

const results = {
  zeros: '(0.0, 0.0, 0.0)',
  ones: '(1.0, 1.0, 1.0)',
  minusOnes: '(-1.0, -1.0, -1.0)',
  rotatedWithQuat01: '(0.7, 0.0, 1.6)',
  rotatedWithQuat02: '(-0.1, -0.9, 0.6)',
  rotatedWithQuat03: '(127.7, 46.0, -30.2)',
  rotatedWithQuat04: '(-115.4, -138.8, 327.7)'
}

const normalize = (v: string) => (v === '-0.0' ? '0.0' : v)

function vector3ToString(vec: Vector3.MutableVector3) {
  const x = normalize(vec.x.toFixed(1).substr(0, 6))
  const y = normalize(vec.y.toFixed(1).substr(0, 6))
  const z = normalize(vec.z.toFixed(1).substr(0, 6))

  return `(${x}, ${y}, ${z})`
}

describe('ECS Vector3 - Next tests', () => {
  it('Vector3.create One', () => {
    expect(vector3ToString(Vector3.One())).toEqual(results.ones)
  })

  it('Vector3.create zeros', () => {
    expect(vector3ToString(Vector3.Zero())).toEqual(results.zeros)
  })

  it('Vector3.add ones + zeros = ones', () => {
    expect(vector3ToString(Vector3.add(Vector3.Zero(), Vector3.One()))).toEqual(
      results.ones
    )
  })

  it('Vector3.sub zeros - ones = -ones', () => {
    expect(
      vector3ToString(Vector3.subtract(Vector3.Zero(), Vector3.One()))
    ).toEqual(results.minusOnes)
  })

  it('Vector3.opposite(ones) = -ones', () => {
    expect(vector3ToString(Vector3.negate(Vector3.One()))).toEqual(
      results.minusOnes
    )
  })

  it('Vector3.copy(ones) = ones', () => {
    const immutableVector = Vector3.One() as Vector3.ReadonlyVector3
    const mutableVector = Vector3.clone(immutableVector)

    mutableVector.x = 2.0

    expect(immutableVector).toStrictEqual(Vector3.One())
    expect(mutableVector.x).toEqual(2.0)
  })

  it('vector3.rotate', () => {
    expect(
      vector3ToString(
        Vector3.rotate(Vector3.One(), Quaternion.fromEulerDegrees(45, 60, 90))
      )
    ).toEqual(results.rotatedWithQuat01)

    expect(
      vector3ToString(
        Vector3.rotate(
          Vector3.create(1, 0, -0.5),
          Quaternion.fromEulerDegrees(-165, 55, 125)
        )
      )
    ).toEqual(results.rotatedWithQuat02)

    expect(
      vector3ToString(
        Vector3.rotate(
          Vector3.create(100, -90, 35),
          Quaternion.fromEulerDegrees(45, 60, 90)
        )
      )
    ).toEqual(results.rotatedWithQuat03)

    expect(
      vector3ToString(
        Vector3.rotate(
          Vector3.create(100, 200, 300),
          Quaternion.fromEulerDegrees(47.572, 13.179, 83.369)
        )
      )
    ).toEqual(results.rotatedWithQuat04)
  })

  it('Vector3 operations isNonUniform', () => {
    expect(Vector3.isNonUniform(Vector3.create(1, -2, -5))).toEqual(true)
    expect(Vector3.isNonUniform(Vector3.create(1, 1, -5))).toEqual(true)
    expect(Vector3.isNonUniform(Vector3.create(1, -1, -1))).toEqual(false)
    expect(Vector3.isNonUniform(Vector3.create(1, 1, 1))).toEqual(false)
  })

  it('Vector3 operations subtract', () => {
    const vector1 = Vector3.create(10, 10, -10) as Vector3.ReadonlyVector3
    const vector2 = Vector3.create(2, 2, 2) as Vector3.ReadonlyVector3
    const result = Vector3.create()

    Vector3.subtractToRef(vector1, vector2, result)
    expect(result.x).toEqual(8)
    expect(result.y).toEqual(8)
    expect(result.z).toEqual(-12)

    Vector3.subtractFromFloatsToRef(vector1, -10, -10, -10, result)
    expect(result.x).toEqual(20)
    expect(result.y).toEqual(20)
    expect(result.z).toEqual(0)
  })

  it('Vector3 operations addToRef', () => {
    const vector1 = Vector3.create(10, 10, -10) as Vector3.ReadonlyVector3
    const vector2 = Vector3.create(2, 2, 2) as Vector3.ReadonlyVector3
    const result = Vector3.create()

    Vector3.addToRef(vector1, vector2, result)
    expect(result.x).toEqual(12)
    expect(result.y).toEqual(12)
    expect(result.z).toEqual(-8)
  })

  it('Vector3 operations addToRef', () => {
    const result = Vector3.create()
    Vector3.copyFromFloats(20, 21, 23.5, result)
    expect(result.x).toEqual(20)
    expect(result.y).toEqual(21)
    expect(result.z).toEqual(23.5)
  })

  it('Vector3 getClipFactor', () => {
    const vector1 = Vector3.create(10, 10, -10) as Vector3.ReadonlyVector3
    const vector2 = Vector3.create(2, 2, 2) as Vector3.ReadonlyVector3
    const axis = Vector3.One()
    expect(Vector3.getClipFactor(vector1, vector2, axis, 1)).toEqual(2.25)
  })

  it('Vector3 getAngleBetweenVectors', () => {
    const vector1 = Vector3.create(5, 5, 5) as Vector3.ReadonlyVector3
    const vector2 = Vector3.Up() as Vector3.ReadonlyVector3
    const normal1 = Vector3.cross(vector1, vector2)
    const normal2 = Vector3.scale(
      Vector3.normalize(Vector3.negate(normal1)),
      0.5
    )

    const angle1 = Vector3.getAngleBetweenVectors(vector1, vector2, normal1)
    const angle2 = Vector3.getAngleBetweenVectors(vector1, vector2, normal2)
    expect(angle1).toBeCloseTo(0.9553)
    expect(angle2).toBeCloseTo(-0.9553)
  })

  it('Vector3 fromFloatArray', () => {
    const arr: FloatArray = new Array(100)
      .fill(0)
      .map(() => Math.random() * 100)

    const vector1 = Vector3.fromFloatArray(arr, 50)
    expect(vector1.x).toBeCloseTo(arr[50])
    expect(vector1.y).toBeCloseTo(arr[51])
    expect(vector1.z).toBeCloseTo(arr[52])

    const vector2 = Vector3.create()
    Vector3.fromFloatArrayToRef(arr, 23, vector2)
    expect(vector2.x).toBeCloseTo(arr[23])
    expect(vector2.y).toBeCloseTo(arr[24])
    expect(vector2.z).toBeCloseTo(arr[25])

    const vector3 = Vector3.fromFloatArray(arr)
    expect(vector3.x).toBeCloseTo(arr[0])
    expect(vector3.y).toBeCloseTo(arr[1])
    expect(vector3.z).toBeCloseTo(arr[2])
  })

  it('Vector3 normalizeToRef', () => {
    const vector2 = Vector3.create(1, 2, 3)
    Vector3.normalizeToRef(vector2, vector2)
    expect(vector2.x).toBeCloseTo(1 / Math.sqrt(1 + 2 ** 2 + 3 ** 2))
    expect(vector2.y).toBeCloseTo(2 / Math.sqrt(1 + 2 ** 2 + 3 ** 2))
    expect(vector2.z).toBeCloseTo(3 / Math.sqrt(1 + 2 ** 2 + 3 ** 2))
  })

  it('Vector3 lerp', () => {
    const vector1 = Vector3.create(1, 2, 3)
    const vector2 = Vector3.create(10, -10, 10)
    const result = Vector3.lerp(vector1, vector2, 0.6)
    expect(result.x).toBeCloseTo(1 + (10 - 1) * 0.6)
    expect(result.y).toBeCloseTo(2 + (-10 - 2) * 0.6)
    expect(result.z).toBeCloseTo(3 + (10 - 3) * 0.6)
  })

  it('Vector3 fract floor', () => {
    const vector1 = Vector3.create(13.15448, -2.1489, -78.98876)
    const floor = Vector3.floor(vector1)
    const fract = Vector3.fract(vector1)
    expect(floor).toStrictEqual(Vector3.create(13, -3, -79))
    expect(fract.x).toBeCloseTo(vector1.x - 13)
    expect(fract.y).toBeCloseTo(vector1.y - -3)
    expect(fract.z).toBeCloseTo(vector1.z - -79)
  })

  it('Vector3 minimize&maximize InPlaceFromFloatsToRef', () => {
    const vector1 = Vector3.create(13.15448, -2.1489, -78.98876)
    const result1 = Vector3.create()
    const result2 = Vector3.create()
    const result3 = Vector3.create()
    const result4 = Vector3.create()

    Vector3.minimizeInPlaceFromFloatsToRef(vector1, 10, 0, 0, result1)
    Vector3.maximizeInPlaceFromFloatsToRef(vector1, 10, 0, 0, result2)

    Vector3.minimizeInPlaceFromFloatsToRef(vector1, 15, -10, -90, result3)
    Vector3.maximizeInPlaceFromFloatsToRef(vector1, 15, -10, -90, result4)

    expect(result1.x).toBeCloseTo(10)
    expect(result1.y).toBeCloseTo(vector1.y)
    expect(result1.z).toBeCloseTo(vector1.z)

    expect(result2.x).toBeCloseTo(vector1.x)
    expect(result2.y).toBeCloseTo(0)
    expect(result2.z).toBeCloseTo(0)

    expect(result3.x).toBeCloseTo(vector1.x)
    expect(result3.y).toBeCloseTo(-10)
    expect(result3.z).toBeCloseTo(-90)

    expect(result4.x).toBeCloseTo(15)
    expect(result4.y).toBeCloseTo(vector1.y)
    expect(result4.z).toBeCloseTo(vector1.z)
  })

  it('Vector3 divide and multiply', () => {
    const vector1 = Vector3.create(3, 7, 11)
    const vector2 = Vector3.create(3, 7, 11)
    const result = Vector3.create()

    Vector3.divideToRef(vector1, vector2, result)
    expect(result).toStrictEqual(Vector3.One())
    expect(Vector3.divide(vector1, vector2)).toStrictEqual(Vector3.One())

    Vector3.multiplyToRef(vector1, vector2, result)
    expect(result).toStrictEqual(Vector3.create(9, 49, 121))
    expect(Vector3.multiply(vector1, vector2)).toStrictEqual(
      Vector3.create(9, 49, 121)
    )

    Vector3.multiplyByFloatsToRef(vector1, 0, 0, 0, result)
    expect(result).toStrictEqual(Vector3.Zero())
    expect(Vector3.multiplyByFloats(vector1, 0, 0, 0)).toStrictEqual(
      Vector3.Zero()
    )
  })

  it('Vector3 equals and more', () => {
    expect(Vector3.equals(Vector3.Backward(), Vector3.create(0, 0, -1))).toBe(
      true
    )
    expect(Vector3.equals(Vector3.Down(), Vector3.create(1, 0, 0))).toBe(false)
    expect(Vector3.equalsToFloats(Vector3.Down(), 0, -1, 0)).toBe(true)
    expect(
      Vector3.equalsWithEpsilon(
        Vector3.Down(),
        Vector3.create(0, -1 + 1.400298e-45, 1.400298e-45)
      )
    ).toBe(true)

    expect(Vector3.getHashCode(Vector3.create(14, 23, 29))).toBe(2198192)
    expect(Vector3.getHashCode(Vector3.Zero())).toBe(0)
    expect(Vector3.toString(Vector3.create(14, 23, 29))).toBe('(14, 23, 29)')
  })

  it('Vector3 clamp', () => {
    expect(
      Vector3.clamp(
        Vector3.create(10, 10, 10),
        Vector3.create(0, 0, 0),
        Vector3.create(20, 20, 20)
      )
    ).toStrictEqual(Vector3.create(10, 10, 10))
    expect(
      Vector3.clamp(
        Vector3.create(0, 0, 0),
        Vector3.create(10, 10, 10),
        Vector3.create(20, 20, 20)
      )
    ).toStrictEqual(Vector3.create(10, 10, 10))
    expect(
      Vector3.clamp(
        Vector3.create(20, 20, 20),
        Vector3.create(0, 0, 0),
        Vector3.create(10, 10, 10)
      )
    ).toStrictEqual(Vector3.create(10, 10, 10))
  })

  it('Vector3 distance center', () => {
    expect(
      Vector3.distance(Vector3.create(3, 0, 4), Vector3.create(0, 0, 0))
    ).toBe(5)
    expect(
      Vector3.distanceSquared(Vector3.create(1, 3, 4), Vector3.create(0, 0, 0))
    ).toBe(26)
    expect(
      Vector3.center(Vector3.create(1, 3, 4), Vector3.create(0, 0, 0))
    ).toStrictEqual(Vector3.create(0.5, 1.5, 2))
  })
  it('Vector3 transform transformNormal and applyMatrix', () => {
    const mat = Matrix.fromValues(
      1,
      2,
      3,
      0.1,
      1,
      2,
      3,
      0.1,
      1,
      2,
      3,
      0.1,
      0,
      0,
      0,
      0.1
    )
    expect(Vector3.transformNormal(Vector3.create(1, 1, 1), mat)).toStrictEqual(
      Vector3.create(3, 6, 9)
    )
    expect(Vector3.transformCoordinates(Vector3.One(), mat)).toStrictEqual(
      Vector3.create(7.5, 15, 22.5)
    )
    expect(Vector3.applyMatrix4(Vector3.One(), mat)).toStrictEqual(
      Vector3.create(7.5, 15, 22.5)
    )
  })
  it('Vector3 catmullRoll & hermite', () => {
    expect(
      Vector3.catmullRom(
        Vector3.Up(),
        Vector3.Left(),
        Vector3.Forward(),
        Vector3.create(1, 1, 1),
        10
      )
    ).toStrictEqual(Vector3.create(-801, 45, -845))

    expect(
      Vector3.hermite(
        Vector3.Up(),
        Vector3.Left(),
        Vector3.Forward(),
        Vector3.create(1, 1, 1),
        10
      )
    ).toStrictEqual(Vector3.create(90, 2601, -800))
  })

  it('Vector3 minimize & maximize', () => {
    const one = Vector3.create(12, 32, -4)
    const other = Vector3.create(8, 50, -50)
    expect(Vector3.minimize(one, other)).toStrictEqual(
      Vector3.create(8, 32, -50)
    )
    expect(Vector3.maximize(one, other)).toStrictEqual(
      Vector3.create(12, 50, -4)
    )
  })

  it('Vector3 rotationFromAxis', () => {
    const angles = Vector3.rotationFromAxis(
      Vector3.Up(),
      Vector3.Forward(),
      Vector3.One()
    )
    expect(angles.x).toBeCloseTo(1.622)
    expect(angles.y).toBeCloseTo(26.772)
    expect(angles.z).toBeCloseTo(42.67)
  })
})
