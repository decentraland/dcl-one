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
