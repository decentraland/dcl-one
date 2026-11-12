import { describe, expect, it } from "vitest";

import { detectDownloadBrowser, stepImages } from "./steps";

const UA = {
  chrome:
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
  edge: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",
  opera:
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 OPR/115.0.0.0",
  firefox:
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:132.0) Gecko/20100101 Firefox/132.0",
  safari:
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15",
};

describe("detectDownloadBrowser", () => {
  it.each(Object.entries(UA))(
    "recognizes %s by name, not by engine",
    (name, ua) => {
      expect(detectDownloadBrowser(ua)).toBe(name);
    },
  );

  it("recognizes Brave from its navigator flag, since its UA reads as Chrome", () => {
    expect(detectDownloadBrowser(UA.chrome, true)).toBe("brave");
  });

  it("falls back to Chrome for an unknown browser", () => {
    expect(detectDownloadBrowser("curl/8.0")).toBe("chrome");
  });
});

describe("stepImages", () => {
  it("highlights the browser's download button on step 1", () => {
    expect(stepImages("macos", "safari")[0].highlight).toEqual({
      x: 49.37,
      y: 26.25,
    });
    expect(stepImages("windows", "firefox")[0].highlight).toEqual({
      x: 76.46,
      y: 41.35,
    });
  });

  it("falls back to the Chrome capture when the OS has none for the browser", () => {
    expect(stepImages("windows", "safari")[0]).toEqual(
      stepImages("windows", "chrome")[0],
    );
  });

  it("reuses the Windows Edge capture on macOS", () => {
    expect(stepImages("macos", "edge")[0]).toEqual(
      stepImages("windows", "edge")[0],
    );
  });

  it("shows the macOS install window whole", () => {
    expect(stepImages("macos", "chrome")[1].imageFit).toBe("contain");
    expect(stepImages("windows", "chrome")[1].imageFit).toBe("cover");
  });
});
