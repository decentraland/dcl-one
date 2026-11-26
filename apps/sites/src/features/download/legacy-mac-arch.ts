/**
 * The legacy site's `mac_arch` on a download `Click` (decentraland/sites `src/modules/macArchHint.ts`), kept with its
 * original classification so the warehouse series does not shift: any renderer naming Apple counts as Apple Silicon,
 * including Safari's masked "Apple GPU" and Chrome's "ANGLE (Apple, …)" prefix over an Intel or AMD GPU. Installer
 * selection uses `detectMacChip` (`data/lib/download/mac-chip.ts`) instead, which tells those apart; the two must
 * stay separate.
 */
export type LegacyMacArch = "apple_silicon" | "intel" | "unknown";

/** The legacy classification of a WebGL unmasked renderer; null means no renderer could be read. */
export function legacyMacArchFromRenderer(renderer: string | null): LegacyMacArch {
  if (!renderer) return "unknown";
  if (/apple/i.test(renderer)) return "apple_silicon";
  // NVIDIA shipped only in Intel-era Macs, so any of these vendors proves an Intel Mac.
  if (/intel|amd|radeon|nvidia|geforce|quadro/i.test(renderer)) return "intel";
  return "unknown";
}

function readWebGlRenderer(): string | null {
  const canvas = document.createElement("canvas");
  const gl = (canvas.getContext("webgl") || canvas.getContext("experimental-webgl")) as WebGLRenderingContext | null;
  if (!gl) return null;
  const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
  const renderer = debugInfo ? String(gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL)) : null;
  // Some browsers cap live WebGL contexts.
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  return renderer;
}

/** Null off a real Mac: an iPhone never says "Macintosh", and an iPad that does has a touch screen. */
function detect(): LegacyMacArch | null {
  if (!/Macintosh/.test(navigator.userAgent)) return null;
  if (navigator.maxTouchPoints > 1) return null;
  try {
    return legacyMacArchFromRenderer(readWebGlRenderer());
  } catch {
    return "unknown";
  }
}

// The GPU cannot change during a page load: one WebGL context at most.
let cached: LegacyMacArch | null | undefined;

export function legacyMacArch(): LegacyMacArch | null {
  if (cached === undefined) cached = detect();
  return cached;
}

/** Test hook: forget the memoized value so a test can change the browser. */
export function resetLegacyMacArch(): void {
  cached = undefined;
}
