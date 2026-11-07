import { FloatArray, ToLinearSpace, ToGammaSpace } from './types'
import { Color4 } from './Color4'
import { Scalar } from './Scalar'

/**
 * @public
 * Color3 is a type and a namespace.
 * - The namespace contains all types and functions to operates with Color3
 * - The type Color3 is an alias to Color3.ReadonlyColor3
 * ```
 *
 * // Namespace usage example
 * Color3.add(blue, red) // sum component by component resulting pink
 *
 * // Type usage example
 * const readonlyBlue: Color3 = Color3.Blue()
 * readonlyBlue.r = 0.1 // this FAILS
 *
 * // For mutable usage, use `Color3.Mutable`
 * const blue: Color3.Mutable = Color3.Blue()
 * blue.r = 0.1 // this WORKS
 * ```
 */
export type Color3 = Color3.ReadonlyColor3

/**
 * @public
 * Color3 is a type and a namespace.
 * ```
 * // The namespace contains all types and functions to operates with Color3
 * Color3.add(blue, red) // sum component by component resulting pink
 * // The type Color3 is an alias to Color3.ReadonlyColor3
 * const readonlyBlue: Color3 = Color3.Blue()
 * readonlyBlue.r = 0.1 // this FAILS
 *
 * // For mutable usage, use `Color3.Mutable`
 * const blue: Color3.Mutable = Color3.Blue()
 * blue.r = 0.1 // this WORKS
 * ```
 */
export namespace Color3 {
  /**
   * @public
   * For external use, type with `Color3`, e.g. `const blackColor: Color3 = Color3.Black()`.
   * For mutable typing, use `Color3.Mutable`, e.g. `const redColor: Color3.Mutable = Color3.Red()`.
   */
  export type ReadonlyColor3 = {
    readonly r: number
    readonly g: number
    readonly b: number
  }

  /**
   * @public
   * For external usage, type with `Color3`, e.g. `const blackColor: Color3 = Color3.Black()`.
   * For mutable typing, use `Color3.Mutable`, e.g. `const redColor: Color3.Mutable = Color3.Red()`.
   */
  export type MutableColor3 = {
    r: number
    g: number
    b: number
  }

  /**
   * @public
   * Type with `Color3` for readonly usage, e.g. `const blackColor: Color3 = Color3.Black()`.
   * For mutable, use `Color3.Mutable`, e.g. `const redColor: Color3.Mutable = Color3.Red()`.
   */
  export type Mutable = MutableColor3

  /**
   * Creates Color3 object from red, green, blue values, all between 0 and 1
   * @param r - defines the red component (between 0 and 1, default is 0)
   * @param g - defines the green component (between 0 and 1, default is 0)
   * @param b - defines the blue component (between 0 and 1, default is 0)
   */
  export function create(
    /**
     * Defines the red component (between 0 and 1, default is 0)
     */
    r: number = 0,
    /**
     * Defines the green component (between 0 and 1, default is 0)
     */
    g: number = 0,
    /**
     * Defines the blue component (between 0 and 1, default is 0)
     */
    b: number = 0
  ) {
    return { r, g, b }
  }

  /**
   * Creates a Vector3 from the string containing valid hexadecimal values
   * @param hex - defines a string containing valid hexadecimal values
   * @returns a new Vector3
   */
  export function fromHexString(hex: string): MutableColor3 {
    if (hex.substring(0, 1) !== '#' || hex.length !== 7) {
      return create(0, 0, 0)
    }

    const r = parseInt(hex.substring(1, 3), 16)
    const g = parseInt(hex.substring(3, 5), 16)
    const b = parseInt(hex.substring(5, 7), 16)

    return fromInts(r, g, b)
  }

  /**
   * Creates a new Vector3 from the starting index of the given array
   * @param array - defines the source array
   * @param offset - defines an offset in the source array
   * @returns a new Vector3
   */
  export function fromArray(
    array: ArrayLike<number>,
    offset: number = 0
  ): MutableColor3 {
    return create(array[offset], array[offset + 1], array[offset + 2])
  }

  /**
   * Creates a Vector3 from integer values (less than 256)
   * @param r - defines the red component to read from (value between 0 and 255)
   * @param g - defines the green component to read from (value between 0 and 255)
   * @param b - defines the blue component to read from (value between 0 and 255)
   * @returns a new Vector3
   */
  export function fromInts(r: number, g: number, b: number): MutableColor3 {
    return create(r / 255.0, g / 255.0, b / 255.0)
  }

  /**
   * Creates a Vector3 with values linearly interpolated of "amount" between the start Color3 and the end Color3
   * @param start - defines the start Color3 value
   * @param end - defines the end Color3 value
   * @param amount - defines the gradient value between start and end
   * @returns a new Vector3
   */
  export function lerp(
    start: ReadonlyColor3,
    end: ReadonlyColor3,
    amount: number
  ): MutableColor3 {
    const result = create(0.0, 0.0, 0.0)
    Color3.lerpToRef(start, end, amount, result)
    return result
  }

  /**
   * Creates a Vector3 with values linearly interpolated of "amount" between the start Color3 and the end Color3
   * @param left - defines the start value
   * @param right - defines the end value
   * @param amount - defines the gradient factor
   * @param result - defines the Color3 object where to store the result
   */
  export function lerpToRef(
    left: ReadonlyColor3,
    right: ReadonlyColor3,
    amount: number,
    result: MutableColor3
  ): void {
    result.r = left.r + (right.r - left.r) * amount
    result.g = left.g + (right.g - left.g) * amount
    result.b = left.b + (right.b - left.b) * amount
  }

  /**
   * Returns a Color3 value containing a red color
   * @returns a new Vector3
   */
  export function Red(): MutableColor3 {
    return create(1, 0, 0)
  }
  /**
   * Returns a Color3 value containing a green color
   * @returns a new Vector3
   */
  export function Green(): MutableColor3 {
    return create(0, 1, 0)
  }
  /**
   * Returns a Color3 value containing a blue color
   * @returns a new Vector3
   */
  export function Blue(): MutableColor3 {
    return create(0, 0, 1)
  }
  /**
   * Returns a Color3 value containing a black color
   * @returns a new Vector3
   */
  export function Black(): MutableColor3 {
    return create(0, 0, 0)
  }
  /**
   * Returns a Color3 value containing a white color
   * @returns a new Vector3
   */
  export function White(): MutableColor3 {
    return create(1, 1, 1)
  }
  /**
   * Returns a Color3 value containing a purple color
   * @returns a new Vector3
   */
  export function Purple(): MutableColor3 {
    return create(0.5, 0, 0.5)
  }
  /**
   * Returns a Color3 value containing a magenta color
   * @returns a new Vector3
   */
  export function Magenta(): MutableColor3 {
    return create(1, 0, 1)
  }
  /**
   * Returns a Color3 value containing a yellow color
   * @returns a new Vector3
   */
  export function Yellow(): MutableColor3 {
    return create(1, 1, 0)
  }
  /**
   * Returns a Color3 value containing a gray color
   * @returns a new Vector3
   */
  export function Gray(): MutableColor3 {
    return create(0.5, 0.5, 0.5)
  }
  /**
   * Returns a Color3 value containing a teal color
   * @returns a new Vector3
   */
  export function Teal(): MutableColor3 {
    return create(0, 1.0, 1.0)
  }
  /**
   * Returns a Color3 value containing a random color
   * @returns a new Vector3
   */
  export function Random(): MutableColor3 {
    return create(Math.random(), Math.random(), Math.random())
  }

  /**
   * Creates a string with the Color3 current values
   * @returns the string representation of the Color3 object
   */
  export function toString(value: ReadonlyColor3): string {
    return '{R: ' + value.r + ' G:' + value.g + ' B:' + value.b + '}'
  }

  /**
   * Compute the Color3 hash code
   * @returns an unique number that can be used to hash Color3 objects
   */
  export function getHashCode(value: ReadonlyColor3): number {
    let hash = value.r || 0
    hash = (hash * 397) ^ (value.g || 0)
    hash = (hash * 397) ^ (value.b || 0)
    return hash
  }

  // Operators

  /**
   * Stores in the given array from the given starting index the red, green, blue values as successive elements
   * @param array - defines the array where to store the r,g,b components
   * @param index - defines an optional index in the target array to define where to start storing values
   *
   */
  export function toArray(
    value: ReadonlyColor3,
    array: FloatArray,
    index: number = 0
  ): void {
    array[index] = value.r
    array[index + 1] = value.g
    array[index + 2] = value.b
  }

  /**
   * Returns a new Color4 object from the current Color3 and the given alpha
   * @param alpha - defines the alpha component on the new Color4 object (default is 1)
   * @returns a new Color4 object
   */
  export function toColor4(
    value: ReadonlyColor3,
    alpha: number = 1
  ): Color4.MutableColor4 {
    return Color4.create(value.r, value.g, value.b, alpha)
  }

  /**
   * Returns a new array populated with 3 numeric elements : red, green and blue values
   * @returns the new array
   */
  export function asArray(value: ReadonlyColor3): number[] {
    const result = new Array<number>()
    toArray(value, result, 0)
    return result
  }

  /**
   * Returns the luminance value
   * @returns a float value
   */
  export function toLuminance(value: ReadonlyColor3): number {
    return value.r * 0.3 + value.g * 0.59 + value.b * 0.11
  }

  /**
   * Multiply each Color3 rgb values by the given Color3 rgb values in Color3 object
   * @param otherColor - defines the second operand
   * @returns the create object
   */
  export function multiply(
    value: ReadonlyColor3,
    otherColor: ReadonlyColor3
  ): MutableColor3 {
    return create(
      value.r * otherColor.r,
      value.g * otherColor.g,
      value.b * otherColor.b
    )
  }

  /**
   * Multiply the rgb values of the Color3 and the given Color3 and stores the result in the object "result"
   * @param otherColor - defines the second operand
   * @param result - defines the Color3 object where to store the result
   * @returns the current Color3
   */
  export function multiplyToRef(
    value: ReadonlyColor3,
    otherColor: ReadonlyColor3,
    result: MutableColor3
  ): void {
    result.r = value.r * otherColor.r
    result.g = value.g * otherColor.g
    result.b = value.b * otherColor.b
  }

  /**
   * Determines equality between Color3 objects
   * @param otherColor - defines the second operand
   * @returns true if the rgb values are equal to the given ones
   */
  export function equals(
    value: ReadonlyColor3,
    otherColor: ReadonlyColor3
  ): boolean {
    return (
      otherColor &&
      value.r === otherColor.r &&
      value.g === otherColor.g &&
      value.b === otherColor.b
    )
  }

  /**
   * Determines equality between the current Color3 object and a set of r,b,g values
   * @param r - defines the red component to check
   * @param g - defines the green component to check
   * @param b - defines the blue component to check
   * @returns true if the rgb values are equal to the given ones
   */
  export function equalsFloats(
    value: ReadonlyColor3,
    r: number,
    g: number,
    b: number
  ): boolean {
    return value.r === r && value.g === g && value.b === b
  }

  /**
   * Multiplies in place each rgb value by scale
   * @param scale - defines the scaling factor
   * @returns the updated Color3
   */
  export function scale(value: ReadonlyColor3, scale: number): MutableColor3 {
    return create(value.r * scale, value.g * scale, value.b * scale)
  }

  /**
   * Multiplies the rgb values by scale and stores the result into "result"
   * @param scale - defines the scaling factor
   * @param result - defines the Color3 object where to store the result
   * @returns the unmodified current Color3
   */
  export function scaleToRef(
    value: ReadonlyColor3,
    scale: number,
    result: MutableColor3
  ): void {
    result.r = value.r * scale
    result.g = value.g * scale
    result.b = value.b * scale
  }

  /**
   * Scale the current Color3 values by a factor and add the result to a given Color3
   * @param scale - defines the scale factor
   * @param result - defines color to store the result into
   * @returns the unmodified current Color3
   */
  export function scaleAndAddToRef(
    value: ReadonlyColor3,
    scale: number,
    result: MutableColor3
  ): void {
    result.r += value.r * scale
    result.g += value.g * scale
    result.b += value.b * scale
  }

  /**
   * Clamps the rgb values by the min and max values and stores the result into "result"
   * @param min - defines minimum clamping value (default is 0)
   * @param max - defines maximum clamping value (default is 1)
   * @param result - defines color to store the result into
   * @returns the original Color3
   */
  export function clampToRef(
    value: ReadonlyColor3,
    min: number = 0,
    max: number = 1,
    result: MutableColor3
  ): void {
    result.r = Scalar.clamp(value.r, min, max)
    result.g = Scalar.clamp(value.g, min, max)
    result.b = Scalar.clamp(value.b, min, max)
  }

  /**
   * Clamps the rgb values by the min and max values and returns the result
   * @param min - defines minimum clamping value (default is 0)
   * @param max - defines maximum clamping value (default is 1)
   * @returns result
   */
  export function clamp(
    value: ReadonlyColor3,
    min: number = 0,
    max: number = 1
  ): MutableColor3 {
    const result: MutableColor3 = Color3.Black()
    clampToRef(value, min, max, result)
    return result
  }

  /**
   * Creates Color3 set with the added values of the current Color3 and of the given one
   * @param otherColor - defines the second operand
   * @returns the create
   */
  export function add(
    value: ReadonlyColor3,
    otherColor: ReadonlyColor3
  ): MutableColor3 {
    return create(
      value.r + otherColor.r,
      value.g + otherColor.g,
      value.b + otherColor.b
    )
  }

  /**
   * Stores the result of the addition of the current Color3 and given one rgb values into "result"
   * @param otherColor - defines the second operand
   * @param result - defines Color3 object to store the result into
   * @returns the unmodified current Color3
   */
  export function addToRef(
    value: ReadonlyColor3,
    otherColor: ReadonlyColor3,
    result: MutableColor3
  ): void {
    result.r = value.r + otherColor.r
    result.g = value.g + otherColor.g
    result.b = value.b + otherColor.b
  }

  /**
   * Returns Color3 set with the subtracted values of the given one from the current Color3
   * @param otherColor - defines the second operand
   * @returns the create
   */
  export function subtract(
    value: ReadonlyColor3,
    otherColor: ReadonlyColor3
  ): MutableColor3 {
    return create(
      value.r - otherColor.r,
      value.g - otherColor.g,
      value.b - otherColor.b
    )
  }

  /**
   * Stores the result of the subtraction of given one from the current Color3 rgb values into "result"
   * @param otherColor - defines the second operand
   * @param result - defines Color3 object to store the result into
   * @returns the unmodified current Color3
   */
  export function subtractToRef(
    value: ReadonlyColor3,
    otherColor: ReadonlyColor3,
    result: MutableColor3
  ): void {
    result.r = value.r - otherColor.r
    result.g = value.g - otherColor.g
    result.b = value.b - otherColor.b
  }

  /**
   * Copy the current object
   * @returns Color3 copied the current one
   */
  export function clone(value: ReadonlyColor3): MutableColor3 {
    return create(value.r, value.g, value.b)
  }

  /**
   * Copies the rgb values from the source in the current Color3
   * @param source - defines the source Color3 object
   * @returns the updated Color3 object
   */
  export function copyFrom(source: ReadonlyColor3, dest: MutableColor3): void {
    dest.r = source.r
    dest.g = source.g
    dest.b = source.b
  }
  /**
   * Updates the Color3 rgb values from the given floats
   * @param dest -
   * @param r - defines the red component to read from
   * @param g - defines the green component to read from
   * @param b - defines the blue component to read from
   * @returns
   */
  export function set(
    dest: MutableColor3,
    r: number,
    g: number,
    b: number
  ): void {
    dest.r = r
    dest.g = g
    dest.b = b
  }

  /**
   * Compute the Color3 hexadecimal code as a string
   * @returns a string containing the hexadecimal representation of the Color3 object
   */
  export function toHexString(value: ReadonlyColor3): string {
    const intR = (value.r * 255) | 0
    const intG = (value.g * 255) | 0
    const intB = (value.b * 255) | 0
    return '#' + Scalar.toHex(intR) + Scalar.toHex(intG) + Scalar.toHex(intB)
  }

  /**
   * Computes Color3 converted from the current one to linear space
   * @returns a new Vector3
   */
  export function toLinearSpace(value: ReadonlyColor3): MutableColor3 {
    const convertedColor = create()
    toLinearSpaceToRef(value, convertedColor)
    return convertedColor
  }

  /**
   * Converts the Color3 values to linear space and stores the result in "convertedColor"
   * @param convertedColor - defines the Color3 object where to store the linear space version
   * @returns the unmodified Color3
   */
  export function toLinearSpaceToRef(
    value: ReadonlyColor3,
    convertedColor: MutableColor3
  ): void {
    convertedColor.r = Math.pow(value.r, ToLinearSpace)
    convertedColor.g = Math.pow(value.g, ToLinearSpace)
    convertedColor.b = Math.pow(value.b, ToLinearSpace)
  }

  /**
   * Computes Color3 converted from the current one to gamma space
   * @returns a new Vector3
   */
  export function toGammaSpace(value: ReadonlyColor3): ReadonlyColor3 {
    const convertedColor = create()
    toGammaSpaceToRef(value, convertedColor)
    return convertedColor
  }

  /**
   * Converts the Color3 values to gamma space and stores the result in "convertedColor"
   * @param convertedColor - defines the Color3 object where to store the gamma space version
   * @returns the unmodified Color3
   */
  export function toGammaSpaceToRef(
    value: ReadonlyColor3,
    convertedColor: MutableColor3
  ): void {
    convertedColor.r = Math.pow(value.r, ToGammaSpace)
    convertedColor.g = Math.pow(value.g, ToGammaSpace)
    convertedColor.b = Math.pow(value.b, ToGammaSpace)
  }
}
