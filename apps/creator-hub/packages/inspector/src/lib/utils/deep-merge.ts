const UNSAFE_KEYS = new Set([
  '__proto__',
  'constructor',
  'prototype',
  'toString',
  'valueOf',
  'hasOwnProperty',
  'isPrototypeOf',
  'propertyIsEnumerable',
  'toLocaleString',
]);

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

/**
 * Gives a new object with `changes` on top of `base`. Plain objects merge key by key. Any other
 * value in `changes` replaces the value in `base`: an array is not joined with the array in
 * `base`, and `undefined` stays. Keys that can change how an object behaves are left out.
 */
export function mergeDeep(base: object, changes: object = {}): Record<string, unknown> {
  const merged: Record<string, unknown> = {};
  for (const source of [base, changes] as Record<string, unknown>[]) {
    for (const key of Object.keys(source)) {
      if (UNSAFE_KEYS.has(key)) continue;
      const value = source[key];
      const current = merged[key];
      merged[key] = isPlainObject(value)
        ? mergeDeep(isPlainObject(current) ? current : {}, value)
        : value;
    }
  }
  return merged;
}
