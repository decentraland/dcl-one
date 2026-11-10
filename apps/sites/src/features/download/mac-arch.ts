import type { MacArch } from "./params";

// The Mac CPU hint download clicks carry as mac_arch (source Sites modules/macArchHint.ts). The UA
// says "Intel Mac" on Apple Silicon too, so the GPU decides: an Apple GPU means Apple Silicon. Only
// the coarse enum leaves the browser, never the renderer string. Analytics only: it never picks a
// binary (the macOS installer is always the arm64 build).

let cached: MacArch | null | undefined;

function webGlRenderer(): string | null {
  const canvas = document.createElement("canvas");
  const gl = (canvas.getContext("webgl") ??
    canvas.getContext("experimental-webgl")) as WebGLRenderingContext | null;
  if (!gl) return null;
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  const renderer = info
    ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL))
    : null;
  // Some browsers cap live WebGL contexts.
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  return renderer;
}

export function macArchFromRenderer(renderer: string | null): MacArch {
  if (!renderer) return "unknown";
  if (/apple/i.test(renderer)) return "apple_silicon";
  // These GPU vendors only shipped in Intel-era Macs.
  if (/intel|amd|radeon|nvidia|geforce|quadro/i.test(renderer)) return "intel";
  return "unknown";
}

/** null off macOS, and on iPads in desktop mode (Macintosh UA, but touch points). */
export function macArchHint(): MacArch | null {
  if (cached !== undefined) return cached;
  if (!/Macintosh/.test(navigator.userAgent) || navigator.maxTouchPoints > 1)
    cached = null;
  else {
    try {
      cached = macArchFromRenderer(webGlRenderer());
    } catch {
      cached = "unknown";
    }
  }
  return cached;
}
