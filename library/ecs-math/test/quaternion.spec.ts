import { Quaternion, Vector3 } from '../src/'

const results = {
  staticAngle01: '90.00',
  staticAngle02: '14.85',
  staticAngle03: '4.99997',
  staticAngle04: '180',
  staticEuler01: '(0.0, 0.0, 0.0, 1.0)',
  staticEuler02: '(0.5, -0.5, 0.5, 0.5)',
  staticEuler03: '(0.0, 0.9, -0.4, 0.0)',
  staticEuler04: '(0.8, 0.0, 0.6, 0.0)',
  staticEuler05: '(-0.7, 0.2, -0.2, -0.6)',
  staticFromToRotation01: '(0.0, 180.0, 0.0)',
  staticFromToRotation02: '(0.0, 93.9, 0.0)',
  staticFromToRotation03: '(0.0, 0.0, 44.5)',
  staticFromToRotation04: '(45.0, 360.0, 360.0)',
  staticFromToRotation05: '(335.2, 276.2, 152.5)',
  staticFromToRotation06: '(39.1, 335.6, 352.5)',
  staticFromToRotation07: '(12.8, 45.1, 351.9)',
  staticFromToRotation08: '(35.3, 345.0, 315.0)',
  staticRotateTowards01: '(0.1, 0.1, 0.1, 1.0)',
  staticRotateTowards02: '(0.0, 0.1, 0.0, 1.0)',
  staticRotateTowards03: '(0.0, 0.1, -0.1, -1.0)',
  staticRotateTowards04: '(0.0, 0.0, 0.0, 1.0)',
  staticLookRotation01: '(-0.3, 0.4, 0.1, 0.9)',
  staticLookRotation02: '(0.0, 0.0, 0.0, 1.0)',
  staticLookRotation03: '(0.0, 0.7, 0.0, 0.7)',
  staticLookRotation04: '(0.0, 0.9, 0.4, 0.0)',
  staticSlerp01: '(0.5, 0.2, 0.2, 0.8)',
  staticSlerp02: '(0.0, 0.1, 0.8, 0.6)',
  staticSlerp03: '(-0.1, 0.0, -0.1, 1.0)',
  staticSlerp04: '(0.0, 0.3, 0.0, 0.9)',
  eulerAngles01: '(10.0, 10.0, 10.0)',
  eulerAngles02: '(0.0, 90.0, 0.0)',
  eulerAngles03: '(80.0, 190.0, 220.0)',
  eulerAngles04: '(360.0, 10.0, 0.0)',
  normalized01: '(0.1, 0.1, 0.1, 1.0)',
  normalized02: '(0.0, 0.7, 0.0, 0.7)',
  normalized03: '(-0.7, 0.2, -0.2, -0.6)',
  normalized04: '(0.0, -0.1, 0.0, -1.0)',
  setFromToRotation01: '(0.0, 0.0, -0.7, 0.7)',
  setFromToRotation02: '(0.7, 0.0, 0.0, 0.7)',
  setFromToRotation03: '(0.5, -0.3, -0.1, 0.8)',
  setFromToRotation04: '(-0.1, -0.5, -0.3, 0.8)',
  setFromToRotation05: '(0.0, 1.0, 0.0, 0.0)',
  setFromToRotation06: '(0.0, 0.0, 0.0, 1.0)',
  setFromToRotation07: '(-1.0, 0.0, 0.0, 0.0)',
  setFromToRotation08: '(0.0, 0.0, 0.0, 1.0)'
}

const normalize = (v: string) => (v === '-0.0' ? '0.0' : v)

function quaternionToString(quat: Quaternion.ReadonlyQuaternion) {
  const x = normalize(quat.x.toFixed(1).substr(0, 6))
  const y = normalize(quat.y.toFixed(1).substr(0, 6))
  const z = normalize(quat.z.toFixed(1).substr(0, 6))
  const w = normalize(quat.w.toFixed(1).substr(0, 6))

  return `(${x}, ${y}, ${z}, ${w})`
}

function vector3ToString(vec: Vector3.ReadonlyVector3) {
  const x = normalize(vec.x.toFixed(1).substr(0, 6))
  const y = normalize(vec.y.toFixed(1).substr(0, 6))
  const z = normalize(vec.z.toFixed(1).substr(0, 6))

  return `(${x}, ${y}, ${z})`
}

describe('ECS Quaternion - Next tests', () => {
  it('Quaternion.angle', () => {
    expect(
      Quaternion.angle(
        Quaternion.fromEulerDegrees(0, 0, 0),
        Quaternion.fromEulerDegrees(90, 90, 90)
      )
        .toString()
        .substr(0, 5)
    ).toEqual(results.staticAngle01.substr(0, 5))

    expect(
      Quaternion.angle(
        Quaternion.fromEulerDegrees(10, 0, 10),
        Quaternion.fromEulerDegrees(360, 0, -1)
      )
        .toString()
        .substr(0, 5)
    ).toEqual(results.staticAngle02.substr(0, 5))

    expect(
      Quaternion.angle(
        Quaternion.fromEulerDegrees(0, 5, 0),
        Quaternion.fromEulerDegrees(0, 0, 0)
      )
        .toString()
        .substr(0, 5)
    ).toEqual(results.staticAngle03.substr(0, 5))

    expect(
      Quaternion.angle(
        Quaternion.fromEulerDegrees(360, -360, 0),
        Quaternion.fromEulerDegrees(180, 90, 0)
      )
        .toString()
        .substr(0, 5)
    ).toEqual(results.staticAngle04.substr(0, 5))
  })

  it('Quaternion.fromEulerDegrees', () => {
    expect(quaternionToString(Quaternion.fromEulerDegrees(0, 0, 0))).toEqual(
      results.staticEuler01
    )

    expect(quaternionToString(Quaternion.fromEulerDegrees(90, 0, 90))).toEqual(
      results.staticEuler02
    )

    expect(
      quaternionToString(Quaternion.fromEulerDegrees(45, 180, -1))
    ).toEqual(results.staticEuler03)

    expect(
      quaternionToString(Quaternion.fromEulerDegrees(360, 110, -180))
    ).toEqual(results.staticEuler04)

    expect(
      quaternionToString(Quaternion.fromEulerDegrees(100, 10, 400))
    ).toEqual(results.staticEuler05)
  })

  it('Quaternion.rotateTowards', () => {
    expect(
      quaternionToString(
        Quaternion.rotateTowards(
          Quaternion.fromEulerDegrees(10, 10, 10),
          Quaternion.fromEulerDegrees(100, 100, 100),
          0.1
        )
      )
    ).toEqual(results.staticRotateTowards01)

    expect(
      quaternionToString(
        Quaternion.rotateTowards(
          Quaternion.fromEulerDegrees(0, 10, -0),
          Quaternion.fromEulerDegrees(0, 9, 45),
          0.1
        )
      )
    ).toEqual(results.staticRotateTowards02)

    expect(
      quaternionToString(
        Quaternion.rotateTowards(
          Quaternion.fromEulerDegrees(360, -10, 10),
          Quaternion.fromEulerDegrees(0, 100, 0),
          0.1
        )
      )
    ).toEqual(results.staticRotateTowards03)

    expect(
      quaternionToString(
        Quaternion.rotateTowards(
          Quaternion.fromEulerDegrees(0, 0, 0),
          Quaternion.fromEulerDegrees(0, 0, 0),
          0.1
        )
      )
    ).toEqual(results.staticRotateTowards04)
  })

  it('Quaternion.lookRotation', () => {
    expect(
      quaternionToString(
        Quaternion.lookRotation(Vector3.create(1, 1, 1), Vector3.Up())
      )
    ).toEqual(results.staticLookRotation01)

    expect(
      quaternionToString(
        Quaternion.lookRotation(Vector3.create(-10, -0, 110), Vector3.Up())
      )
    ).toEqual(results.staticLookRotation02)

    expect(
      quaternionToString(
        Quaternion.lookRotation(Vector3.create(1230, 10, 0), Vector3.Up())
      )
    ).toEqual(results.staticLookRotation03)

    expect(
      quaternionToString(
        Quaternion.lookRotation(Vector3.create(0, 123, -123), Vector3.Up())
      )
    ).toEqual(results.staticLookRotation04)
  })

  // These tests check against euler angles since we get a different quaternion result in Unity, but represent the same euler angles rotation.
  it('Quaternion.fromToRotation', () => {
    const rotation01 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(0, 0, 0),
        Vector3.create(100, 100, 100)
      )
    )

    const rotation02 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(-10, -0, 110),
        Vector3.create(4452, 0, 100)
      )
    )

    const rotation03 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(1230, 10, 0),
        Vector3.create(100, 100, 0)
      )
    )

    const rotation04 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(0, 123, -123),
        Vector3.create(100, 213123, 100)
      )
    )

    const rotation05 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(-10, -10, -10),
        Vector3.create(360, -10, 360)
      )
    )

    const rotation06 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(12, -0, -400),
        Vector3.create(200, 360, -400)
      )
    )

    const rotation07 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(25, 45, 180),
        Vector3.create(127, 0, 90)
      )
    )

    const rotation08 = Quaternion.toEulerAngles(
      Quaternion.fromToRotation(
        Vector3.create(0, 1, 0),
        Vector3.create(1, 1, 1)
      )
    )

    expect(vector3ToString(rotation01)).toEqual(results.staticFromToRotation01)
    expect(vector3ToString(rotation02)).toEqual(results.staticFromToRotation02)
    expect(vector3ToString(rotation03)).toEqual(results.staticFromToRotation03)
    expect(vector3ToString(rotation04)).toEqual(results.staticFromToRotation04)
    expect(vector3ToString(rotation05)).toEqual(results.staticFromToRotation05)
    expect(vector3ToString(rotation06)).toEqual(results.staticFromToRotation06)
    expect(vector3ToString(rotation07)).toEqual(results.staticFromToRotation07)
    expect(vector3ToString(rotation08)).toEqual(results.staticFromToRotation08)
  })

  it('quaternion.toEulerAngles', () => {
    expect(
      vector3ToString(
        Quaternion.toEulerAngles(Quaternion.fromEulerDegrees(10, 10, 10))
      )
    ).toEqual(results.eulerAngles01)

    expect(
      vector3ToString(
        Quaternion.toEulerAngles(Quaternion.fromEulerDegrees(0, 90, 0))
      )
    ).toEqual(results.eulerAngles02)

    expect(
      vector3ToString(
        Quaternion.toEulerAngles(Quaternion.fromEulerDegrees(100, 10, 400))
      )
    ).toEqual(results.eulerAngles03)

    expect(
      vector3ToString(
        Quaternion.toEulerAngles(Quaternion.fromEulerDegrees(360, 10, 0))
      )
    ).toEqual(results.eulerAngles04)
  })

  it('quaternion.normalized', () => {
    expect(
      quaternionToString(
        Quaternion.normalize(Quaternion.fromEulerDegrees(10, 10, 10))
      )
    ).toEqual(results.normalized01)

    expect(
      quaternionToString(
        Quaternion.normalize(Quaternion.fromEulerDegrees(0, 90, 0))
      )
    ).toEqual(results.normalized02)

    expect(
      quaternionToString(
        Quaternion.normalize(Quaternion.fromEulerDegrees(100, 10, 400))
      )
    ).toEqual(results.normalized03)

    expect(
      quaternionToString(
        Quaternion.normalize(Quaternion.fromEulerDegrees(360, 10, 0))
      )
    ).toEqual(results.normalized04)
  })

  it('quaternion.setFromToRotation', () => {
    let upVector = Vector3.Up()
    let quat = Quaternion.Identity()
    quat = Quaternion.fromToRotation(Vector3.Up(), Vector3.Right(), upVector)
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation01)

    upVector = Vector3.Up()
    quat = Quaternion.fromToRotation(Vector3.Up(), Vector3.Forward(), upVector)
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation02)

    upVector = Vector3.create(50, 0, -39)
    quat = Quaternion.fromToRotation(
      Vector3.create(38, 56, 23),
      Vector3.create(12, -5, 99),
      upVector
    )
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation03)

    upVector = Vector3.create(-60, -80, -23)
    quat = Quaternion.fromToRotation(
      Vector3.create(-50, 0.5, 15),
      Vector3.create(-66, 66, -91),
      upVector
    )
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation04)

    upVector = Vector3.Up()
    quat = Quaternion.fromToRotation(
      Vector3.create(-1, 0, 0),
      Vector3.create(1, 0, 0),
      upVector
    ) // Parallel opposite vectors case
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation05)

    upVector = Vector3.Up()
    quat = Quaternion.fromToRotation(
      Vector3.create(1, 0, 0),
      Vector3.create(1, 0, 0),
      upVector
    ) // same vectors case
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation06)

    upVector = Vector3.Left()
    quat = Quaternion.fromToRotation(
      Vector3.create(0, 1, 0),
      Vector3.create(0, -1, 0),
      upVector
    )
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation07)

    upVector = Vector3.Left()
    quat = Quaternion.fromToRotation(
      Vector3.create(0, -1, 0),
      Vector3.create(0, -1, 0),
      upVector
    )
    expect(quaternionToString(quat)).toEqual(results.setFromToRotation08)
  })

  it('Quaternion.Slerp', () => {
    expect(
      quaternionToString(
        Quaternion.slerp(
          Quaternion.fromEulerDegrees(10, 10, 10),
          Quaternion.fromEulerDegrees(45, 45, 45),
          1
        )
      )
    ).toEqual(results.staticSlerp01)

    expect(
      quaternionToString(
        Quaternion.slerp(
          Quaternion.fromEulerDegrees(-10, -0, 110),
          Quaternion.fromEulerDegrees(4100, 100, 100),
          0.00123
        )
      )
    ).toEqual(results.staticSlerp02)

    expect(
      quaternionToString(
        Quaternion.slerp(
          Quaternion.fromEulerDegrees(0, 123, -123),
          Quaternion.fromEulerDegrees(360, -10, 360),
          0.9
        )
      )
    ).toEqual(results.staticSlerp03)

    expect(
      quaternionToString(
        Quaternion.slerp(
          Quaternion.fromEulerDegrees(1, 1, 0),
          Quaternion.fromEulerDegrees(12, 12341, 1),
          0.4
        )
      )
    ).toEqual(results.staticSlerp04)
  })
})
