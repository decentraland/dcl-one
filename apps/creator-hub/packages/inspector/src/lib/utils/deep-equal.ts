/**
 * Tells if two values are equal in depth. Arrays are equal when their items are equal, and
 * objects when they have the same own keys with equal values. Dates are equal by time, regular
 * expressions by source and flags, and `NaN` is equal to `NaN`.
 */
export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (!a || !b || typeof a !== 'object' || typeof b !== 'object') {
    return a !== a && b !== b;
  }
  if (a.constructor !== b.constructor) return false;

  if (Array.isArray(a)) {
    const other = b as unknown[];
    if (a.length !== other.length) return false;
    for (let index = 0; index < a.length; index++) {
      if (!deepEqual(a[index], other[index])) return false;
    }
    return true;
  }
  if (a instanceof RegExp && b instanceof RegExp) {
    return a.source === b.source && a.flags === b.flags;
  }
  if (a.valueOf && a.valueOf !== Object.prototype.valueOf) return a.valueOf() === b.valueOf();
  if (a.toString && a.toString !== Object.prototype.toString) {
    return a.toString() === b.toString();
  }

  const left = a as Record<string, unknown>;
  const right = b as Record<string, unknown>;
  const keys = Object.keys(left);
  if (keys.length !== Object.keys(right).length) return false;
  return keys.every(key => Object.hasOwn(right, key) && deepEqual(left[key], right[key]));
}

/**
 * Deep equality check with tolerance for floating-point numbers
 * Wrapper around deepEqual that provides the same interface as jest-matcher-deep-close-to/lib/recursiveCheck
 */

export function deepEqualWithTolerance(actual: any, expected: any, tolerance: number = 0): boolean {
  if (tolerance === 0) {
    return deepEqual(actual, expected);
  }

  // For tolerance > 0, we need custom comparison for numbers
  return deepEqualWithToleranceRecursive(actual, expected, tolerance);
}

function deepEqualWithToleranceRecursive(actual: any, expected: any, tolerance: number): boolean {
  // Handle null/undefined cases
  if (actual === expected) return true;
  if (actual == null || expected == null) return actual === expected;

  // Handle primitive types
  if (typeof actual !== 'object' || typeof expected !== 'object') {
    // Only apply tolerance for floating-point numbers
    if (
      typeof actual === 'number' &&
      typeof expected === 'number' &&
      (!Number.isInteger(actual) || !Number.isInteger(expected))
    ) {
      if (isNaN(actual) && isNaN(expected)) return true; // consider NaN equal to NaN
      return Math.abs(actual - expected) < Math.pow(10, -tolerance);
    }

    return actual === expected;
  }

  // Handle arrays
  if (Array.isArray(actual) !== Array.isArray(expected)) return false;
  if (Array.isArray(actual)) {
    if (actual.length !== expected.length) return false;
    for (let i = 0; i < actual.length; i++) {
      if (!deepEqualWithToleranceRecursive(actual[i], expected[i], tolerance)) {
        return false;
      }
    }
    return true;
  }

  // Handle objects
  const actualKeys = Object.keys(actual);
  const expectedKeys = Object.keys(expected);

  if (actualKeys.length !== expectedKeys.length) return false;

  for (const key of actualKeys) {
    if (!expectedKeys.includes(key)) return false;
    if (!deepEqualWithToleranceRecursive(actual[key], expected[key], tolerance)) {
      return false;
    }
  }

  return true;
}

/**
 * Alias for backward compatibility with existing code
 * This function returns true if the objects are NOT equal (opposite of deepEqualWithTolerance)
 * to maintain the same behavior as the original recursiveCheck
 */
export function recursiveCheck(actual: any, expected: any, tolerance: number = 0): boolean {
  return !deepEqualWithTolerance(actual, expected, tolerance);
}
