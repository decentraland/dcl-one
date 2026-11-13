// The journey's settings: environment variables, optionally read from a KEY=value file kept outside
// the repository (E2E_ENV_FILE). Values are never printed; only addresses derived from keys are.
import { readFileSync } from "node:fs";

/** KEY=value lines; `#` comments, blank lines and `export ` prefixes are skipped; quotes are stripped. */
export function parseEnvFile(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim().replace(/^export\s+/, "");
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    let value = line.slice(eq + 1).trim();
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) value = value.slice(1, -1);
    out[line.slice(0, eq).trim()] = value;
  }
  return out;
}

/** The process environment, under the file E2E_ENV_FILE names (the environment wins). */
export function journeyEnv(env: NodeJS.ProcessEnv = process.env): NodeJS.ProcessEnv {
  const file = env.E2E_ENV_FILE;
  if (!file) return env;
  return { ...parseEnvFile(readFileSync(file, "utf8")), ...env };
}

/** `0xcontract:tokenId` → its parts, lowercased contract. */
export function parseToken(value: string | undefined): { contract: string; tokenId: string } {
  const m = value?.trim().match(/^(0x[0-9a-fA-F]{40}):(\d+)$/);
  if (!m) throw new Error("E2E_TOKEN must be <contract>:<tokenId>, an Amoy NFT the seller owns");
  return { contract: m[1]!.toLowerCase(), tokenId: m[2]! };
}

/** `0xcollection:itemId` → its parts, lowercased collection: a primary item, by its blockchain item id. */
export function parseItem(value: string | undefined): { contract: string; itemId: string } {
  const m = value?.trim().match(/^(0x[0-9a-fA-F]{40}):(\d+)$/);
  if (!m) throw new Error("E2E_ITEM must be <collection>:<itemId>, an Amoy item of the seller's collection listed in the Shop");
  return { contract: m[1]!.toLowerCase(), itemId: m[2]! };
}

/** A whole-number discount within the Shop's bounds (MIN_SALE_PCT..MAX_SALE_PCT in seller/lib/coupons.ts). */
export function parseDiscount(value: string | undefined): number {
  const text = value?.trim() || "10";
  const pct = Number(text);
  if (!/^\d+$/.test(text) || pct < 5 || pct > 70) throw new Error("E2E_DISCOUNT must be a whole percentage from 5 to 70");
  return pct;
}

/** The mock top-up: positive credits with at most two decimals, within the server's 10,000 per request. */
export function parseTopup(value: string | undefined): string {
  const text = value?.trim() || "100";
  if (!/^\d+(\.\d{1,2})?$/.test(text) || Number(text) <= 0 || Number(text) > 10_000) throw new Error("E2E_TOPUP must be credits from 0.01 to 10000");
  return text;
}
