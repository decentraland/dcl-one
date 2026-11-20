import { RouterContextProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";

import { requestSessionContext } from "@/core/lib/telemetry/session";
import { LEGAL_DOCS, type LegalDocument, type LegalKey } from "@/data/legal";

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

    it("should give the page its pinned title, description and canonical link", () => {
      expect(
        meta({ loaderData: result.data } as unknown as Route.MetaArgs),
      ).toEqual([
        { title: doc.pageTitle },
        {
          name: "description",
          content: `Read Decentraland’s ${doc.title}, including the policies and conditions that apply to the community.`,
        },
        { tagName: "link", rel: "canonical", href: `/${doc.slug}` },
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
      expect(
        meta({ loaderData: result.data } as unknown as Route.MetaArgs),
      ).toContainEqual({ tagName: "link", rel: "canonical", href });
    });
  });

  describe("and there is no document", () => {
    it("should describe the legal section", () => {
      expect(
        meta({ loaderData: undefined } as unknown as Route.MetaArgs),
      ).toEqual([
        { title: "Legal | Decentraland" },
        {
          name: "description",
          content: "Decentraland policies and legal documents.",
        },
      ]);
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
