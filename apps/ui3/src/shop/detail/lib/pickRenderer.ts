// from: shop@ae3083ac:app/src/lib/pickRenderer.ts
// Which preview to start with. Upstream chose between the Unity and Babylon iframes; ui3 has one 3D
// preview (wearable-preview, three.js or the Explorer's Bevy host), so the choice here is whether to
// mount it straight away (`full`) or show the thumbnail with an explicit "Show 3D preview" (`lite`)
// without WebGL, on a phone, data-saver, a slow link, little memory or a hi-DPI screen on a modest CPU. Unknown
// signals stay optimistic. The link is the probe download's reading (`previewDevice.ts`), never the page's own
// resource timings: its chunks are small enough that latency reads a fast link as slow.

export type PreviewRenderer = "full" | "lite";

export const FULL_MIN_MBPS = 10;
export const FULL_MIN_DEVICE_MEMORY = 4;
export const FULL_MIN_DPR = 2;
export const FULL_HIDPI_MIN_HW_CONCURRENCY = 8;
const MOBILE_MAX_WIDTH = 768;

export type RendererReason =
  | "mobile"
  | "save-data"
  | "slow-connection"
  | "low-device-memory"
  | "gpu-capability"
  | "connection-ok"
  | "optimistic-default"
  | "env-override"
  | "no-webgl";

export type RendererDecision = { renderer: PreviewRenderer; reason: RendererReason };

type NavigatorExtras = { connection?: { saveData?: boolean }; deviceMemory?: number; hardwareConcurrency?: number };

function nav(): NavigatorExtras | undefined {
  return typeof navigator === "undefined" ? undefined : (navigator as unknown as NavigatorExtras);
}

function isMobile(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia(`(max-width: ${MOBILE_MAX_WIDTH}px)`).matches;
}

/** `measuredMbps` is the probe download's reading: `null` when it has not run or could not. */
export function pickRenderer(measuredMbps: number | null = null): RendererDecision {
  const lite = (reason: RendererReason): RendererDecision => ({ renderer: "lite", reason });
  const forced = import.meta.env?.VITE_SHOP_PREVIEW as string | undefined;
  if (forced === "lite") return lite("env-override");
  if (forced === "full") return { renderer: "full", reason: "env-override" };
  if (typeof window === "undefined" || typeof (window as { WebGLRenderingContext?: unknown }).WebGLRenderingContext === "undefined") return lite("no-webgl");
  if (isMobile()) return lite("mobile");
  const n = nav();
  if (n?.connection?.saveData) return lite("save-data");
  if (measuredMbps !== null && measuredMbps < FULL_MIN_MBPS) return lite("slow-connection");
  if (n?.deviceMemory !== undefined && n.deviceMemory < FULL_MIN_DEVICE_MEMORY) return lite("low-device-memory");
  const dpr = typeof window !== "undefined" && typeof window.devicePixelRatio === "number" ? window.devicePixelRatio : undefined;
  const cores = n?.hardwareConcurrency;
  if (dpr !== undefined && dpr >= FULL_MIN_DPR && cores !== undefined && cores < FULL_HIDPI_MIN_HW_CONCURRENCY) return lite("gpu-capability");
  return { renderer: "full", reason: measuredMbps !== null ? "connection-ok" : "optimistic-default" };
}
