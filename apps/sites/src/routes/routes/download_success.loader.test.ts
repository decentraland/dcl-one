import { describe, expect, it } from "vitest";
import { loader } from "./download_success";
import { loader as aliasLoader } from "./download_.success";
import { loader as legacyLoader } from "./landings.download_.success";
function read(search = "", cookie = "") {
  return loader({
    request: new Request(`https://site.test/download_success${search}`, {
      headers: { cookie },
    }),
  } as never);
}
describe("Download success loader", () => {
  describe("when locale and platform vary", () => {
    it("should isolate the locale of each request", () => {
      expect(read("?lang=es").locale).toBe("es");
      expect(read("", "lang=fr").locale).toBe("fr");
      expect(read().locale).toBe("en");
    });
    it("should reject prototype keys as locales", () => {
      expect(read("?lang=__proto__").locale).toBe("en");
    });
    it("should preserve the legacy Intel Mac selection", () => {
      expect(read("?os=macos&arch=amd64")).toMatchObject({
        os: "macos",
        arch: "amd64",
      });
    });
  });
  describe("when visiting an older URL", () => {
    it.each([aliasLoader, legacyLoader])(
      "should retain platform parameters in the canonical redirect",
      (fn) => {
        const response = fn({
          request: new Request(
            "https://site.test/download/success?os=windows&arch=amd64",
          ),
        });
        expect(response.status).toBe(302);
        expect(response.headers.get("Location")).toBe(
          "/download_success?os=windows&arch=amd64",
        );
      },
    );
  });
});
