/** The name a new GUI starts from: `GUI`, then `GUI1`, `GUI2`. */
export const DEFAULT_ROOT_NAME = 'GUI';

/** Turn arbitrary input into a valid PascalCase component identifier; falls back to "GUI" when empty. */
export function toComponentName(input: string): string {
  const pascal = (input || '')
    .replace(/[^A-Za-z0-9]+/g, ' ')
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map(w => w.charAt(0).toUpperCase() + w.slice(1))
    .join('');
  const safe = pascal.replace(/^[^A-Za-z_]+/, '');
  return safe || DEFAULT_ROOT_NAME;
}

/** Ensure `base` is unique among `existing` by appending the smallest non-colliding numeric suffix. */
export function uniqueName(base: string, existing: readonly string[]): string {
  const names = new Set(existing);
  if (!names.has(base)) return base;
  let i = 1;
  while (names.has(`${base}${i}`)) i++;
  return `${base}${i}`;
}
