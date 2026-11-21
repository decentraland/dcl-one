import { RouterContextProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";

import { requestSessionContext } from "@/core/lib/telemetry/session";
import { LEGAL_DOCS, type LegalDocument, type LegalKey } from "@/data/legal";

import { DEFAULT_SOCIAL_IMAGE } from "../lib/social-meta";
import { canonicalHrefs, rootMatches, tagContent } from "../lib/social-meta.test-utils";
import { loader, meta } from "./$slug";
import type { Route } from "./+types/$slug";

type Wrapped = {
  data: { doc: LegalKey; legal: LegalDocument; sid: string };
  init?: { headers?: HeadersInit };
};

const args = (
  slug: string,
  context = new RouterContextProvider(),
  cookie?: string,
) =>
  ({
    request: new Request(`https://sites.example/${slug}`, {
      headers: cookie ? { cookie } : {},
    }),
    params: { slug },
    context,
  }) as unknown as Route.LoaderArgs;

const headOf = (loaderData: Wrapped["data"] | undefined, base?: string) =>
  meta({ loaderData, matches: rootMatches(base) } as unknown as Route.MetaArgs);

const setCookies = (result: Wrapped) =>
  new Headers(result.init?.headers).getSetCookie();

describe("when resolving canonical legal routes", () => {
  describe.each(Object.values(LEGAL_DOCS))("and the slug is $slug", (doc) => {
    let result: Wrapped;

    beforeEach(async () => {
      const context = new RouterContextProvider();
      context.set(requestSessionContext, {
        sid: "legal-test",
        created: true,
      });

      result = (await loader(args(doc.slug, context))) as unknown as Wrapped;
    });

    it("should load the document with the request's session", () => {
      expect(result.data).toEqual({
        doc: doc.key,
        legal: doc,
        sid: "legal-test",
      });
    });

    it("should set no cookie", () => {
      expect(setCookies(result)).toEqual([]);
    });

    it("should point the card and the canonical link at the document's path", () => {
      const tags = headOf(result.data, "https://public.test");
      expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(tagContent(tags, "twitter:card")).toEqual(["summary_large_image"]);
      expect(tagContent(tags, "og:type")).toEqual(["website"]);
      expect(tagContent(tags, "og:url")).toEqual([`https://public.test/${doc.slug}`]);
      expect(canonicalHrefs(tags)).toEqual([`https://public.test/${doc.slug}`]);
    });

    describe("and the root loader has no public page base", () => {
      it("should keep the card and drop the canonical link", () => {
        const tags = headOf(result.data);
        expect(tagContent(tags, "og:title")).toHaveLength(1);
        expect(tagContent(tags, "og:url")).toEqual([]);
        expect(canonicalHrefs(tags)).toEqual([]);
      });
    });
  });

  describe.each([
    ["brand", "Brand Guidelines | Decentraland", "Logos, colors and usage rules for the Decentraland brand."],
    ["security", "Security | Decentraland", "How Decentraland protects your wallet, identity and content."],
    ["privacy", "Privacy Policy | Decentraland", "How Decentraland handles your personal data."],
    ["terms", "Terms of Use | Decentraland", "The rules for using Decentraland."],
    ["ethics", "Code of Ethics | Decentraland", "The values that guide the Decentraland community."],
    ["content", "Content Policy | Decentraland", "What is and isn’t allowed in Decentraland."],
    ["rewards-terms", "Rewards Terms | Decentraland", "Terms and conditions for Decentraland rewards programs."],
    ["referral-terms", "Referral Program Terms | Decentraland", "Terms and conditions for the Decentraland referral program."],
  ])("and the slug is %s, which decentraland.org gives its own card", (slug, title, description) => {
    it("should carry that card's title and description", async () => {
      const result = (await loader(args(slug))) as unknown as Wrapped;
      const tags = headOf(result.data, "https://public.test");
      expect(tags[0]).toEqual({ title });
      expect(tagContent(tags, "og:title")).toEqual([title]);
      expect(tagContent(tags, "description")).toEqual([description]);
      expect(tagContent(tags, "og:description")).toEqual([description]);
    });
  });

  describe.each(["creator-fund-terms", "credits-terms"])("and the slug is %s, which has no card of its own", (slug) => {
    it("should carry the document's pinned title and description", async () => {
      const result = (await loader(args(slug))) as unknown as Wrapped;
      const tags = headOf(result.data, "https://public.test");
      expect(tags[0]).toEqual({ title: result.data.legal.pageTitle });
      expect(tagContent(tags, "description")).toEqual([
        `Read Decentraland’s ${result.data.legal.title}, including the policies and conditions that apply to the community.`,
      ]);
    });
  });

  describe.each([
    ["rewards", "/rewards-terms"],
    ["referral", "/referral-terms"],
    ["content-policy", "/content"],
  ])("and the slug is the alias %s", (slug, href) => {
    let result: Wrapped;

    beforeEach(async () => {
      result = (await loader(args(slug))) as unknown as Wrapped;
    });

    it(`should point the canonical link at ${href}`, () => {
      expect(canonicalHrefs(headOf(result.data, "https://public.test"))).toEqual([`https://public.test${href}`]);
    });
  });

  describe("and there is no document", () => {
    it("should describe the legal section without a canonical link", () => {
      const tags = headOf(undefined, "https://public.test");
      expect(tags[0]).toEqual({ title: "Legal | Decentraland" });
      expect(tagContent(tags, "description")).toEqual(["Decentraland policies and legal documents."]);
      expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(canonicalHrefs(tags)).toEqual([]);
    });
  });

  describe("and the slug is creator-fund", () => {
    it("should not claim the program's own landing page and answer 404", async () => {
      await expect(loader(args("creator-fund"))).rejects.toMatchObject({
        status: 404,
      });
    });
  });

  describe("and the slug is credits", () => {
    it("should leave it to the Credits API and answer 404", async () => {
      await expect(loader(args("credits"))).rejects.toMatchObject({
        status: 404,
      });
    });
  });

  describe.each(["missing", "__proto__", "constructor"])(
    "and the slug is %s",
    (slug) => {
      it("should return 404", async () => {
        await expect(loader(args(slug))).rejects.toMatchObject({ status: 404 });
      });
    },
  );

  describe("and the slug is seasons", () => {
    let response: unknown;

    beforeEach(async () => {
      response = await loader(args("seasons")).then(
        () => new Error("Expected the seasons route to redirect"),
        (thrown: unknown) => thrown,
      );
    });

    it("should retain the redirect", () => {
      expect(response).toMatchObject({ status: 302 });
    });

    it("should redirect to the credits shop", () => {
      expect((response as Response).headers.get("location")).toBe(
        "/shop/credits",
      );
    });
  });
});

describe("when loading a legal page session", () => {
  let result: Wrapped;

  describe("and the request has no sid", () => {
    beforeEach(async () => {
      result = (await loader(args("terms"))) as unknown as Wrapped;
    });

    it("should mint a sid", () => {
      expect(result.data.sid).toMatch(/.+/);
    });

    it("should return one sid cookie", () => {
      expect(setCookies(result)).toHaveLength(1);
      expect(setCookies(result)[0]).toContain(
        `sid=${encodeURIComponent(result.data.sid)}`,
      );
    });
  });

  describe("and the request has a sid cookie", () => {
    beforeEach(async () => {
      result = (await loader(
        args("terms", new RouterContextProvider(), "sid=existing"),
      )) as unknown as Wrapped;
    });

    it("should retain the existing sid", () => {
      expect(result.data.sid).toBe("existing");
    });

    it("should not set another cookie", () => {
      expect(setCookies(result)).toEqual([]);
    });
  });
});
