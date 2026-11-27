import { pickRenderer as chooseRenderer, type RendererDecision } from "@shop/detail/lib/pickRenderer";
export type { RendererDecision } from "@shop/detail/lib/pickRenderer";

const MOBILE_MAX_WIDTH = 768;

type NavigatorExtras = { connection?: { saveData?: boolean }; deviceMemory?: number; hardwareConcurrency?: number };

function nav(): NavigatorExtras | undefined {
  return typeof navigator === "undefined" ? undefined : (navigator as unknown as NavigatorExtras);
}

function isMobile(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia(`(max-width: ${MOBILE_MAX_WIDTH}px)`).matches;
}

export function pickRenderer(measuredMbps: number | null = null): RendererDecision {
  const n = nav();
  return chooseRenderer({
    forced: import.meta.env?.VITE_SHOP_PREVIEW,
    webgl: typeof window !== "undefined" && typeof window.WebGLRenderingContext !== "undefined",
    get mobile() { return isMobile(); },
    saveData: n?.connection?.saveData,
    measuredMbps,
    deviceMemory: n?.deviceMemory,
    dpr: typeof window === "undefined" ? undefined : window.devicePixelRatio,
    cores: n?.hardwareConcurrency,
  });
}
