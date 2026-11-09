// The Shop modules the in-world Shop reads through take their host as arguments: the client, its base, transport
// and signer, and the peers. None reaches for the browser host's wiring (api/host.ts), ui3's catalyst or auth
// modules, or the page, so each can move to library/shop as it is.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const SHOP = join(__dirname, "..");
const SEAM = [
  "api/client.ts",
  "api/shop.ts",
  "api/signer.ts",
  "checkout/lib/api.ts",
  "credits/credits.ts",
  "detail/api.ts",
  "detail/lib/urn.ts",
  "discovery/api.ts",
];

// The code only: a comment may name what the module no longer reaches for.
const code = (file: string): string => readFileSync(join(SHOP, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");

describe("the host seam of the Shop's read modules", () => {
  it.each(SEAM)("%s imports no host wiring and reads no page", (file) => {
    const source = code(file);
    expect(source).not.toMatch(/from\s+["'][^"']*\/(api\/host|data\/(catalyst|auth)\/[^"']*)["']/);
    expect(source).not.toMatch(/\b(window|document|localStorage|sessionStorage|location)\b|import\.meta|\bfetch\(/);
    expect(source).not.toMatch(/\bshopClient\(\)/);
  });
});
