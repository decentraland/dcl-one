// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { subscribeSiteNewsletter } from "./newsletter";

afterEach(() => vi.unstubAllGlobals());

describe("when a visitor subscribes to the newsletter", () => {
  describe("and the service accepts it", () => {
    it("should let go of the answer's body and resolve", async () => {
      const accepted = new Response(JSON.stringify({ ok: true }), { status: 200 });
      const cancel = vi.spyOn(accepted.body!, "cancel");
      vi.stubGlobal("fetch", vi.fn().mockResolvedValue(accepted));

      await expect(subscribeSiteNewsletter("a@example.com")).resolves.toBeUndefined();

      expect(cancel).toHaveBeenCalledOnce();
    });

    it("should resolve when the answer has no body", async () => {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 204 })));

      await expect(subscribeSiteNewsletter("a@example.com")).resolves.toBeUndefined();
    });

    it("should still resolve when letting go of the body fails", async () => {
      const accepted = new Response("ok", { status: 200 });
      vi.spyOn(accepted.body!, "cancel").mockRejectedValue(new Error("stream locked"));
      vi.stubGlobal("fetch", vi.fn().mockResolvedValue(accepted));

      await expect(subscribeSiteNewsletter("a@example.com")).resolves.toBeUndefined();
    });
  });

  describe("and the service refuses it", () => {
    it("should let go of the body and reject with the status", async () => {
      const refused = new Response("nope", { status: 422 });
      const cancel = vi.spyOn(refused.body!, "cancel");
      vi.stubGlobal("fetch", vi.fn().mockResolvedValue(refused));

      await expect(subscribeSiteNewsletter("a@example.com")).rejects.toThrow("Newsletter request returned 422");

      expect(cancel).toHaveBeenCalledOnce();
    });
  });
});
