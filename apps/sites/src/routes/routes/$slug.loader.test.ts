import { describe, expect, it, vi } from "vitest";
vi.mock("@/core/lib/telemetry/page-loader", () => ({
  pageLoader: async () => ({ sid: "legal-test", wrap: <T>(value: T) => value }),
}));
import { loader, meta } from "./$slug";
import { LEGAL_DOCS } from "@/data/legal";
import type { Route } from "./+types/$slug";
const args = (slug: string) =>
  ({
    request: new Request(`https://sites.example/${slug}`),
    params: { slug },
  }) as Route.LoaderArgs;
describe("when resolving canonical legal routes", () => {
  it.each(Object.values(LEGAL_DOCS))(
    "should load $slug and its pinned page title",
    async (doc) => {
      const result = await loader(args(doc.slug));
      expect(result).toEqual({ doc: doc.key, sid: "legal-test" });
      expect(
        meta({ loaderData: result } as unknown as Route.MetaArgs)[0],
      ).toEqual({
        title: doc.pageTitle,
      });
    },
  );
  it("should leave /credits to the Credits API", async () => {
    await expect(loader(args("credits"))).rejects.toMatchObject({
      status: 404,
    });
  });
  it.each(["missing", "__proto__", "constructor"])(
    "should return 404 for %s",
    async (slug) => {
      await expect(loader(args(slug))).rejects.toMatchObject({ status: 404 });
    },
  );
  it("should retain the seasons redirect", async () => {
    await expect(loader(args("seasons"))).rejects.toMatchObject({
      status: 302,
    });
    try {
      await loader(args("seasons"));
    } catch (response) {
      expect((response as Response).headers.get("location")).toBe(
        "/shop/credits",
      );
    }
  });
});
