import { RouterContextProvider } from "react-router";
import { describe, expect, it } from "vitest";

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
  it.each(Object.values(LEGAL_DOCS))(
    "should load $slug and its pinned page title",
    async (doc) => {
      const context = new RouterContextProvider();
      context.set(requestSessionContext, {
        sid: "legal-test",
        created: true,
      });

      const result = (await loader(args(doc.slug, context))) as unknown as Wrapped;

      expect(result.data).toEqual({
        doc: doc.key,
        legal: doc,
        sid: "legal-test",
      });
      expect(setCookies(result)).toEqual([]);
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
    },
  );

  it.each([
    ["rewards", "/rewards-terms"],
    ["referral", "/referral-terms"],
    ["content-policy", "/content"],
  ])("should point the alias %s at its canonical path", async (slug, href) => {
    const result = (await loader(args(slug))) as unknown as Wrapped;

    expect(
      meta({ loaderData: result.data } as unknown as Route.MetaArgs),
    ).toContainEqual({ tagName: "link", rel: "canonical", href });
  });

  it("should describe the legal section when there is no document", () => {
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
    try {
      await loader(args("seasons"));
      throw new Error("Expected the seasons route to redirect");
    } catch (response) {
      expect(response).toMatchObject({ status: 302 });
      expect((response as Response).headers.get("location")).toBe(
        "/shop/credits",
      );
    }
  });
});

describe("when loading a legal page session", () => {
  it("should mint and return one sid cookie when the request has no sid", async () => {
    const result = (await loader(args("terms"))) as unknown as Wrapped;

    expect(result.data.sid).toMatch(/.+/);
    expect(setCookies(result)).toHaveLength(1);
    expect(setCookies(result)[0]).toContain(
      `sid=${encodeURIComponent(result.data.sid)}`,
    );
  });

  it("should retain an existing sid cookie without setting another", async () => {
    const result = (await loader(
      args("terms", new RouterContextProvider(), "sid=existing"),
    )) as unknown as Wrapped;

    expect(result.data.sid).toBe("existing");
    expect(setCookies(result)).toEqual([]);
  });
});
