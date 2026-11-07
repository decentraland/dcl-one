import { Epsilon, FloatArray } from './types'
import { Quaternion } from './Quaternion'
import { Matrix } from './Matrix'
import { Scalar } from './Scalar'

/**
 * @public
 * Vector3 is a type and a namespace.
 * - The namespace contains all types and functions to operates with Vector3
 * - The type Vector3 is an alias to Vector3.ReadonlyVector3
 * ```
 *
 * // Namespace usage example
 * const next = Vector3.add(pointA, velocityA)
 *
 * // Type usage example
 * const readonlyPosition: Vector3 = Vector3.Zero()
 * readonlyPosition.x = 0.1 // this FAILS
 *
 * // For mutable usage, use `Vector3.Mutable`
 * const position: Vector3.Mutable = Vector3.One()
 * position.x = 3.0 // this WORKS
 * ```
 */
export type Vector3 = Vector3.ReadonlyVector3

/**
 * @public
 * Vector3 is a type and a namespace.
 * ```
 * // The namespace contains all types and functions to operates with Vector3
 * const next = Vector3.add(pointA, velocityA)
 * // The type Vector3 is an alias to Vector3.ReadonlyVector3
 * const readonlyPosition: Vector3 = Vector3.Zero()
 * readonlyPosition.x = 0.1 // this FAILS
 *
 * // For mutable usage, use `Vector3.Mutable`
 * const position: Vector3.Mutable = Vector3.One()
 * position.x = 3.0 // this WORKS
 * ```
 */
export namespace Vector3 {
  /**
   * @public
   * For external use, type with `Vector3`, e.g. `const zeroPosition: Vector3 = Vector3.Zero()`.
   * For mutable typing, use `Vector3.Mutable`, e.g. `const upVector: Vector3.Mutable = Vector3.Up()`.
   */
  export type ReadonlyVector3 = {
    readonly x: number
    readonly y: number
    readonly z: number
  }

  /**
   * @public
   * For external usage, type with `Vector3`, e.g. `const zeroPosition: Vector3 = Vector3.Zero()`.
   * For mutable typing, use `Vector3.Mutable`, e.g. `const upVector: Vector3.Mutable = Vector3.Up()`.
   */
  export type MutableVector3 = {
    x: number
    y: number
    z: number
  }

  /**
   * @public
   * Type with `Vector3` for readonly usage, e.g. `const zeroPosition: Vector3 = Vector3.Zero()`.
   * For mutable, use `Vector3.Mutable`, e.g. `const upVector: Vector3.Mutable = Vector3.Up()`.
   */
  export type Mutable = MutableVector3

  /**
   * Gets a boolean indicating that the vector is non uniform meaning x, y or z are not all the same
   * @param vector - vector to check
   */
  export function isNonUniform(vector: ReadonlyVector3): boolean {
    const absX = Math.abs(vector.x)
    const absY = Math.abs(vector.y)
    if (absX !== absY) {
      return true
    }

    const absZ = Math.abs(vector.z)
    if (absX !== absZ) {
      return true
    }

    return false
  }

  /**
   * Creates a new Vector3 object from the given x, y, z (floats) coordinates.
   * @param x - defines the first coordinates (on X axis)
   * @param y - defines the second coordinates (on Y axis)
   * @param z - defines the third coordinates (on Z axis)
   */
  export function create(
    /**
     * Defines the first coordinates (on X axis)
     */
    x: number = 0,
    /**
     * Defines the second coordinates (on Y axis)
     */
    y: number = 0,
    /**
     * Defines the third coordinates (on Z axis)
     */
    z: number = 0
  ): MutableVector3 {
    return { x, y, z }
  }

  /**
   * Returns a new Vector3 as the result of the addition of the two given vectors.
   * @param vector1 - the first vector
   * @param vector2 - the second vector
   * @returns the resulting vector
   */
  export function add(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3
  ): MutableVector3 {
    return {
      x: vector1.x + vector2.x,
      y: vector1.y + vector2.y,
      z: vector1.z + vector2.z
    }
  }

  /**
   * Performs addition between vectorA and vectorB and stores the result into result
   * @param vectorA - the first vector for the addition operation
   * @param vectorB - the second vector for the addition operation
   * @param result - the vector where the result of the addition is stored
   */
  export function addToRef(
    vectorA: ReadonlyVector3,
    vectorB: ReadonlyVector3,
    result: MutableVector3
  ): void {
    result.x = vectorA.x + vectorB.x
    result.y = vectorA.y + vectorB.y
    result.z = vectorA.z + vectorB.z
  }

  /**
   * Returns a new Vector3 as the result of the substraction of the two given vectors.
   * @returns the resulting vector
   */
  export function subtract(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3
  ): MutableVector3 {
    return {
      x: vector1.x - vector2.x,
      y: vector1.y - vector2.y,
      z: vector1.z - vector2.z
    }
  }

  /**
   * Performs substraction between vectorA and vectorB and stores the result into result
   * @param vectorA - the first vector for the substraction operation
   * @param vectorB - the second vector for the substraction operation
   * @param result - the vector where the result of the substraction is stored
   */
  export function subtractToRef(
    vectorA: ReadonlyVector3,
    vectorB: ReadonlyVector3,
    result: MutableVector3
  ): void {
    result.x = vectorA.x - vectorB.x
    result.y = vectorA.y - vectorB.y
    result.z = vectorA.z - vectorB.z
  }

  /**
   * Subtracts the given floats from the current Vector3 coordinates and set the given vector "result" with this result
   * @param x - defines the x coordinate of the operand
   * @param y - defines the y coordinate of the operand
   * @param z - defines the z coordinate of the operand
   * @param result - defines the Vector3 object where to store the result
   */
  export function subtractFromFloatsToRef(
    vector1: ReadonlyVector3,
    x: number,
    y: number,
    z: number,
    result: MutableVector3
  ): void {
    result.x = vector1.x - x
    result.y = vector1.y - y
    result.z = vector1.z - z
  }

  /**
   * Returns a new Vector3 with the other sign
   * @returns the resulting vector
   */
  export function negate(value: ReadonlyVector3): MutableVector3 {
    return { x: -value.x, y: -value.y, z: -value.z }
  }

  /**
   * Copy source into dest
   *
   */
  export function copyFrom(
    source: ReadonlyVector3,
    dest: MutableVector3
  ): void {
    dest.x = source.x
    dest.y = source.y
    dest.z = source.z
  }

  /**
   * Sets the given vector "dest" with the given floats.
   * @param x - defines the x coordinate of the source
   * @param y - defines the y coordinate of the source
   * @param z - defines the z coordinate of the source
   * @param dest - defines the Vector3 where to store the result
   */
  export function copyFromFloats(
    x: number,
    y: number,
    z: number,
    dest: MutableVector3
  ): void {
    dest.x = x
    dest.y = y
    dest.z = z
  }

  /**
   * Returns a new Vector3 with the same value
   * @returns the resulting vector
   */
  export function clone(source: ReadonlyVector3): MutableVector3 {
    return create(source.x, source.y, source.z)
  }

  /**
   * Get the clip factor between two vectors
   * @param vector0 - defines the first operand
   * @param vector1 - defines the second operand
   * @param axis - defines the axis to use
   * @param size - defines the size along the axis
   * @returns the clip factor
   */
  export function getClipFactor(
    vector0: ReadonlyVector3,
    vector1: ReadonlyVector3,
    axis: ReadonlyVector3,
    size: number
  ) {
    const d0 = dot(vector0, axis) - size
    const d1 = dot(vector1, axis) - size

    const s = d0 / (d0 - d1)

    return s
  }

  /**
   * Get angle between two vectors
   * @param vector0 - angle between vector0 and vector1
   * @param vector1 - angle between vector0 and vector1
   * @param normal - direction of the normal
   * @returns the angle between vector0 and vector1
   */
  export function getAngleBetweenVectors(
    vector0: ReadonlyVector3,
    vector1: ReadonlyVector3,
    normal: ReadonlyVector3
  ): number {
    const v0 = normalize(vector0)
    const v1 = normalize(vector1)
    const v0v1dot: number = dot(v0, v1)
    const n = create()
    crossToRef(v0, v1, n)
    if (dot(n, normal) > 0) {
      return Math.acos(v0v1dot)
    }
    return -Math.acos(v0v1dot)
  }

  /**
   * Returns a new Vector3 set from the index "offset" of the given array
   * @param array - defines the source array
   * @param offset - defines the offset in the source array
   * @returns the new Vector3
   */
  export function fromArray(
    array: FloatArray,
    offset: number = 0
  ): MutableVector3 {
    return create(array[offset], array[offset + 1], array[offset + 2])
  }

  /**
   * Returns a new Vector3 set from the index "offset" of the given FloatArray
   * This function is deprecated.  Use FromArray instead
   * @param array - defines the source array
   * @param offset - defines the offset in the source array
   * @returns the new Vector3
   */
  export function fromFloatArray(
    array: FloatArray,
    offset?: number
  ): MutableVector3 {
    return fromArray(array, offset)
  }

  /**
   * Sets the given vector "result" with the element values from the index "offset" of the given array
   * @param array - defines the source array
   * @param offset - defines the offset in the source array
   * @param result - defines the Vector3 where to store the result
   */
  export function fromArrayToRef(
    array: number[],
    offset: number,
    result: MutableVector3
  ): void {
    result.x = array[offset]
    result.y = array[offset + 1]
    result.z = array[offset + 2]
  }

  /**
   * Sets the given vector "result" with the element values from the index "offset" of the given FloatArray
   * This function is deprecated.  Use FromArrayToRef instead.
   * @param array - defines the source array
   * @param offset - defines the offset in the source array
   * @param result - defines the Vector3 where to store the result
   */
  export function fromFloatArrayToRef(
    array: FloatArray,
    offset: number,
    result: MutableVector3
  ): void {
    return fromArrayToRef(array, offset, result)
  }

  // Properties
  /**
   * Gets the length of the Vector3
   * @returns the length of the Vector3
   */
  export function length(vector: ReadonlyVector3): number {
    return Math.sqrt(
      vector.x * vector.x + vector.y * vector.y + vector.z * vector.z
    )
  }

  /**
   * Gets the squared length of the Vector3
   * @returns squared length of the Vector3
   */
  export function lengthSquared(vector: ReadonlyVector3): number {
    return vector.x * vector.x + vector.y * vector.y + vector.z * vector.z
  }

  /**
   * Returns a new Vector3 set with the current Vector3 coordinates multiplied by the float "scale"
   * @param scale - defines the multiplier factor
   * @returns a new Vector3
   */
  export function scaleToRef(
    vector: ReadonlyVector3,
    scale: number,
    result: MutableVector3
  ): void {
    result.x = vector.x * scale
    result.y = vector.y * scale
    result.z = vector.z * scale
  }

  /**
   * Returns a new Vector3 set with the current Vector3 coordinates multiplied by the float "scale"
   * @param scale - defines the multiplier factor
   * @returns a new Vector3
   */
  export function scale(
    vector: ReadonlyVector3,
    scale: number
  ): MutableVector3 {
    return create(vector.x * scale, vector.y * scale, vector.z * scale)
  }

  /**
   * Normalize the current Vector3 with the given input length.
   * Please note that this is an in place operation.
   * @param len - the length of the vector
   * @returns the current updated Vector3
   */
  export function normalizeFromLength(
    vector: ReadonlyVector3,
    len: number
  ): MutableVector3 {
    const result = create(0, 0, 0)
    normalizeFromLengthToRef(vector, len, result)
    return result
  }

  /**
   * Normalize the current Vector3 with the given input length.
   * Please note that this is an in place operation.
   * @param len - the length of the vector
   * @returns the current updated Vector3
   */
  export function normalizeFromLengthToRef(
    vector: ReadonlyVector3,
    len: number,
    result: MutableVector3
  ): void {
    if (len === 0 || len === 1.0) {
      copyFrom(vector, result)
      return
    }
    scaleToRef(vector, 1.0 / len, result)
  }

  /**
   * Normalize the current Vector3.
   * Please note that this is an in place operation.
   * @returns the current updated Vector3
   */
  export function normalize(vector: ReadonlyVector3): MutableVector3 {
    return normalizeFromLength(vector, length(vector))
  }

  /**
   * Normalize the current Vector3.
   * Please note that this is an in place operation.
   * @returns the current updated Vector3
   */
  export function normalizeToRef(
    vector: ReadonlyVector3,
    result: MutableVector3
  ): void {
    normalizeFromLengthToRef(vector, length(vector), result)
  }

  /**
   * Returns the dot product (float) between the vectors "left" and "right"
   * @param left - defines the left operand
   * @param right - defines the right operand
   * @returns the dot product
   */
  export function dot(left: ReadonlyVector3, right: ReadonlyVector3): number {
    return left.x * right.x + left.y * right.y + left.z * right.z
  }

  /**
   * Multiplies this vector (with an implicit 1 in the 4th dimension) and m, and divides by perspective
   * @param matrix - The transformation matrix
   * @returns result Vector3
   */
  export function applyMatrix4(
    vector: ReadonlyVector3,
    matrix: Matrix.ReadonlyMatrix
  ): MutableVector3 {
    const result = clone(vector)
    applyMatrix4ToRef(vector, matrix, result)
    return result
  }

  /**
   * Multiplies this vector (with an implicit 1 in the 4th dimension) and m, and divides by perspective and set the given vector "result" with this result
   * @param matrix - The transformation matrix
   * @param result - defines the Vector3 object where to store the result
   */
  export function applyMatrix4ToRef(
    vector: ReadonlyVector3,
    matrix: Matrix.ReadonlyMatrix,
    result: MutableVector3
  ): void {
    const { x, y, z } = vector
    const m = matrix._m
    const w = 1 / (m[3] * x + m[7] * y + m[11] * z + m[15])

    result.x = (m[0] * x + m[4] * y + m[8] * z + m[12]) * w
    result.y = (m[1] * x + m[5] * y + m[9] * z + m[13]) * w
    result.z = (m[2] * x + m[6] * y + m[10] * z + m[14]) * w
  }

  /**
   * Rotates the current Vector3 based on the given quaternion
   * @param q - defines the Quaternion
   * @returns the current Vector3
   */
  export function rotate(
    vector: ReadonlyVector3,
    q: Quaternion.ReadonlyQuaternion
  ): MutableVector3 {
    const result = create()
    rotateToRef(vector, q, result)
    return result
  }

  /**
   * Rotates current Vector3 based on the given quaternion, but applies the rotation to target Vector3.
   * @param q - defines the Quaternion
   * @param result - defines the target Vector3
   * @returns the current Vector3
   */
  export function rotateToRef(
    vector: ReadonlyVector3,
    q: Quaternion.ReadonlyQuaternion,
    result: MutableVector3
  ): void {
    const { x, y, z } = vector
    const { x: qx, y: qy, z: qz, w: qw } = q

    // calculate quat * vector

    const ix = qw * x + qy * z - qz * y
    const iy = qw * y + qz * x - qx * z
    const iz = qw * z + qx * y - qy * x
    const iw = -qx * x - qy * y - qz * z

    // calculate result * inverse quat

    result.x = ix * qw + iw * -qx + iy * -qz - iz * -qy
    result.y = iy * qw + iw * -qy + iz * -qx - ix * -qz
    result.z = iz * qw + iw * -qz + ix * -qy - iy * -qx
  }

  /**
   * Returns a new Vector3 located for "amount" (float) on the linear interpolation between the vectors "start" and "end"
   * @param start - defines the start value
   * @param end - defines the end value
   * @param amount - max defines amount between both (between 0 and 1)
   * @returns the new Vector3
   */
  export function lerp(
    start: ReadonlyVector3,
    end: ReadonlyVector3,
    amount: number
  ): MutableVector3 {
    const result = create(0, 0, 0)
    lerpToRef(start, end, amount, result)
    return result
  }

  /**
   * Sets the given vector "result" with the result of the linear interpolation from the vector "start" for "amount" to the vector "end"
   * @param start - defines the start value
   * @param end - defines the end value
   * @param amount - max defines amount between both (between 0 and 1)
   * @param result - defines the Vector3 where to store the result
   */
  export function lerpToRef(
    start: ReadonlyVector3,
    end: ReadonlyVector3,
    amount: number,
    result: MutableVector3
  ): void {
    result.x = start.x + (end.x - start.x) * amount
    result.y = start.y + (end.y - start.y) * amount
    result.z = start.z + (end.z - start.z) * amount
  }

  /**
   * Returns a new Vector3 as the cross product of the vectors "left" and "right"
   * The cross product is then orthogonal to both "left" and "right"
   * @param left - defines the left operand
   * @param right - defines the right operand
   * @returns the cross product
   */
  export function cross(
    left: ReadonlyVector3,
    right: ReadonlyVector3
  ): MutableVector3 {
    const result = Zero()
    crossToRef(left, right, result)
    return result
  }

  /**
   * Sets the given vector "result" with the cross product of "left" and "right"
   * The cross product is then orthogonal to both "left" and "right"
   * @param left - defines the left operand
   * @param right - defines the right operand
   * @param result - defines the Vector3 where to store the result
   */
  export function crossToRef(
    left: ReadonlyVector3,
    right: ReadonlyVector3,
    result: MutableVector3
  ): void {
    result.x = left.y * right.z - left.z * right.y
    result.y = left.z * right.x - left.x * right.z
    result.z = left.x * right.y - left.y * right.x
  }

  /**
   * Returns a new Vector3 set with the result of the transformation by the given matrix of the given vector.
   * This method computes tranformed coordinates only, not transformed direction vectors (ie. it takes translation in account)
   * @param vector - defines the Vector3 to transform
   * @param transformation - defines the transformation matrix
   * @returns the transformed Vector3
   */
  export function transformCoordinates(
    vector: ReadonlyVector3,
    transformation: Matrix.ReadonlyMatrix
  ): MutableVector3 {
    const result = Zero()
    transformCoordinatesToRef(vector, transformation, result)
    return result
  }

  /**
   * Sets the given vector "result" coordinates with the result of the transformation by the given matrix of the given vector
   * This method computes tranformed coordinates only, not transformed direction vectors (ie. it takes translation in account)
   * @param vector - defines the Vector3 to transform
   * @param transformation - defines the transformation matrix
   * @param result - defines the Vector3 where to store the result
   */
  export function transformCoordinatesToRef(
    vector: ReadonlyVector3,
    transformation: Matrix.ReadonlyMatrix,
    result: MutableVector3
  ): void {
    return transformCoordinatesFromFloatsToRef(
      vector.x,
      vector.y,
      vector.z,
      transformation,
      result
    )
  }

  /**
   * Sets the given vector "result" coordinates with the result of the transformation by the given matrix of the given floats (x, y, z)
   * This method computes tranformed coordinates only, not transformed direction vectors
   * @param x - define the x coordinate of the source vector
   * @param y - define the y coordinate of the source vector
   * @param z - define the z coordinate of the source vector
   * @param transformation - defines the transformation matrix
   * @param result - defines the Vector3 where to store the result
   */
  export function transformCoordinatesFromFloatsToRef(
    x: number,
    y: number,
    z: number,
    transformation: Matrix.ReadonlyMatrix,
    result: MutableVector3
  ): void {
    const m = transformation._m
    const rx = x * m[0] + y * m[4] + z * m[8] + m[12]
    const ry = x * m[1] + y * m[5] + z * m[9] + m[13]
    const rz = x * m[2] + y * m[6] + z * m[10] + m[14]
    const rw = 1 / (x * m[3] + y * m[7] + z * m[11] + m[15])

    result.x = rx * rw
    result.y = ry * rw
    result.z = rz * rw
  }

  /**
   * Returns a new Vector3 set with the result of the normal transformation by the given matrix of the given vector
   * This methods computes transformed normalized direction vectors only (ie. it does not apply translation)
   * @param vector - defines the Vector3 to transform
   * @param transformation - defines the transformation matrix
   * @returns the new Vector3
   */
  export function transformNormal(
    vector: ReadonlyVector3,
    transformation: Matrix.ReadonlyMatrix
  ): MutableVector3 {
    const result = Zero()
    transformNormalToRef(vector, transformation, result)
    return result
  }

  /**
   * Sets the given vector "result" with the result of the normal transformation by the given matrix of the given vector
   * This methods computes transformed normalized direction vectors only (ie. it does not apply translation)
   * @param vector - defines the Vector3 to transform
   * @param transformation - defines the transformation matrix
   * @param result - defines the Vector3 where to store the result
   */
  export function transformNormalToRef(
    vector: ReadonlyVector3,
    transformation: Matrix.ReadonlyMatrix,
    result: MutableVector3
  ): void {
    transformNormalFromFloatsToRef(
      vector.x,
      vector.y,
      vector.z,
      transformation,
      result
    )
  }

  /**
   * Sets the given vector "result" with the result of the normal transformation by the given matrix of the given floats (x, y, z)
   * This methods computes transformed normalized direction vectors only (ie. it does not apply translation)
   * @param x - define the x coordinate of the source vector
   * @param y - define the y coordinate of the source vector
   * @param z - define the z coordinate of the source vector
   * @param transformation - defines the transformation matrix
   * @param result - defines the Vector3 where to store the result
   */
  export function transformNormalFromFloatsToRef(
    x: number,
    y: number,
    z: number,
    transformation: Matrix.ReadonlyMatrix,
    result: MutableVector3
  ): void {
    const m = transformation._m
    result.x = x * m[0] + y * m[4] + z * m[8]
    result.y = x * m[1] + y * m[5] + z * m[9]
    result.z = x * m[2] + y * m[6] + z * m[10]
  }

  /**
   * Returns a new Vector3 located for "amount" on the CatmullRom interpolation spline defined by the vectors "value1", "value2", "value3", "value4"
   * @param value1 - defines the first control point
   * @param value2 - defines the second control point
   * @param value3 - defines the third control point
   * @param value4 - defines the fourth control point
   * @param amount - defines the amount on the spline to use
   * @returns the new Vector3
   */
  export function catmullRom(
    value1: ReadonlyVector3,
    value2: ReadonlyVector3,
    value3: ReadonlyVector3,
    value4: ReadonlyVector3,
    amount: number
  ): MutableVector3 {
    const squared = amount * amount
    const cubed = amount * squared

    const x =
      0.5 *
      (2.0 * value2.x +
        (-value1.x + value3.x) * amount +
        (2.0 * value1.x - 5.0 * value2.x + 4.0 * value3.x - value4.x) *
          squared +
        (-value1.x + 3.0 * value2.x - 3.0 * value3.x + value4.x) * cubed)

    const y =
      0.5 *
      (2.0 * value2.y +
        (-value1.y + value3.y) * amount +
        (2.0 * value1.y - 5.0 * value2.y + 4.0 * value3.y - value4.y) *
          squared +
        (-value1.y + 3.0 * value2.y - 3.0 * value3.y + value4.y) * cubed)

    const z =
      0.5 *
      (2.0 * value2.z +
        (-value1.z + value3.z) * amount +
        (2.0 * value1.z - 5.0 * value2.z + 4.0 * value3.z - value4.z) *
          squared +
        (-value1.z + 3.0 * value2.z - 3.0 * value3.z + value4.z) * cubed)

    return create(x, y, z)
  }

  /**
   * Returns a new Vector3 set with the coordinates of "value", if the vector "value" is in the cube defined by the vectors "min" and "max"
   * If a coordinate value of "value" is lower than one of the "min" coordinate, then this "value" coordinate is set with the "min" one
   * If a coordinate value of "value" is greater than one of the "max" coordinate, then this "value" coordinate is set with the "max" one
   * @param value - defines the current value
   * @param min - defines the lower range value
   * @param max - defines the upper range value
   * @returns the new Vector3
   */
  export function clamp(
    value: ReadonlyVector3,
    min: ReadonlyVector3,
    max: ReadonlyVector3
  ): MutableVector3 {
    const v = create()
    clampToRef(value, min, max, v)
    return v
  }
  /**
   * Sets the given vector "result" with the coordinates of "value", if the vector "value" is in the cube defined by the vectors "min" and "max"
   * If a coordinate value of "value" is lower than one of the "min" coordinate, then this "value" coordinate is set with the "min" one
   * If a coordinate value of "value" is greater than one of the "max" coordinate, then this "value" coordinate is set with the "max" one
   * @param value - defines the current value
   * @param min - defines the lower range value
   * @param max - defines the upper range value
   * @param result - defines the Vector3 where to store the result
   */
  export function clampToRef(
    value: ReadonlyVector3,
    min: ReadonlyVector3,
    max: ReadonlyVector3,
    result: MutableVector3
  ): void {
    let x = value.x
    x = x > max.x ? max.x : x
    x = x < min.x ? min.x : x

    let y = value.y
    y = y > max.y ? max.y : y
    y = y < min.y ? min.y : y

    let z = value.z
    z = z > max.z ? max.z : z
    z = z < min.z ? min.z : z

    copyFromFloats(x, y, z, result)
  }

  /**
   * Returns a new Vector3 located for "amount" (float) on the Hermite interpolation spline defined by the vectors "value1", "tangent1", "value2", "tangent2"
   * @param value1 - defines the first control point
   * @param tangent1 - defines the first tangent vector
   * @param value2 - defines the second control point
   * @param tangent2 - defines the second tangent vector
   * @param amount - defines the amount on the interpolation spline (between 0 and 1)
   * @returns the new Vector3
   */
  export function hermite(
    value1: ReadonlyVector3,
    tangent1: ReadonlyVector3,
    value2: ReadonlyVector3,
    tangent2: ReadonlyVector3,
    amount: number
  ): MutableVector3 {
    const squared = amount * amount
    const cubed = amount * squared
    const part1 = 2.0 * cubed - 3.0 * squared + 1.0
    const part2 = -2.0 * cubed + 3.0 * squared
    const part3 = cubed - 2.0 * squared + amount
    const part4 = cubed - squared

    const x =
      value1.x * part1 +
      value2.x * part2 +
      tangent1.x * part3 +
      tangent2.x * part4
    const y =
      value1.y * part1 +
      value2.y * part2 +
      tangent1.y * part3 +
      tangent2.y * part4
    const z =
      value1.z * part1 +
      value2.z * part2 +
      tangent1.z * part3 +
      tangent2.z * part4
    return create(x, y, z)
  }

  /**
   * Gets the minimal coordinate values between two Vector3
   * @param left - defines the first operand
   * @param right - defines the second operand
   * @returns the new Vector3
   */
  export function minimize(
    left: ReadonlyVector3,
    right: ReadonlyVector3
  ): MutableVector3 {
    const min = create()
    minimizeInPlaceFromFloatsToRef(right, left.x, left.y, left.z, min)
    return min
  }

  /**
   * Gets the maximal coordinate values between two Vector3
   * @param left - defines the first operand
   * @param right - defines the second operand
   * @returns the new Vector3
   */
  export function maximize(
    left: MutableVector3,
    right: MutableVector3
  ): MutableVector3 {
    const max = create()
    maximizeInPlaceFromFloatsToRef(left, right.x, right.y, right.z, max)
    return max
  }

  /**
   * Returns the distance between the vectors "value1" and "value2"
   * @param value1 - defines the first operand
   * @param value2 - defines the second operand
   * @returns the distance
   */
  export function distance(
    value1: ReadonlyVector3,
    value2: ReadonlyVector3
  ): number {
    return Math.sqrt(distanceSquared(value1, value2))
  }

  /**
   * Returns the squared distance between the vectors "value1" and "value2"
   * @param value1 - defines the first operand
   * @param value2 - defines the second operand
   * @returns the squared distance
   */
  export function distanceSquared(
    value1: ReadonlyVector3,
    value2: ReadonlyVector3
  ): number {
    const x = value1.x - value2.x
    const y = value1.y - value2.y
    const z = value1.z - value2.z

    return x * x + y * y + z * z
  }

  /**
   * Returns a new Vector3 located at the center between "value1" and "value2"
   * @param value1 - defines the first operand
   * @param value2 - defines the second operand
   * @returns the new Vector3
   */
  export function center(
    value1: ReadonlyVector3,
    value2: ReadonlyVector3
  ): MutableVector3 {
    const center = add(value1, value2)
    scaleToRef(center, 0.5, center)
    return center
  }

  /**
   * Given three orthogonal normalized left-handed oriented Vector3 axis in space (target system),
   * RotationFromAxis() returns the rotation Euler angles (ex : rotation.x, rotation.y, rotation.z) to apply
   * to something in order to rotate it from its local system to the given target system
   * Note: axis1, axis2 and axis3 are normalized during this operation
   * @param axis1 - defines the first axis
   * @param axis2 - defines the second axis
   * @param axis3 - defines the third axis
   * @returns a new Vector3
   */
  export function rotationFromAxis(
    axis1: MutableVector3,
    axis2: MutableVector3,
    axis3: MutableVector3
  ): MutableVector3 {
    const rotation = Zero()
    rotationFromAxisToRef(axis1, axis2, axis3, rotation)
    return rotation
  }

  /**
   * The same than RotationFromAxis but updates the given ref Vector3 parameter instead of returning a new Vector3
   * @param axis1 - defines the first axis
   * @param axis2 - defines the second axis
   * @param axis3 - defines the third axis
   * @param ref - defines the Vector3 where to store the result
   */
  export function rotationFromAxisToRef(
    axis1: MutableVector3,
    axis2: MutableVector3,
    axis3: MutableVector3,
    result: MutableVector3
  ): void {
    const quat = Quaternion.create()
    Quaternion.fromAxisToRotationQuaternionToRef(axis1, axis2, axis3, quat)
    copyFrom(Quaternion.toEulerAngles(quat), result)
  }

  /**
   * Creates a string representation of the Vector3
   * @returns a string with the Vector3 coordinates.
   */
  export function toString(vector: ReadonlyVector3): string {
    return `(${vector.x}, ${vector.y}, ${vector.z})`
  }

  /**
   * Creates the Vector3 hash code
   * @returns a number which tends to be unique between Vector3 instances
   */
  export function getHashCode(vector: ReadonlyVector3): number {
    let hash = vector.x || 0
    hash = (hash * 397) ^ (vector.y || 0)
    hash = (hash * 397) ^ (vector.z || 0)
    return hash
  }

  /**
   * Returns true if the vector1 and the vector2 coordinates are strictly equal
   * @param vector1 - defines the first operand
   * @param vector2 - defines the second operand
   * @returns true if both vectors are equals
   */
  export function equals(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3
  ): boolean {
    return (
      vector1.x === vector2.x &&
      vector1.y === vector2.y &&
      vector1.z === vector2.z
    )
  }

  /**
   * Returns true if the current Vector3 and the given vector coordinates are distant less than epsilon
   * @param otherVector - defines the second operand
   * @param epsilon - defines the minimal distance to define values as equals
   * @returns true if both vectors are distant less than epsilon
   */
  export function equalsWithEpsilon(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3,
    epsilon: number = Epsilon
  ): boolean {
    return (
      Scalar.withinEpsilon(vector1.x, vector2.x, epsilon) &&
      Scalar.withinEpsilon(vector1.y, vector2.y, epsilon) &&
      Scalar.withinEpsilon(vector1.z, vector2.z, epsilon)
    )
  }

  /**
   * Returns true if the current Vector3 coordinates equals the given floats
   * @param x - defines the x coordinate of the operand
   * @param y - defines the y coordinate of the operand
   * @param z - defines the z coordinate of the operand
   * @returns true if both vectors are equals
   */
  export function equalsToFloats(
    vector: ReadonlyVector3,
    x: number,
    y: number,
    z: number
  ): boolean {
    return vector.x === x && vector.y === y && vector.z === z
  }

  /**
   * Returns a new Vector3, result of the multiplication of vector1 by the vector2
   * @param vector1 - defines the first operand
   * @param vector2 - defines the second operand
   * @returns the new Vector3
   */
  export function multiply(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3
  ): MutableVector3 {
    const result = create()
    multiplyToRef(vector1, vector2, result)
    return result
  }

  /**
   * Multiplies the current Vector3 by the given one and stores the result in the given vector "result"
   * @param otherVector - defines the second operand
   * @param result - defines the Vector3 object where to store the result
   * @returns the current Vector3
   */
  export function multiplyToRef(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3,
    result: MutableVector3
  ): void {
    result.x = vector1.x * vector2.x
    result.y = vector1.y * vector2.y
    result.z = vector1.z * vector2.z
  }

  /**
   * Returns a new Vector3 set with the result of the mulliplication of the current Vector3 coordinates by the given floats
   * @param x - defines the x coordinate of the operand
   * @param y - defines the y coordinate of the operand
   * @param z - defines the z coordinate of the operand
   * @returns the new Vector3
   */
  export function multiplyByFloatsToRef(
    vector1: ReadonlyVector3,
    x: number,
    y: number,
    z: number,
    result: MutableVector3
  ): void {
    result.x = vector1.x * x
    result.y = vector1.y * y
    result.z = vector1.z * z
  }

  /**
   * Returns a new Vector3 set with the result of the mulliplication of the current Vector3 coordinates by the given floats
   * @param x - defines the x coordinate of the operand
   * @param y - defines the y coordinate of the operand
   * @param z - defines the z coordinate of the operand
   * @returns the new Vector3
   */
  export function multiplyByFloats(
    vector1: ReadonlyVector3,
    x: number,
    y: number,
    z: number
  ): MutableVector3 {
    const result = create()
    multiplyByFloatsToRef(vector1, x, y, z, result)
    return result
  }

  /**
   * Returns a new Vector3 set with the result of the division of the current Vector3 coordinates by the given ones
   * @param otherVector - defines the second operand
   * @returns the new Vector3
   */
  export function divide(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3
  ): MutableVector3 {
    return {
      x: vector1.x / vector2.x,
      y: vector1.y / vector2.y,
      z: vector1.z / vector2.z
    }
  }

  /**
   * Divides the current Vector3 coordinates by the given ones and stores the result in the given vector "result"
   * @param otherVector - defines the second operand
   * @param result - defines the Vector3 object where to store the result
   * @returns the current Vector3
   */
  export function divideToRef(
    vector1: ReadonlyVector3,
    vector2: ReadonlyVector3,
    result: MutableVector3
  ): void {
    result.x = vector1.x / vector2.x
    result.y = vector1.y / vector2.y
    result.z = vector1.z / vector2.z
  }

  /**
   * Set result Vector3 with the maximal coordinate values between vector1 and the given coordinates
   * @param x - defines the x coordinate of the operand
   * @param y - defines the y coordinate of the operand
   * @param z - defines the z coordinate of the operand
   * @param result - the set Vector3
   */
  export function maximizeInPlaceFromFloatsToRef(
    vector1: ReadonlyVector3,
    x: number,
    y: number,
    z: number,
    result: MutableVector3
  ): void {
    if (x > vector1.x) {
      result.x = x
    } else {
      result.x = vector1.x
    }
    if (y > vector1.y) {
      result.y = y
    } else {
      result.y = vector1.y
    }
    if (z > vector1.z) {
      result.z = z
    } else {
      result.z = vector1.z
    }
  }

  /**
   * Set result Vector3 with the minimal coordinate values between vector1 and the given coordinates
   * @param x - defines the x coordinate of the operand
   * @param y - defines the y coordinate of the operand
   * @param z - defines the z coordinate of the operand
   * @param result - the set Vector3
   */
  export function minimizeInPlaceFromFloatsToRef(
    vector1: ReadonlyVector3,
    x: number,
    y: number,
    z: number,
    result: MutableVector3
  ): void {
    if (x < vector1.x) {
      result.x = x
    } else {
      result.x = vector1.x
    }
    if (y < vector1.y) {
      result.y = y
    } else {
      result.y = vector1.y
    }
    if (z < vector1.z) {
      result.z = z
    } else {
      result.z = vector1.z
    }
  }

  /**
   * Gets a new Vector3 from vector1 floored values
   * @returns a new Vector3
   */
  export function floor(vector1: ReadonlyVector3): MutableVector3 {
    return create(
      Math.floor(vector1.x),
      Math.floor(vector1.y),
      Math.floor(vector1.z)
    )
  }

  /**
   * Gets a new Vector3 from vector1 floored values
   * @returns a new Vector3
   */
  export function fract(vector1: ReadonlyVector3): MutableVector3 {
    return create(
      vector1.x - Math.floor(vector1.x),
      vector1.y - Math.floor(vector1.y),
      vector1.z - Math.floor(vector1.z)
    )
  }

  /**
   * Returns a new Vector3 set to (0.0, 0.0, 0.0)
   * @returns a new empty Vector3
   */
  export function Zero(): MutableVector3 {
    return create(0.0, 0.0, 0.0)
  }
  /**
   * Returns a new Vector3 set to (1.0, 1.0, 1.0)
   * @returns a new unit Vector3
   */
  export function One(): MutableVector3 {
    return create(1.0, 1.0, 1.0)
  }
  /**
   * Returns a new Vector3 set tolengthSquared (0.0, 1.0, 0.0)
   * @returns a new up Vector3
   */
  export function Up(): MutableVector3 {
    return create(0.0, 1.0, 0.0)
  }
  /**
   * Returns a new Vector3 set to (0.0, -1.0, 0.0)
   * @returns a new down Vector3
   */
  export function Down(): MutableVector3 {
    return create(0.0, -1.0, 0.0)
  }
  /**
   * Returns a new Vector3 set to (0.0, 0.0, 1.0)
   * @returns a new forward Vector3
   */
  export function Forward(): MutableVector3 {
    return create(0.0, 0.0, 1.0)
  }
  /**
   * Returns a new Vector3 set to (0.0, 0.0, -1.0)
   * @returns a new forward Vector3
   */
  export function Backward(): MutableVector3 {
    return create(0.0, 0.0, -1.0)
  }
  /**
   * Returns a new Vector3 set to (1.0, 0.0, 0.0)
   * @returns a new right Vector3
   */
  export function Right(): MutableVector3 {
    return create(1.0, 0.0, 0.0)
  }
  /**
   * Returns a new Vector3 set to (-1.0, 0.0, 0.0)
   * @returns a new left Vector3
   */
  export function Left(): MutableVector3 {
    return create(-1.0, 0.0, 0.0)
  }

  /**
   * Returns a new random Vector3
   * @returns a random Vector3
   */
  export function Random(): MutableVector3 {
    return create(Math.random(), Math.random(), Math.random())
  }
}
