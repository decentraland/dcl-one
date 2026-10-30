// from: shop@94584a5c:app/src/lib/pickRenderer.ts
// Which preview to start with. Upstream chose between the Unity and Babylon iframes; ui3 has one 3D
// preview (wearable-preview, three.js or the Explorer's Bevy host), so the choice here is whether to
// mount it straight away (`full`) or show the thumbnail with an explicit "Show 3D preview" (`lite`)
// without WebGL, on a phone, data-saver, a slow link, little memory or a hi-DPI screen on a modest CPU. Unknown
// signals stay optimistic.

export type PreviewRenderer = "full" | "lite";

export const FULL_MIN_MBPS = 10;
export const FULL_MIN_DOWNLINK_MBPS = 4;
export const FULL_MIN_DEVICE_MEMORY = 4;
export const FULL_MIN_DPR = 2;
export const FULL_HIDPI_MIN_HW_CONCURRENCY = 8;
const MOBILE_MAX_WIDTH = 768;
const MIN_SAMPLE_BYTES = 30_000;

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

type NavigatorExtras = { connection?: { downlink?: number; saveData?: boolean }; deviceMemory?: number; hardwareConcurrency?: number };

function nav(): NavigatorExtras | undefined {
  return typeof navigator === "undefined" ? undefined : (navigator as unknown as NavigatorExtras);
}

function isMobile(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia(`(max-width: ${MOBILE_MAX_WIDTH}px)`).matches;
}

function measuredMbps(): number | null {
  if (typeof performance === "undefined" || typeof performance.getEntriesByType !== "function") return null;
  let best: number | null = null;
  for (const e of performance.getEntriesByType("resource") as PerformanceResourceTiming[]) {
    if (e.transferSize >= MIN_SAMPLE_BYTES && e.duration > 0) {
      const mbps = (e.transferSize * 8) / 1e6 / (e.duration / 1000);
      if (best === null || mbps > best) best = mbps;
    }
  }
  return best;
}

export function pickRenderer(): RendererDecision {
  const lite = (reason: RendererReason): RendererDecision => ({ renderer: "lite", reason });
  const forced = import.meta.env?.VITE_SHOP_PREVIEW as string | undefined;
  if (forced === "lite") return lite("env-override");
  if (forced === "full") return { renderer: "full", reason: "env-override" };
  if (typeof window === "undefined" || typeof (window as { WebGLRenderingContext?: unknown }).WebGLRenderingContext === "undefined") return lite("no-webgl");
  if (isMobile()) return lite("mobile");
  const n = nav();
  if (n?.connection?.saveData) return lite("save-data");
  const measured = measuredMbps();
  const downlink = n?.connection?.downlink;
  if (measured !== null && measured < FULL_MIN_MBPS) return lite("slow-connection");
  if (measured === null && typeof downlink === "number" && downlink > 0 && downlink < FULL_MIN_DOWNLINK_MBPS) return lite("slow-connection");
  if (n?.deviceMemory !== undefined && n.deviceMemory < FULL_MIN_DEVICE_MEMORY) return lite("low-device-memory");
  const dpr = typeof window !== "undefined" && typeof window.devicePixelRatio === "number" ? window.devicePixelRatio : undefined;
  const cores = n?.hardwareConcurrency;
  if (dpr !== undefined && dpr >= FULL_MIN_DPR && cores !== undefined && cores < FULL_HIDPI_MIN_HW_CONCURRENCY) return lite("gpu-capability");
  const hasReading = measured !== null || typeof downlink === "number";
  return { renderer: "full", reason: hasReading ? "connection-ok" : "optimistic-default" };
}
