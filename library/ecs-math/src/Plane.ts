import { Vector3 } from './Vector3'
import { Matrix } from './Matrix'

/**
 * Represens a plane by the equation ax + by + cz + d = 0
 * @public
 */
export namespace Plane {
  export type MutablePlane = {
    /**
     * Normal of the plane (a,b,c)
     */
    normal: Vector3.MutableVector3
    /**
     * d component of the plane
     */
    d: number
  }

  export type ReadonlyPlane = {
    /**
     * Normal of the plane (a,b,c)
     */
    normal: Vector3.ReadonlyVector3
    /**
     * d component of the plane
     */
    d: number
  }

  /**
   * Creates a Plane object according to the given floats a, b, c, d and the plane equation : ax + by + cz + d = 0
   * @param a - a component of the plane
   * @param b - b component of the plane
   * @param c - c component of the plane
   * @param d - d component of the plane
   */
  export function create(a: number, b: number, c: number, d: number) {
    return {
      normal: Vector3.create(a, b, c),
      d: d
    }
  }

  // Statics
  /**
   * Creates a plane from an  array
   * @param array - the array to create a plane from
   * @returns a new Plane from the given array.
   */
  export function fromArray(array: number[]): MutablePlane {
    return create(array[0], array[1], array[2], array[3])
  }
  /**
   * Creates a plane from three points
   * @param point1 - point used to create the plane
   * @param point2 - point used to create the plane
   * @param point3 - point used to create the plane
   * @returns a new Plane defined by the three given points.
   */
  export function fromPoints(
    _point1: Vector3.ReadonlyVector3,
    _point2: Vector3.ReadonlyVector3,
    _point3: Vector3.ReadonlyVector3
  ): MutablePlane {
    const result = create(0.0, 0.0, 0.0, 0.0)
    // TODO
    // result.copyFromPoints(point1, point2, point3)
    return result
  }
  /**
   * Creates a plane from an origin point and a normal
   * @param origin - origin of the plane to be constructed
   * @param normal - normal of the plane to be constructed
   * @returns a new Plane the normal vector to this plane at the given origin point.
   * Note : the vector "normal" is updated because normalized.
   */
  export function romPositionAndNormal(
    origin: Vector3.ReadonlyVector3,
    normal: Vector3.ReadonlyVector3
  ): MutablePlane {
    const result = create(0.0, 0.0, 0.0, 0.0)
    result.normal = Vector3.normalize(normal)
    result.d = -(
      normal.x * origin.x +
      normal.y * origin.y +
      normal.z * origin.z
    )
    return result
  }

  /**
   * Calculates the distance from a plane and a point
   * @param origin - origin of the plane to be constructed
   * @param normal - normal of the plane to be constructed
   * @param point - point to calculate distance to
   * @returns the signed distance between the plane defined by the normal vector at the "origin"" point and the given other point.
   */
  export function signedDistanceToPlaneFromPositionAndNormal(
    origin: Vector3.ReadonlyVector3,
    normal: Vector3.ReadonlyVector3,
    point: Vector3.ReadonlyVector3
  ): number {
    const d = -(normal.x * origin.x + normal.y * origin.y + normal.z * origin.z)
    return Vector3.dot(point, normal) + d
  }

  /**
   * @returns the plane coordinates as a new array of 4 elements [a, b, c, d].
   */
  export function asArray(plane: ReadonlyPlane): number[] {
    return [plane.normal.x, plane.normal.y, plane.normal.z, plane.d]
  }

  // Methods
  /**
   * @returns a new plane copied from the current Plane.
   */
  export function clone(plane: ReadonlyPlane): MutablePlane {
    return create(plane.normal.x, plane.normal.y, plane.normal.z, plane.d)
  }

  /**
   * @returns the Plane hash code.
   */
  export function getHashCode(_plane: ReadonlyPlane): number {
    // TODO
    // let hash = plane.normal.getHashCode()
    // hash = (hash * 397) ^ (plane.d || 0)
    // return hash
    return 0
  }
  /**
   * Normalize the current Plane in place.
   * @returns the updated Plane.
   */
  export function normalize(plane: ReadonlyPlane): MutablePlane {
    const result = create(0, 0, 0, 0)
    const norm = Math.sqrt(
      plane.normal.x * plane.normal.x +
        plane.normal.y * plane.normal.y +
        plane.normal.z * plane.normal.z
    )
    let magnitude = 0.0

    if (norm !== 0) {
      magnitude = 1.0 / norm
    }
    result.normal.x = plane.normal.x * magnitude
    result.normal.y = plane.normal.y * magnitude
    result.normal.z = plane.normal.z * magnitude
    result.d *= magnitude
    return plane
  }
  /**
   * Applies a transformation the plane and returns the result
   * @param transformation - the transformation matrix to be applied to the plane
   * @returns a new Plane as the result of the transformation of the current Plane by the given matrix.
   */
  export function transform(
    plane: ReadonlyPlane,
    transformation: Matrix.ReadonlyMatrix
  ): MutablePlane {
    const transposedMatrix = Matrix.create()
    Matrix.transposeToRef(transformation, transposedMatrix)
    const m = transposedMatrix._m
    const x = plane.normal.x
    const y = plane.normal.y
    const z = plane.normal.z
    const d = plane.d

    const normalX = x * m[0] + y * m[1] + z * m[2] + d * m[3]
    const normalY = x * m[4] + y * m[5] + z * m[6] + d * m[7]
    const normalZ = x * m[8] + y * m[9] + z * m[10] + d * m[11]
    const finalD = x * m[12] + y * m[13] + z * m[14] + d * m[15]

    return create(normalX, normalY, normalZ, finalD)
  }

  /**
   * Calcualtte the dot product between the point and the plane normal
   * @param point - point to calculate the dot product with
   * @returns the dot product (float) of the point coordinates and the plane normal.
   */
  export function dotCoordinate(
    plane: ReadonlyPlane,
    point: Vector3.ReadonlyVector3
  ): number {
    return (
      plane.normal.x * point.x +
      plane.normal.y * point.y +
      plane.normal.z * point.z +
      plane.d
    )
  }

  /**
   * Updates the current Plane from the plane defined by the three given points.
   * @param point1 - one of the points used to contruct the plane
   * @param point2 - one of the points used to contruct the plane
   * @param point3 - one of the points used to contruct the plane
   * @returns the updated Plane.
   */
  export function copyFromPoints(
    point1: Vector3.ReadonlyVector3,
    point2: Vector3.ReadonlyVector3,
    point3: Vector3.ReadonlyVector3
  ): MutablePlane {
    const x1 = point2.x - point1.x
    const y1 = point2.y - point1.y
    const z1 = point2.z - point1.z
    const x2 = point3.x - point1.x
    const y2 = point3.y - point1.y
    const z2 = point3.z - point1.z
    const yz = y1 * z2 - z1 * y2
    const xz = z1 * x2 - x1 * z2
    const xy = x1 * y2 - y1 * x2
    const pyth = Math.sqrt(yz * yz + xz * xz + xy * xy)
    let invPyth

    if (pyth !== 0) {
      invPyth = 1.0 / pyth
    } else {
      invPyth = 0.0
    }

    const normal = Vector3.create(yz * invPyth, xz * invPyth, xy * invPyth)
    return {
      normal,
      d: -(normal.x * point1.x + normal.y * point1.y + normal.z * point1.z)
    }
  }

  /**
   * Checks if the plane is facing a given direction
   * @param direction - the direction to check if the plane is facing
   * @param epsilon - value the dot product is compared against (returns true if dot &lt;= epsilon)
   * @returns True is the vector "direction"  is the same side than the plane normal.
   */
  export function isFrontFacingTo(
    plane: ReadonlyPlane,
    direction: Vector3.ReadonlyVector3,
    epsilon: number
  ): boolean {
    const dot = Vector3.dot(plane.normal, direction)
    return dot <= epsilon
  }

  /**
   * Calculates the distance to a point
   * @param point - point to calculate distance to
   * @returns the signed distance (float) from the given point to the Plane.
   */
  export function signedDistanceTo(
    plane: ReadonlyPlane,
    point: Vector3.ReadonlyVector3
  ): number {
    return Vector3.dot(point, plane.normal) + plane.d
  }
}
