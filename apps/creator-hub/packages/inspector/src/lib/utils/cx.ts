export type ClassValue =
  | string
  | number
  | boolean
  | null
  | undefined
  | ClassValue[]
  | { [name: string]: unknown };

/**
 * Joins class names with a space. A string or a number is a name, an array gives its names, and
 * an object gives each key that has a truthy value. Any other falsy value is left out.
 */
export function cx(...values: ClassValue[]): string {
  const names: string[] = [];
  for (const value of values) {
    if (!value) continue;
    if (typeof value === 'string' || typeof value === 'number') {
      names.push(String(value));
    } else if (Array.isArray(value)) {
      const inner = cx(...value);
      if (inner) names.push(inner);
    } else if (typeof value === 'object') {
      for (const [name, enabled] of Object.entries(value)) {
        if (enabled && name) names.push(name);
      }
    }
  }
  return names.join(' ');
}
