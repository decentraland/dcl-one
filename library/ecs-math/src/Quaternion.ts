import { Vector3 } from './Vector3'
import { Scalar } from './Scalar'
import { DEG2RAD, RAD2DEG } from './types'
import { Matrix } from './Matrix'

/**
 * @public
 * Quaternion is a type and a namespace.
 * - The namespace contains all types and functions to operates with Quaternion
 * - The type Quaternion is an alias to Quaternion.ReadonlyQuaternion
 * ```
 *
 * // Namespace usage example
 * const next = Quaternion.add(pointA, velocityA)
 *
 * // Type usage example
 * const readonlyRotation: Quaternion = Quaternion.Zero()
 * readonlyRotation.x = 0.1 // this FAILS
 *
 * // For mutable usage, use `Quaternion.Mutable`
 * const rotation: Quaternion.Mutable = Quaternion.Identity()
 * rotation.x = 3.0 // this WORKS
 * ```
 */
export type Quaternion = Quaternion.ReadonlyQuaternion

/**
 * @public
 * Quaternion is a type and a namespace.
 * ```
 * // The namespace contains all types and functions to operates with Quaternion
 * const next = Quaternion.add(pointA, velocityA)
 * // The type Quaternion is an alias to Quaternion.ReadonlyQuaternion
 * const readonlyRotation: Quaternion = Quaternion.Zero()
 * readonlyRotation.x = 0.1 // this FAILS
 *
 * // For mutable usage, use `Quaternion.Mutable`
 * const rotation: Quaternion.Mutable = Quaternion.Identity()
 * rotation.x = 3.0 // this WORKS
 * ```
 */
export namespace Quaternion {
  /**
   * @public
   * For external use, type with `Quaternion`, e.g. `const zeroRotation: Quaternion = Quaternion.Zero()`.
   * For mutable typing, use `Quaternion.Mutable`, e.g. `const identityQuaternion: Quaternion.Mutable = Quaternion.Identity()`.
   */
  export type ReadonlyQuaternion = {
    readonly x: number
    readonly y: number
    readonly z: number
    readonly w: number
  }

  /**
   * @public
   * For external usage, type with `Quaternion`, e.g. `const zeroRotation: Quaternion = Quaternion.Zero()`.
   * For mutable typing, use `Quaternion.Mutable`, e.g. `const identityQuaternion: Quaternion.Mutable = Quaternion.Identity()`.
   */
  export type MutableQuaternion = {
    x: number
    y: number
    z: number
    w: number
  }

  /**
   * @public
   * Type with `Quaternion` for readonly usage, e.g. `const zeroRotation: Quaternion = Quaternion.Zero()`.
   * For mutable, use `Quaternion.Mutable`, e.g. `const identityQuaternion: Quaternion.Mutable = Quaternion.Identity()`.
   */
  export type Mutable = MutableQuaternion

  /**
   * Creates a new Quaternion from the given floats
   * @param x - defines the first component (0 by default)
   * @param y - defines the second component (0 by default)
   * @param z - defines the third component (0 by default)
   * @param w - defines the fourth component (1.0 by default)
   */
  export function create(
    /** defines the first component (0 by default) */
    x: number = 0.0,
    /** defines the second component (0 by default) */
    y: number = 0.0,
    /** defines the third component (0 by default) */
    z: number = 0.0,
    /** defines the fourth component (1.0 by default) */
    w: number = 1.0
  ): MutableQuaternion {
    return { x, y, z, w }
  }

  /**
   * Returns a new Quaternion as the result of the addition of the two given quaternions.
   * @param q1 - the first quaternion
   * @param q2 - the second quaternion
   * @returns the resulting quaternion
   */
  export function add(
    q1: ReadonlyQuaternion,
    q2: ReadonlyQuaternion
  ): MutableQuaternion {
    return { x: q1.x + q2.x, y: q1.y + q2.y, z: q1.z + q2.z, w: q1.w + q2.w }
  }

  /**
   * Creates a new rotation from the given Euler float angles (y, x, z) and stores it in the target quaternion
   * @param yaw - defines the rotation around Y axis (radians)
   * @param pitch - defines the rotation around X axis (radians)
   * @param roll - defines the rotation around Z axis (radians)
   * @returns result quaternion
   */
  export function fromRotationYawPitchRoll(
    yaw: number,
    pitch: number,
    roll: number
  ): MutableQuaternion {
    // Implemented unity-based calculations from: https://stackoverflow.com/a/56055813

    const halfPitch = pitch * 0.5
    const halfYaw = yaw * 0.5
    const halfRoll = roll * 0.5

    const c1 = Math.cos(halfPitch)
    const c2 = Math.cos(halfYaw)
    const c3 = Math.cos(halfRoll)
    const s1 = Math.sin(halfPitch)
    const s2 = Math.sin(halfYaw)
    const s3 = Math.sin(halfRoll)

    return create(
      c2 * s1 * c3 + s2 * c1 * s3,
      s2 * c1 * c3 - c2 * s1 * s3,
      c2 * c1 * s3 - s2 * s1 * c3,
      c2 * c1 * c3 + s2 * s1 * s3
    )
  }

  /**
   * Returns a rotation that rotates z degrees around the z axis, x degrees around the x axis, and y degrees around the y axis.
   * @param x - the rotation on the x axis in euler degrees
   * @param y - the rotation on the y axis in euler degrees
   * @param z - the rotation on the z axis in euler degrees
   */
  export function fromEulerDegrees(
    x: number,
    y: number,
    z: number
  ): MutableQuaternion {
    return fromRotationYawPitchRoll(y * DEG2RAD, x * DEG2RAD, z * DEG2RAD)
  }

  /**
   * Gets length of current quaternion
   * @returns the quaternion length (float)
   */
  export function length(q: ReadonlyQuaternion): number {
    return Math.sqrt(lengthSquared(q))
  }

  /**
   * Gets length of current quaternion
   * @returns the quaternion length (float)
   */
  export function lengthSquared(q: ReadonlyQuaternion): number {
    return q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w
  }

  /**
   * Returns the dot product (float) between the quaternions "left" and "right"
   * @param left - defines the left operand
   * @param right - defines the right operand
   * @returns the dot product
   */
  export function dot(
    left: ReadonlyQuaternion,
    right: ReadonlyQuaternion
  ): number {
    return (
      left.x * right.x + left.y * right.y + left.z * right.z + left.w * right.w
    )
  }

  /**
   * Returns the angle in degrees between two rotations a and b.
   * @param quat1 - defines the first quaternion
   * @param quat2 - defines the second quaternion
   * @returns the degrees angle
   */
  export function angle(
    quat1: ReadonlyQuaternion,
    quat2: ReadonlyQuaternion
  ): number {
    const dotVal = dot(quat1, quat2)
    return Math.acos(Math.min(Math.abs(dotVal), 1)) * 2 * RAD2DEG
  }

  /**
   * The from quaternion is rotated towards to by an angular step of maxDegreesDelta.
   * @param from - defines the first quaternion
   * @param to - defines the second quaternion
   * @param maxDegreesDelta - the interval step
   */
  export function rotateTowards(
    from: ReadonlyQuaternion,
    to: ReadonlyQuaternion,
    maxDegreesDelta: number
  ): MutableQuaternion {
    const num: number = angle(from, to)
    if (num === 0) {
      return to
    }
    const t: number = Math.min(1, maxDegreesDelta / num)

    return slerp(from, to, t)
  }

  /**
   * Creates a rotation with the specified forward and upwards directions.
   * @param forward - the direction to look in
   * @param up - the vector that defines in which direction up is
   */
  export function lookRotation(
    forward: Vector3.ReadonlyVector3,
    up: Vector3.ReadonlyVector3 = { x: 0.0, y: 1.0, z: 0.0 }
  ): MutableQuaternion {
    const forwardNew = Vector3.normalize(forward)
    const right = Vector3.normalize(Vector3.cross(up, forwardNew))
    const upNew = Vector3.cross(forwardNew, right)
    const m00 = right.x
    const m01 = right.y
    const m02 = right.z
    const m10 = upNew.x
    const m11 = upNew.y
    const m12 = upNew.z
    const m20 = forwardNew.x
    const m21 = forwardNew.y
    const m22 = forwardNew.z

    const num8 = m00 + m11 + m22
    const quaternion = create()

    if (num8 > 0) {
      let num = Math.sqrt(num8 + 1)
      quaternion.w = num * 0.5
      num = 0.5 / num
      quaternion.x = (m12 - m21) * num
      quaternion.y = (m20 - m02) * num
      quaternion.z = (m01 - m10) * num
      return quaternion
    }

    if (m00 >= m11 && m00 >= m22) {
      const num7 = Math.sqrt(1 + m00 - m11 - m22)
      const num4 = 0.5 / num7
      quaternion.x = 0.5 * num7
      quaternion.y = (m01 + m10) * num4
      quaternion.z = (m02 + m20) * num4
      quaternion.w = (m12 - m21) * num4
      return quaternion
    }

    if (m11 > m22) {
      const num6 = Math.sqrt(1 + m11 - m00 - m22)
      const num3 = 0.5 / num6
      quaternion.x = (m10 + m01) * num3
      quaternion.y = 0.5 * num6
      quaternion.z = (m21 + m12) * num3
      quaternion.w = (m20 - m02) * num3
      return quaternion
    }

    const num5 = Math.sqrt(1 + m22 - m00 - m11)
    const num2 = 0.5 / num5
    quaternion.x = (m20 + m02) * num2
    quaternion.y = (m21 + m12) * num2
    quaternion.z = 0.5 * num5
    quaternion.w = (m01 - m10) * num2
    return quaternion
  }

  /**
   * Normalize in place the current quaternion
   * @returns the current updated quaternion
   */
  export function normalize(q: ReadonlyQuaternion): MutableQuaternion {
    const qLength = 1.0 / length(q)
    return create(q.x * qLength, q.y * qLength, q.z * qLength, q.w * qLength)
  }

  /**
   * Creates a rotation which rotates from fromDirection to toDirection.
   * @param from - defines the first direction Vector
   * @param to - defines the target direction Vector
   */
  export function fromToRotation(
    from: Vector3.ReadonlyVector3,
    to: Vector3.ReadonlyVector3,
    up: Vector3.ReadonlyVector3 = Vector3.Up()
  ): MutableQuaternion {
    // Unity-based calculations implemented from https://forum.unity.com/threads/quaternion-lookrotation-around-an-axis.608470/#post-4069888

    const v0 = Vector3.normalize(from)
    const v1 = Vector3.normalize(to)

    const a = Vector3.cross(v0, v1)
    const w =
      Math.sqrt(Vector3.lengthSquared(v0) * Vector3.lengthSquared(v1)) +
      Vector3.dot(v0, v1)
    if (Vector3.lengthSquared(a) < 0.0001) {
      // the vectors are parallel, check w to find direction
      // if w is 0 then values are opposite, and we sould rotate 180 degrees around the supplied axis
      // otherwise the vectors in the same direction and no rotation should occur
      return Math.abs(w) < 0.0001
        ? normalize(create(up.x, up.y, up.z, 0))
        : Identity()
    } else {
      return normalize(create(a.x, a.y, a.z, w))
    }
  }

  /**
   * Creates an identity quaternion
   * @returns - the identity quaternion
   */
  export function Identity(): MutableQuaternion {
    return create(0.0, 0.0, 0.0, 1.0)
  }

  /**
   * Gets or sets the euler angle representation of the rotation.
   * Implemented unity-based calculations from: https://stackoverflow.com/a/56055813
   * @public
   * @returns a new Vector3 with euler angles degrees
   */
  export function toEulerAngles(q: MutableQuaternion): Vector3.Mutable {
    const out = Vector3.create()

    // if the input quaternion is normalized, this is exactly one. Otherwise, this acts as a correction factor for the quaternion's not-normalizedness
    const unit = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w

    // q will have a magnitude of 0.5 or greater if and only if q is a singularity case
    const test = q.x * q.w - q.y * q.z

    if (test > 0.4995 * unit) {
      // singularity at north pole
      out.x = Math.PI / 2
      out.y = 2 * Math.atan2(q.y, q.x)
      out.z = 0
    } else if (test < -0.4995 * unit) {
      // singularity at south pole
      out.x = -Math.PI / 2
      out.y = -2 * Math.atan2(q.y, q.x)
      out.z = 0
    } else {
      // no singularity - q is the majority of cases
      out.x = Math.asin(2 * (q.w * q.x - q.y * q.z))
      out.y = Math.atan2(
        2 * q.w * q.y + 2 * q.z * q.x,
        1 - 2 * (q.x * q.x + q.y * q.y)
      )
      out.z = Math.atan2(
        2 * q.w * q.z + 2 * q.x * q.y,
        1 - 2 * (q.z * q.z + q.x * q.x)
      )
    }
    out.x *= RAD2DEG
    out.y *= RAD2DEG
    out.z *= RAD2DEG

    // ensure the degree values are between 0 and 360
    out.x = Scalar.repeat(out.x, 360)
    out.y = Scalar.repeat(out.y, 360)
    out.z = Scalar.repeat(out.z, 360)

    return out
  }

  /**
   * Creates a new rotation from the given Euler float angles (y, x, z) and stores it in the target quaternion
   * @param yaw - defines the rotation around Y axis (radians)
   * @param pitch - defines the rotation around X axis (radians)
   * @param roll - defines the rotation around Z axis (radians)
   * @param result - defines the target quaternion
   */
  export function fromRotationYawPitchRollToRef(
    yaw: number,
    pitch: number,
    roll: number,
    result: Quaternion.MutableQuaternion
  ): void {
    // Implemented unity-based calculations from: https://stackoverflow.com/a/56055813

    const halfPitch = pitch * 0.5
    const halfYaw = yaw * 0.5
    const halfRoll = roll * 0.5

    const c1 = Math.cos(halfPitch)
    const c2 = Math.cos(halfYaw)
    const c3 = Math.cos(halfRoll)
    const s1 = Math.sin(halfPitch)
    const s2 = Math.sin(halfYaw)
    const s3 = Math.sin(halfRoll)

    result.x = c2 * s1 * c3 + s2 * c1 * s3
    result.y = s2 * c1 * c3 - c2 * s1 * s3
    result.z = c2 * c1 * s3 - s2 * s1 * c3
    result.w = c2 * c1 * c3 + s2 * s1 * s3
  }

  /**
   * Updates the given quaternion with the given rotation matrix values
   * @param matrix - defines the source matrix
   * @param result - defines the target quaternion
   */
  export function fromRotationMatrixToRef(
    matrix: Matrix.ReadonlyMatrix,
    result: Quaternion.MutableQuaternion
  ): void {
    const data = matrix._m
    // tslint:disable:one-variable-per-declaration
    const m11 = data[0],
      m12 = data[4],
      m13 = data[8]
    const m21 = data[1],
      m22 = data[5],
      m23 = data[9]
    const m31 = data[2],
      m32 = data[6],
      m33 = data[10]
    // tslint:enable:one-variable-per-declaration
    const trace = m11 + m22 + m33
    let s

    if (trace > 0) {
      s = 0.5 / Math.sqrt(trace + 1.0)

      result.w = 0.25 / s
      result.x = (m32 - m23) * s
      result.y = (m13 - m31) * s
      result.z = (m21 - m12) * s
    } else if (m11 > m22 && m11 > m33) {
      s = 2.0 * Math.sqrt(1.0 + m11 - m22 - m33)

      result.w = (m32 - m23) / s
      result.x = 0.25 * s
      result.y = (m12 + m21) / s
      result.z = (m13 + m31) / s
    } else if (m22 > m33) {
      s = 2.0 * Math.sqrt(1.0 + m22 - m11 - m33)

      result.w = (m13 - m31) / s
      result.x = (m12 + m21) / s
      result.y = 0.25 * s
      result.z = (m23 + m32) / s
    } else {
      s = 2.0 * Math.sqrt(1.0 + m33 - m11 - m22)

      result.w = (m21 - m12) / s
      result.x = (m13 + m31) / s
      result.y = (m23 + m32) / s
      result.z = 0.25 * s
    }
  }

  /**
   * Interpolates between two quaternions
   * @param left - defines first quaternion
   * @param right - defines second quaternion
   * @param amount - defines the gradient to use
   * @returns the new interpolated quaternion
   */
  export function slerp(
    left: ReadonlyQuaternion,
    right: ReadonlyQuaternion,
    amount: number
  ): MutableQuaternion {
    const result = Quaternion.Identity()
    Quaternion.slerpToRef(left, right, amount, result)
    return result
  }

  /**
   * Interpolates between two quaternions and stores it into a target quaternion
   * @param left - defines first quaternion
   * @param right - defines second quaternion
   * @param amount - defines the gradient to use
   * @param result - defines the target quaternion
   */
  export function slerpToRef(
    left: ReadonlyQuaternion,
    right: ReadonlyQuaternion,
    amount: number,
    result: MutableQuaternion
  ): void {
    let num2
    let num3
    let num4 =
      left.x * right.x + left.y * right.y + left.z * right.z + left.w * right.w
    let flag = false

    if (num4 < 0) {
      flag = true
      num4 = -num4
    }

    if (num4 > 0.999999) {
      num3 = 1 - amount
      num2 = flag ? -amount : amount
    } else {
      const num5 = Math.acos(num4)
      const num6 = 1.0 / Math.sin(num5)
      num3 = Math.sin((1.0 - amount) * num5) * num6
      num2 = flag
        ? -Math.sin(amount * num5) * num6
        : Math.sin(amount * num5) * num6
    }

    result.x = num3 * left.x + num2 * right.x
    result.y = num3 * left.y + num2 * right.y
    result.z = num3 * left.z + num2 * right.z
    result.w = num3 * left.w + num2 * right.w
  }

  /**
   * Multiplies two quaternions
   * @param self - defines the first operand
   * @param q1 - defines the second operand
   * @returns a new quaternion set as the multiplication result of the self one with the given one "q1"
   */
  export function multiply(
    self: ReadonlyQuaternion,
    q1: ReadonlyQuaternion
  ): MutableQuaternion {
    const result = create(0, 0, 0, 1.0)
    multiplyToRef(self, q1, result)
    return result
  }

  /**
   * Sets the given "result" as the the multiplication result of the self one with the given one "q1"
   * @param self - defines the first operand
   * @param q1 - defines the second operand
   * @param result - defines the target quaternion
   * @returns the current quaternion
   */
  export function multiplyToRef(
    self: ReadonlyQuaternion,
    q1: ReadonlyQuaternion,
    result: MutableQuaternion
  ): void {
    result.x = self.x * q1.w + self.y * q1.z - self.z * q1.y + self.w * q1.x
    result.y = -self.x * q1.z + self.y * q1.w + self.z * q1.x + self.w * q1.y
    result.z = self.x * q1.y - self.y * q1.x + self.z * q1.w + self.w * q1.z
    result.w = -self.x * q1.x - self.y * q1.y - self.z * q1.z + self.w * q1.w
  }

  /**
   *
   * @param degrees - the angle degrees
   * @param axis - vector3
   * @returns a new Quaternion
   */
  export function fromAngleAxis(
    degrees: number,
    axis: Vector3.ReadonlyVector3
  ): MutableQuaternion {
    if (Vector3.lengthSquared(axis) === 0) {
      return Quaternion.Identity()
    }

    const result: MutableQuaternion = Identity()
    let radians = degrees * DEG2RAD
    radians *= 0.5

    const a2: Vector3.MutableVector3 = Vector3.normalize(axis)
    Vector3.scaleToRef(a2, Math.sin(radians), a2)

    result.x = a2.x
    result.y = a2.y
    result.z = a2.z
    result.w = Math.cos(radians)

    return normalize(result)
  }

  /**
   * Creates a new quaternion containing the rotation value to reach the target (axis1, axis2, axis3) orientation as a rotated XYZ system (axis1, axis2 and axis3 are normalized during this operation)
   * @param axis1 - defines the first axis
   * @param axis2 - defines the second axis
   * @param axis3 - defines the third axis
   * @returns the new quaternion
   */
  export function fromAxisToRotationQuaternion(
    axis1: Vector3.ReadonlyVector3,
    axis2: Vector3.ReadonlyVector3,
    axis3: Vector3.ReadonlyVector3
  ): MutableQuaternion {
    const quat = Quaternion.create(0.0, 0.0, 0.0, 0.0)
    fromAxisToRotationQuaternionToRef(axis1, axis2, axis3, quat)
    return quat
  }

  /**
   * Creates a rotation value to reach the target (axis1, axis2, axis3) orientation as a rotated XYZ system (axis1, axis2 and axis3 are normalized during this operation) and stores it in the target quaternion
   * @param axis1 - defines the first axis
   * @param axis2 - defines the second axis
   * @param axis3 - defines the third axis
   * @param ref - defines the target quaternion
   */
  export function fromAxisToRotationQuaternionToRef(
    axis1: Vector3.ReadonlyVector3,
    axis2: Vector3.ReadonlyVector3,
    axis3: Vector3.ReadonlyVector3,
    ref: MutableQuaternion
  ): void {
    const rotMat = Matrix.create()
    Matrix.fromXYZAxesToRef(
      Vector3.normalize(axis1),
      Vector3.normalize(axis2),
      Vector3.normalize(axis3),
      rotMat
    )
    Quaternion.fromRotationMatrixToRef(rotMat, ref)
  }

  /**
   * Returns a zero filled quaternion
   */
  export function Zero(): MutableQuaternion {
    return create(0.0, 0.0, 0.0, 0.0)
  }

  /**
   * @public
   * Rotates the transform so the forward vector points at target's current position.
   */
  export function fromLookAt(
    position: Vector3.ReadonlyVector3,
    target: Vector3.ReadonlyVector3,
    worldUp: Vector3.ReadonlyVector3 = Vector3.Up()
  ) {
    const result = Quaternion.Identity()
    fromLookAtToRef(position, target, worldUp, result)
    return result
  }

  /**
   * @public
   * Rotates the transform so the forward vector points at target's current position.
   */
  export function fromLookAtToRef(
    position: Vector3.ReadonlyVector3,
    target: Vector3.ReadonlyVector3,
    worldUp: Vector3.ReadonlyVector3 = Vector3.Up(),
    result: MutableQuaternion
  ): void {
    const m = Matrix.Identity()
    Matrix.lookAtLHToRef(position, target, worldUp, m)
    Matrix.invertToRef(m, m)

    Quaternion.fromRotationMatrixToRef(m, result)
  }
}
