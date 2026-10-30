import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const TOOL = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const ROOT = resolve(TOOL, "../..");
export const STATE = join(ROOT, "state", "shop-parity");
/** Recordings hold real answers from the production services: they stay in state/, out of git. */
export const RECORDINGS = join(STATE, "recordings");
/** The pages found identical on this machine's recording. */
export const GATE_FILE = join(STATE, "gate.json");
export const UI3 = join(ROOT, "apps", "ui3");
export const SITES = join(ROOT, "apps", "sites");

export type ReferencePin = {
  repository: string;
  revision: string;
  date: string;
  app: string;
  buildEnv: Record<string, string>;
};

export function referencePin(): ReferencePin {
  return JSON.parse(readFileSync(join(TOOL, "reference.json"), "utf8")) as ReferencePin;
}

/** Where the pinned reference is checked out and built (gitignored). */
export function referenceDir(pin: ReferencePin = referencePin()): string {
  return join(STATE, "reference", pin.revision);
}

export function referenceDist(pin: ReferencePin = referencePin()): string {
  return join(referenceDir(pin), "src", pin.app, "dist");
}

/** Loopback ports of a capture; SHOP_PARITY_PORT_BASE moves the three together. */
const PORT_BASE = Number(process.env.SHOP_PARITY_PORT_BASE || 5176);
if (!Number.isInteger(PORT_BASE) || PORT_BASE < 1024 || PORT_BASE > 65533) throw new Error(`SHOP_PARITY_PORT_BASE must be a port from 1024 to 65533, not ${JSON.stringify(process.env.SHOP_PARITY_PORT_BASE)}`);

export const PORTS = {
  reference: () => PORT_BASE,
  replay: () => PORT_BASE + 1,
  sites: () => PORT_BASE + 2,
};
