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
  describe("when the user agent names a known browser", () => {
    it.each(Object.entries(UA))(
      "should recognize %s by name, not by engine",
      (name, ua) => {
        expect(detectDownloadBrowser(ua)).toBe(name);
      },
    );
  });

  describe("when the navigator reports Brave", () => {
    it("should recognize Brave even though its UA reads as Chrome", () => {
      expect(detectDownloadBrowser(UA.chrome, true)).toBe("brave");
    });
  });

  describe("when the browser is unknown", () => {
    it("should fall back to Chrome", () => {
      expect(detectDownloadBrowser("curl/8.0")).toBe("chrome");
    });
  });
});

describe("stepImages", () => {
  describe("when showing step 1", () => {
    describe("and the browser is Safari on macOS", () => {
      it("should highlight the browser's download button", () => {
        expect(stepImages("macos", "safari")[0].highlight).toEqual({
          x: 49.37,
          y: 26.25,
        });
      });
    });

    describe("and the browser is Firefox on Windows", () => {
      it("should highlight the browser's download button", () => {
        expect(stepImages("windows", "firefox")[0].highlight).toEqual({
          x: 76.46,
          y: 41.35,
        });
      });
    });

    describe("and the OS has no capture for the browser", () => {
      it("should fall back to the Chrome capture", () => {
        expect(stepImages("windows", "safari")[0]).toEqual(
          stepImages("windows", "chrome")[0],
        );
      });
    });

    describe("and the browser is Edge on macOS", () => {
      it("should reuse the Windows Edge capture", () => {
        expect(stepImages("macos", "edge")[0]).toEqual(
          stepImages("windows", "edge")[0],
        );
      });
    });
  });

  describe("when showing step 2", () => {
    describe("and the OS is macOS", () => {
      it("should show the install window whole", () => {
        expect(stepImages("macos", "chrome")[1].imageFit).toBe("contain");
      });
    });

    describe("and the OS is Windows", () => {
      it("should cover the frame with the install window", () => {
        expect(stepImages("windows", "chrome")[1].imageFit).toBe("cover");
      });
    });
  });
});
