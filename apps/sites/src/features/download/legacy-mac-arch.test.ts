// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { macChipFromRenderer } from "@/data/lib/download/mac-chip";

import { legacyMacArch, legacyMacArchFromRenderer, resetLegacyMacArch } from "./legacy-mac-arch";

// Renderer strings with the value decentraland/sites `macArchHint.ts` sent as `mac_arch`, and what installer
// selection (`macChipFromRenderer`) makes of the same string. The rows where they differ are why the two stay separate.
const RENDERERS: Array<[renderer: string, legacy: string, installer: string]> = [
  ["ANGLE (Apple, ANGLE Metal Renderer: Apple M4 Pro, Unspecified Version)", "apple_silicon", "apple_silicon"],
  ["Apple M1", "apple_silicon", "apple_silicon"],
  // Safari masks every Mac GPU.
  ["Apple GPU", "apple_silicon", "unknown"],
  // Chrome's ANGLE prefix names Apple over an Intel or AMD GPU: legacy counted these as Apple Silicon.
  ["ANGLE (Apple, ANGLE Metal Renderer: AMD Radeon Pro 5500M, Unspecified Version)", "apple_silicon", "intel"],
  ["ANGLE (Apple, ANGLE Metal Renderer: Intel(R) Iris(TM) Plus Graphics, Unspecified Version)", "apple_silicon", "intel"],
  ["ANGLE (Intel Inc., Intel(R) UHD Graphics 630, OpenGL 4.1)", "intel", "intel"],
  ["AMD Radeon Pro 560X OpenGL Engine", "intel", "intel"],
  ["NVIDIA GeForce GT 750M OpenGL Engine", "intel", "intel"],
  ["Quadro K2000", "intel", "intel"],
  ["Mesa llvmpipe", "unknown", "unknown"],
  ["", "unknown", "unknown"],
];

describe("legacyMacArchFromRenderer", () => {
  it.each(RENDERERS)("%j is %s for the Click", (renderer, legacy) => {
    expect(legacyMacArchFromRenderer(renderer)).toBe(legacy);
  });

  it.each(RENDERERS)("%j (Click %s) is %s for installer selection", (renderer, _legacy, installer) => {
    expect(macChipFromRenderer(renderer)).toBe(installer);
  });

  it("is unknown when no renderer could be read", () => {
    expect(legacyMacArchFromRenderer(null)).toBe("unknown");
  });
});

describe("legacyMacArch", () => {
  const MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/17.0 Safari/605.1.15";

  function browser(ua: string, { touch = 0, renderer = "Apple GPU" as string | null, webgl = true } = {}) {
    vi.stubGlobal("navigator", { userAgent: ua, maxTouchPoints: touch });
    const loseContext = vi.fn();
    const gl = {
      getExtension: (name: string) =>
        name === "WEBGL_debug_renderer_info"
          ? renderer === null
            ? null
            : { UNMASKED_RENDERER_WEBGL: 37446 }
          : { loseContext },
      getParameter: () => renderer,
    };
    const getContext = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue((webgl ? gl : null) as never);
    resetLegacyMacArch();
    return { getContext, loseContext };
  }

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("classifies a real Mac's GPU the legacy way, once per page, freeing the context", () => {
    const { getContext, loseContext } = browser(MAC);
    expect(legacyMacArch()).toBe("apple_silicon");
    expect(legacyMacArch()).toBe("apple_silicon");
    expect(getContext).toHaveBeenCalledTimes(1);
    expect(loseContext).toHaveBeenCalledTimes(1);
  });

  it("is null off a Mac and for an iPad that names itself one, without touching WebGL", () => {
    const { getContext } = browser("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
    expect(legacyMacArch()).toBeNull();
    browser(MAC, { touch: 5 });
    expect(legacyMacArch()).toBeNull();
    expect(getContext).not.toHaveBeenCalled();
  });

  it("is unknown on a Mac without WebGL or without the renderer extension", () => {
    browser(MAC, { webgl: false });
    expect(legacyMacArch()).toBe("unknown");
    browser(MAC, { renderer: null });
    expect(legacyMacArch()).toBe("unknown");
  });

  it("is unknown when WebGL throws", () => {
    browser(MAC);
    vi.mocked(HTMLCanvasElement.prototype.getContext).mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(legacyMacArch()).toBe("unknown");
  });
});
