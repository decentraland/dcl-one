import { pickRenderer as chooseRenderer, type RendererDecision } from "@shop/detail/lib/pickRenderer";

const MOBILE_MAX_WIDTH = 768;
const MIN_SAMPLE_BYTES = 30_000;
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
  const n = nav();
  return chooseRenderer({
    forced: import.meta.env?.VITE_SHOP_PREVIEW,
    webgl: typeof window !== "undefined" && typeof window.WebGLRenderingContext !== "undefined",
    get mobile() { return isMobile(); },
    saveData: n?.connection?.saveData,
    downlink: n?.connection?.downlink,
    get measuredMbps() { return measuredMbps(); },
    deviceMemory: n?.deviceMemory,
    dpr: typeof window === "undefined" ? undefined : window.devicePixelRatio,
    cores: n?.hardwareConcurrency,
  });
}
