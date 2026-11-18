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
