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
