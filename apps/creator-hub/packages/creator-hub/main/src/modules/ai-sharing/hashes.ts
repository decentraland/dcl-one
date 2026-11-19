import { createHash } from 'crypto';

/**
 * How a shared turn names the assistant it ran with. The release workflow's
 * `assistant-manifest.json` is built with these same functions, and Assistant Insights checks
 * a manifest's hashes against the records' before trusting it, so changing either changes both.
 */

export function sha256(value: string): string {
  return createHash('sha256').update(value, 'utf8').digest('hex');
}

/** JSON with object keys sorted at every level and no whitespace: one text per value. */
export function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(v => stableJson(v ?? null)).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, v]) => v !== undefined)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stableJson(v)}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

/** The hex SHA-256 of a tool list, whatever order its entries and keys come in. */
export function hashTools(tools: readonly { name: string }[]): string {
  const sorted = [...tools].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  return sha256(stableJson(sorted));
}
