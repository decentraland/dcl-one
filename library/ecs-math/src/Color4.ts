import { Color3 } from './Color3'
import { Scalar } from './Scalar'
import { ToLinearSpace, ToGammaSpace } from './types'

/**
 * @public
 * Color4 is a type and a namespace.
 * - The namespace contains all types and functions to operates with Color4
 * - The type Color4 is an alias to Color4.ReadonlyColor4
 * ```
 *
 * // Namespace usage example
 * Color4.add(blue, red) // sum component by component resulting pink
 *
 * // Type usage example
 * const readonlyBlue: Color4 = Color4.Blue()
 * readonlyBlue.a = 0.1 // this FAILS
 *
 * // For mutable usage, use `Color4.Mutable`
 * const blue: Color4.Mutable = Color4.Blue()
 * blue.a = 0.1 // this WORKS
 * ```
 */
export type Color4 = Color4.ReadonlyColor4

/**
 * @public
 * Color4 is a type and a namespace.
 * ```
 * // The namespace contains all types and functions to operates with Color4
 * Color4.add(blue, red) // sum component by component resulting pink
 * // The type Color4 is an alias to Color4.ReadonlyColor4
 * const readonlyBlue: Color4 = Color4.Blue()
 * readonlyBlue.a = 0.1 // this FAILS
 *
 * // For mutable usage, use `Color4.Mutable`
 * const blue: Color4.Mutable = Color4.Blue()
 * blue.a = 0.1 // this WORKS
 * ```
 */
export namespace Color4 {
  /**
   * @public
   * For external use, type with `Color4`, e.g. `const blackColor: Color4 = Color4.Black()`.
   * For mutable typing, use `Color4.Mutable`, e.g. `const redColor: Color4.Mutable = Color4.Red()`.
   */
  export type ReadonlyColor4 = {
    readonly r: number
    readonly g: number
    readonly b: number
    readonly a: number
  }

  /**
   * @public
   * For external usage, type with `Color4`, e.g. `const blackColor: Color4 = Color4.Black()`.
   * For mutable typing, use `Color4.Mutable`, e.g. `const redColor: Color4.Mutable = Color4.Red()`.
   */
  export type MutableColor4 = {
    r: number
    g: number
    b: number
    a: number
  }

  /**
   * @public
   * Type with `Color4` for readonly usage, e.g. `const blackColor: Color4 = Color4.Black()`.
   * For mutable, use `Color4.Mutable`, e.g. `const redColor: Color4.Mutable = Color4.Red()`.
   */
  export type Mutable = MutableColor4

  /**
   * Creates create mutable Color4 from red, green, blue values, all between 0 and 1
   * @param r - defines the red component (between 0 and 1, default is 0)
   * @param g - defines the green component (between 0 and 1, default is 0)
   * @param b - defines the blue component (between 0 and 1, default is 0)
   * @param a - defines the alpha component (between 0 and 1, default is 1)
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
    b: number = 0,
    /**
     * Defines the alpha component (between 0 and 1, default is 1)
     */
    a: number = 1
  ): MutableColor4 {
    return { r, g, b, a }
  }

  // Statics

  /**
   * Creates a Color4 from the string containing valid hexadecimal values
   * @param hex - defines a string containing valid hexadecimal values
   * @returns create mutable Color4
   */
  export function fromHexString(hex: string): MutableColor4 {
    if (hex.substring(0, 1) !== '#' || (hex.length !== 7 && hex.length !== 9)) {
      return create(0.0, 0.0, 0.0, 1.0)
    }

    const r = parseInt(hex.substring(1, 3), 16)
    const g = parseInt(hex.substring(3, 5), 16)
    const b = parseInt(hex.substring(5, 7), 16)
    const aStr = hex.substring(7, 9)
    const a = aStr ? parseInt(aStr, 16) : 255

    return Color4.fromInts(r, g, b, a)
  }

  /**
   * Creates create mutable Color4  set with the linearly interpolated values of "amount" between the left Color4 object and the right Color4 object
   * @param left - defines the start value
   * @param right - defines the end value
   * @param amount - defines the gradient factor
   * @returns create mutable Color4
   */
  export function lerp(
    left: ReadonlyColor4,
    right: ReadonlyColor4,
    amount: number
  ): MutableColor4 {
    const result = create(0.0, 0.0, 0.0, 0.0)
    Color4.lerpToRef(left, right, amount, result)
    return result
  }

  /**
   * Set the given "result" with the linearly interpolated values of "amount" between the left Color4 object and the right Color4 object
   * @param left - defines the start value
   * @param right - defines the end value
   * @param amount - defines the gradient factor
   * @param result - defines the Color4 object where to store data
   */
  export function lerpToRef(
    left: ReadonlyColor4,
    right: ReadonlyColor4,
    amount: number,
    result: MutableColor4
  ): void {
    result.r = left.r + (right.r - left.r) * amount
    result.g = left.g + (right.g - left.g) * amount
    result.b = left.b + (right.b - left.b) * amount
    result.a = left.a + (right.a - left.a) * amount
  }

  /**
   * Returns a Color4 value containing a red color
   * @returns a new Color4
   */
  export function Red(): MutableColor4 {
    return create(1.0, 0, 0, 1.0)
  }
  /**
   * Returns a Color4 value containing a green color
   * @returns create mutable Color4
   */
  export function Green(): MutableColor4 {
    return create(0, 1.0, 0, 1.0)
  }
  /**
   * Returns a Color4 value containing a blue color
   * @returns create mutable Color4
   */
  export function Blue(): MutableColor4 {
    return create(0, 0, 1.0, 1.0)
  }
  /**
   * Returns a Color4 value containing a black color
   * @returns create mutable Color4
   */
  export function Black(): MutableColor4 {
    return create(0, 0, 0, 1)
  }
  /**
   * Returns a Color4 value containing a white color
   * @returns create mutable Color4
   */
  export function White(): MutableColor4 {
    return create(1, 1, 1, 1)
  }
  /**
   * Returns a Color4 value containing a purple color
   * @returns create mutable Color4
   */
  export function Purple(): MutableColor4 {
    return create(0.5, 0, 0.5, 1)
  }
  /**
   * Returns a Color4 value containing a magenta color
   * @returns create mutable Color4
   */
  export function Magenta(): MutableColor4 {
    return create(1, 0, 1, 1)
  }
  /**
   * Returns a Color4 value containing a yellow color
   * @returns create mutable Color4
   */
  export function Yellow(): MutableColor4 {
    return create(1, 1, 0, 1)
  }
  /**
   * Returns a Color4 value containing a gray color
   * @returns create mutable Color4
   */
  export function Gray(): MutableColor4 {
    return create(0.5, 0.5, 0.5, 1.0)
  }
  /**
   * Returns a Color4 value containing a teal color
   * @returns create mutable Color4
   */
  export function Teal(): MutableColor4 {
    return create(0, 1.0, 1.0, 1.0)
  }
  /**
   * Returns a Color4 value containing a transparent color
   * @returns create mutable Color4
   */
  export function Clear(): MutableColor4 {
    return create(0, 0, 0, 0)
  }

  /**
   * Creates a Color4 from a Color3 and an alpha value
   * @param color3 - defines the source Color3 to read from
   * @param alpha - defines the alpha component (1.0 by default)
   * @returns create mutable Color4
   */
  export function fromColor3(
    color3: Color3.ReadonlyColor3,
    alpha: number = 1.0
  ): MutableColor4 {
    return create(color3.r, color3.g, color3.b, alpha)
  }

  /**
   * Creates a Color4 from the starting index element of the given array
   * @param array - defines the source array to read from
   * @param offset - defines the offset in the source array
   * @returns create mutable Color4
   */
  export function fromArray(
    array: ArrayLike<number>,
    offset: number = 0
  ): ReadonlyColor4 {
    return create(
      array[offset],
      array[offset + 1],
      array[offset + 2],
      array[offset + 3]
    )
  }

  /**
   * Creates a new Color3 from integer values (less than 256)
   * @param r - defines the red component to read from (value between 0 and 255)
   * @param g - defines the green component to read from (value between 0 and 255)
   * @param b - defines the blue component to read from (value between 0 and 255)
   * @param a - defines the alpha component to read from (value between 0 and 255)
   * @returns a new Color4
   */
  export function fromInts(
    r: number,
    g: number,
    b: number,
    a: number
  ): MutableColor4 {
    return create(r / 255.0, g / 255.0, b / 255.0, a / 255.0)
  }

  /**
   * Check the content of a given array and convert it to an array containing RGBA data
   * If the original array was already containing count * 4 values then it is returned directly
   * @param colors - defines the array to check
   * @param count - defines the number of RGBA data to expect
   * @returns an array containing count * 4 values (RGBA)
   */
  export function checkColors4(colors: number[], count: number): number[] {
    // Check if color3 was used
    if (colors.length === count * 3) {
      const colors4 = []
      for (let index = 0; index < colors.length; index += 3) {
        const newIndex = (index / 3) * 4
        colors4[newIndex] = colors[index]
        colors4[newIndex + 1] = colors[index + 1]
        colors4[newIndex + 2] = colors[index + 2]
        colors4[newIndex + 3] = 1.0
      }

      return colors4
    }

    return colors
  }

  // Operators

  /**
   * Adds  the given Color4 values to the ref Color4 object
   * @param a - defines the first operand
   * @param b - defines the second operand
   * @param ref - defines the result rference
   * @returns
   */
  export function addToRef(
    a: ReadonlyColor4,
    b: ReadonlyColor4,
    ref: MutableColor4
  ): void {
    ref.r = a.r + b.r
    ref.g = a.g + b.g
    ref.b = a.b + b.b
    ref.a = a.a + b.a
  }

  /**
   * Stores from the starting index in the given array the Color4 successive values
   * @param array - defines the array where to store the r,g,b components
   * @param index - defines an optional index in the target array to define where to start storing values
   * @returns the current Color4 object
   */
  export function toArray(
    value: ReadonlyColor4,
    array: number[],
    index: number = 0
  ): void {
    array[index] = value.r
    array[index + 1] = value.g
    array[index + 2] = value.b
    array[index + 3] = value.a
  }

  /**
   * Creates a Color4 set with the added values of the current Color4 and of the given one
   * @param right - defines the second operand
   * @returns create mutable Color4
   */
  export function add(
    value: ReadonlyColor4,
    right: ReadonlyColor4
  ): MutableColor4 {
    const ret: MutableColor4 = Clear()
    addToRef(value, right, ret)
    return ret
  }

  /**
   * Creates a Color4 set with the subtracted values of the given one from the current Color4
   * @param right - defines the second operand
   * @returns create mutable Color4
   */
  export function subtract(
    value: ReadonlyColor4,
    right: ReadonlyColor4
  ): ReadonlyColor4 {
    const ret: MutableColor4 = Clear()
    subtractToRef(value, right, ret)
    return ret
  }

  /**
   * Subtracts the given ones from the current Color4 values and stores the results in "result"
   * @param right - defines the second operand
   * @param result - defines the Color4 object where to store the result
   * @returns the current Color4 object
   */
  export function subtractToRef(
    a: ReadonlyColor4,
    b: ReadonlyColor4,
    result: MutableColor4
  ): void {
    result.r = a.r - b.r
    result.g = a.g - b.g
    result.b = a.b - b.b
    result.a = a.a - b.a
  }

  /**
   * Creates a Color4 with the current Color4 values multiplied by scale
   * @param scale - defines the scaling factor to apply
   * @returns create mutable Color4
   */
  export function scale(value: ReadonlyColor4, scale: number): ReadonlyColor4 {
    return create(
      value.r * scale,
      value.g * scale,
      value.b * scale,
      value.a * scale
    )
  }

  /**
   * Multiplies the current Color4 values by scale and stores the result in "result"
   * @param scale - defines the scaling factor to apply
   * @param result - defines the Color4 object where to store the result
   */
  export function scaleToRef(
    value: ReadonlyColor4,
    scale: number,
    result: MutableColor4
  ): void {
    result.r = value.r * scale
    result.g = value.g * scale
    result.b = value.b * scale
    result.a = value.a * scale
  }

  /**
   * Scale the current Color4 values by a factor and add the result to a given Color4
   * @param scale - defines the scale factor
   * @param result - defines the Color4 object where to store the result
   */
  export function scaleAndAddToRef(
    value: ReadonlyColor4,
    scale: number,
    result: MutableColor4
  ): void {
    result.r += value.r * scale
    result.g += value.g * scale
    result.b += value.b * scale
    result.a += value.a * scale
  }

  /**
   * Clamps the rgb values by the min and max values and stores the result into "result"
   * @param min - defines minimum clamping value (default is 0)
   * @param max - defines maximum clamping value (default is 1)
   * @param result - defines color to store the result into.
   */
  export function clampToRef(
    value: ReadonlyColor4,
    min: number = 0,
    max: number = 1,
    result: MutableColor4
  ): void {
    result.r = Scalar.clamp(value.r, min, max)
    result.g = Scalar.clamp(value.g, min, max)
    result.b = Scalar.clamp(value.b, min, max)
    result.a = Scalar.clamp(value.a, min, max)
  }

  /**
   * Multipy an Color4 value by another and return create mutable Color4
   * @param color - defines the Color4 value to multiply by
   * @returns create mutable Color4
   */
  export function multiply(
    value: ReadonlyColor4,
    color: ReadonlyColor4
  ): ReadonlyColor4 {
    return create(
      value.r * color.r,
      value.g * color.g,
      value.b * color.b,
      value.a * color.a
    )
  }

  /**
   * Multipy a Color4 value by another and push the result in a reference value
   * @param color - defines the Color4 value to multiply by
   * @param result - defines the Color4 to fill the result in
   * @returns the result Color4
   */
  export function multiplyToRef(
    value: ReadonlyColor4,
    color: ReadonlyColor4,
    result: MutableColor4
  ): void {
    result.r = value.r * color.r
    result.g = value.g * color.g
    result.b = value.b * color.b
    result.a = value.a * color.a
  }

  /**
   * Creates a string with the Color4 current values
   * @returns the string representation of the Color4 object
   */
  export function toString(value: ReadonlyColor4): string {
    return (
      '{R: ' +
      value.r +
      ' G:' +
      value.g +
      ' B:' +
      value.b +
      ' A:' +
      value.a +
      '}'
    )
  }

  /**
   * Compute the Color4 hash code
   * @returns an unique number that can be used to hash Color4 objects
   */
  export function getHashCode(value: ReadonlyColor4): number {
    let hash = value.r || 0
    hash = (hash * 397) ^ (value.g || 0)
    hash = (hash * 397) ^ (value.b || 0)
    hash = (hash * 397) ^ (value.a || 0)
    return hash
  }

  /**
   * Creates a Color4 copied from the current one
   * @returns create mutable Color4
   */
  export function clone(value: ReadonlyColor4): MutableColor4 {
    return create(value.r, value.g, value.b, value.a)
  }

  /**
   * Copies the given Color4 values into the destination
   * @param source - defines the source Color4 object
   * @param dest - defines the destination Color4 object
   * @returns
   */
  export function copyFrom(source: ReadonlyColor4, dest: MutableColor4): void {
    dest.r = source.r
    dest.g = source.g
    dest.b = source.b
    dest.a = source.a
  }

  /**
   * Copies the given float values into the current one
   * @param r - defines the red component to read from
   * @param g - defines the green component to read from
   * @param b - defines the blue component to read from
   * @param a - defines the alpha component to read from
   * @returns the current updated Color4 object
   */
  export function copyFromFloats(
    r: number,
    g: number,
    b: number,
    a: number,
    dest: MutableColor4
  ): void {
    dest.r = r
    dest.g = g
    dest.b = b
    dest.a = a
  }

  /**
   * Copies the given float values into the current one
   * @param r - defines the red component to read from
   * @param g - defines the green component to read from
   * @param b - defines the blue component to read from
   * @param a - defines the alpha component to read from
   * @returns the current updated Color4 object
   */
  export function set(
    r: number,
    g: number,
    b: number,
    a: number,
    dest: MutableColor4
  ): void {
    dest.r = r
    dest.g = g
    dest.b = b
    dest.a = a
  }

  /**
   * Compute the Color4 hexadecimal code as a string
   * @returns a string containing the hexadecimal representation of the Color4 object
   */
  export function toHexString(value: ReadonlyColor4): string {
    const intR = (value.r * 255) | 0
    const intG = (value.g * 255) | 0
    const intB = (value.b * 255) | 0
    const intA = (value.a * 255) | 0
    return (
      '#' +
      Scalar.toHex(intR) +
      Scalar.toHex(intG) +
      Scalar.toHex(intB) +
      Scalar.toHex(intA)
    )
  }

  /**
   * Computes a Color4 converted from the current one to linear space
   * @returns create mutable Color4
   */
  export function toLinearSpace(value: ReadonlyColor4): MutableColor4 {
    const convertedColor = create()
    toLinearSpaceToRef(value, convertedColor)
    return convertedColor
  }

  /**
   * Converts the Color4 values to linear space and stores the result in "convertedColor"
   * @param convertedColor - defines the Color4 object where to store the linear space version
   * @returns the unmodified Color4
   */
  export function toLinearSpaceToRef(
    value: ReadonlyColor4,
    ref: MutableColor4
  ): void {
    ref.r = Math.pow(value.r, ToLinearSpace)
    ref.g = Math.pow(value.g, ToLinearSpace)
    ref.b = Math.pow(value.b, ToLinearSpace)
    ref.a = value.a
  }

  /**
   * Computes a Color4 converted from the current one to gamma space
   * @returns create mutable Color4
   */
  export function toGammaSpace(value: ReadonlyColor4): ReadonlyColor4 {
    const convertedColor = create()
    toGammaSpaceToRef(value, convertedColor)
    return convertedColor
  }

  /**
   * Converts the Color4 values to gamma space and stores the result in "convertedColor"
   * @param convertedColor - defines the Color4 object where to store the gamma space version
   * @returns the unmodified Color4
   */
  export function toGammaSpaceToRef(
    value: ReadonlyColor4,
    convertedColor: MutableColor4
  ): void {
    convertedColor.r = Math.pow(value.r, ToGammaSpace)
    convertedColor.g = Math.pow(value.g, ToGammaSpace)
    convertedColor.b = Math.pow(value.b, ToGammaSpace)
    convertedColor.a = value.a
  }
}
