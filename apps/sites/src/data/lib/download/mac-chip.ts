export type MacChip = "apple_silicon" | "intel" | "unknown";

/**
 * A Mac's chip from the name of its GPU. Intel GPUs first: Chrome prefixes every Mac renderer with
 * "ANGLE (Apple, ANGLE Metal Renderer: …)", and NVIDIA and AMD shipped only in Intel-era Macs. Only a named
 * M-series chip proves Apple Silicon; Safari names every Mac "Apple GPU", which stays unknown.
 */
export function macChipFromRenderer(renderer: string): MacChip {
  if (/intel|amd|radeon|nvidia|geforce|quadro/i.test(renderer)) return "intel";
  if (/\bApple M\d/.test(renderer)) return "apple_silicon";
  return "unknown";
}

/**
 * The chip of the Mac this page runs on. The User-Agent says "Intel" on every Mac, so the server cannot tell;
 * the WebGL renderer names the GPU in Chrome and Firefox.
 */
export function detectMacChip(): MacChip {
  let renderer = "";
  try {
    const gl = document.createElement("canvas").getContext("webgl");
    const info = gl?.getExtension("WEBGL_debug_renderer_info");
    if (gl && info) renderer = String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL));
    // Some browsers cap live WebGL contexts.
    gl?.getExtension("WEBGL_lose_context")?.loseContext();
  } catch {
    // No WebGL here: the renderer stays unnamed, and so the chip unknown.
  }
  return macChipFromRenderer(renderer);
}

/** An iPad names itself a Mac in its User-Agent; its touch screen gives it away. */
export function isTouchMac(): boolean {
  return navigator.maxTouchPoints > 1;
}

type Build = { osKey: string; arch: string };

/**
 * Puts the build for a Mac's chip first where it differs from the server's pick, which assumes Apple Silicon.
 * Other systems, and a Mac whose chip is unknown, keep the order they came in.
 */
export function preferChip<T extends Build>(chip: MacChip, primary: T, others: T[]): { primary: T; others: T[] } {
  if (primary.osKey !== "macos") return { primary, others };
  const arch = chip === "intel" ? "amd64" : chip === "apple_silicon" ? "arm64" : null;
  const match = arch && arch !== primary.arch ? others.find((o) => o.osKey === "macos" && o.arch === arch) : undefined;
  if (!match) return { primary, others };
  return { primary: match, others: others.map((o) => (o === match ? primary : o)) };
}
