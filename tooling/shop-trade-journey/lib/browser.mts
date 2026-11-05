// The journey's Chromium. CHROMIUM_BIN overrides; otherwise the newest /nix/store chromium, else
// playwright-core's own download (`npx playwright-core install chromium`).
import { existsSync, readdirSync } from "node:fs";
import { chromium, type Browser } from "playwright-core";

function nixChromium(): string | undefined {
  if (!existsSync("/nix/store")) return undefined;
  const version = (d: string) => d.match(/-chromium-(\d+(?:\.\d+)*)$/)?.[1]?.split(".").map(Number);
  const newer = (a: number[], b: number[]) => {
    for (let i = 0; i < Math.max(a.length, b.length); i++) if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) - (b[i] ?? 0);
    return 0;
  };
  const found = readdirSync("/nix/store")
    .filter((d) => version(d) && existsSync(`/nix/store/${d}/bin/chromium`))
    .sort((a, b) => newer(version(a)!, version(b)!))
    .at(-1);
  return found ? `/nix/store/${found}/bin/chromium` : undefined;
}

export function launch(): Promise<Browser> {
  return chromium.launch({ executablePath: process.env.CHROMIUM_BIN || nixChromium(), headless: true });
}
