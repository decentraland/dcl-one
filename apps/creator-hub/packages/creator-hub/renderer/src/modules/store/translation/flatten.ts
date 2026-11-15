/**
 * Gives an object with one level of keys. The key of a nested value is the path to it, joined
 * with dots: `{ a: { b: 'c' } }` gives `{ 'a.b': 'c' }`. An array gives one key for each index,
 * and an empty object or an empty array stays as a value.
 */
export function flatten<T, R>(target: T): R {
  const output: Record<string, unknown> = {};

  function step(value: object, prefix: string) {
    for (const [key, child] of Object.entries(value)) {
      const path = prefix ? `${prefix}.${key}` : key;
      const kind = Object.prototype.toString.call(child);
      const nested = kind === '[object Object]' || kind === '[object Array]';
      if (nested && Object.keys(child).length > 0) {
        step(child, path);
      } else {
        output[path] = child;
      }
    }
  }

  step(target as object, '');
  return output as R;
}
