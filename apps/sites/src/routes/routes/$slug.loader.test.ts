import { describe, expect, it } from "vitest";

import * as legalRedirect from "./legal.$doc";
import * as legalRoute from "./$slug";
import type { Route } from "./+types/$slug";

// Each document: its key, the path production serves it at, and production's page title.
const DOCS = [
  ["brand", "brand", "Brand Guidelines | Decentraland"],
  ["content", "content", "Content Policy | Decentraland"],
  ["ethics", "ethics", "Code of Ethics | Decentraland"],
  ["rewards", "rewards-terms", "Rewards Terms | Decentraland"],
  ["credits", "credits-terms", "Decentraland"],
  ["security", "security", "Security | Decentraland"],
  ["privacy", "privacy", "Privacy Policy | Decentraland"],
  ["referral", "referral-terms", "Referral Program Terms | Decentraland"],
  ["terms", "terms", "Terms of Use | Decentraland"],
] as const;

type Loaded = {
  data: { doc: string; sid: string; messages: { locale: string } };
};

const load = (slug: string, headers: Record<string, string> = {}) =>
  legalRoute.loader({
    request: new Request(`http://localhost/${slug}`, { headers }),
    params: { slug },
    context: {},
  } as unknown as Route.LoaderArgs) as Promise<unknown>;

const thrown = (promise: Promise<unknown>) =>
  promise.then(
    () => null,
    (response: unknown) => response as Response,
  );

describe("the legal routes", () => {
  const paths: [slug: string, key: string][] = [
    ...DOCS.map(([key, slug]): [string, string] => [slug, key]),
    ["rewards", "rewards"],
    ["referral", "referral"],
    ["credits", "credits"],
    ["content-policy", "content"],
  ];
  describe.each(paths)("when /%s is requested", (slug, key) => {
    it(`should serve the ${key} document`, async () => {
      const result = (await load(slug)) as Loaded;

      expect(result.data.doc).toBe(key);
    });
  });

  describe("when /seasons is requested", () => {
    it("should redirect to the credits shop", async () => {
      const response = await thrown(load("seasons"));

      expect(response?.status).toBe(302);
      expect(response?.headers.get("Location")).toBe("/shop/credits");
    });
  });

  describe("when the slug names no document", () => {
    it("should answer 404", async () => {
      expect((await thrown(load("nope")))?.status).toBe(404);
    });
  });

  describe("when a /legal/:doc path is requested", () => {
    it("should redirect permanently to the document's own path", async () => {
      const response = await thrown(
        Promise.resolve().then(() =>
          legalRedirect.loader({ params: { doc: "credits-terms" } } as never),
        ),
      );

      expect(response?.status).toBe(301);
      expect(response?.headers.get("Location")).toBe("/credits-terms");
    });
  });

  describe.each(DOCS)(
    "when the page title of %s is read",
    (key, _slug, title) => {
      it(`should be "${title}", as production`, () => {
        const tags = legalRoute.meta({
          loaderData: { doc: key },
        } as unknown as Route.MetaArgs) as { title?: string }[];

        expect(tags.find((tag) => tag.title !== undefined)?.title).toBe(title);
      });
    },
  );

  // The page chrome follows the visitor's language, resolved as /download resolves it.
  describe("when the language cookie names Spanish", () => {
    it("should load the Spanish chrome strings", async () => {
      const result = (await load("privacy", {
        cookie: "dcl-locale=es",
      })) as Loaded;

      expect(result.data.messages.locale).toBe("es");
    });
  });

  describe("when only the browser's languages name French", () => {
    it("should load the French chrome strings", async () => {
      const result = (await load("privacy", {
        "accept-language": "fr-FR,fr;q=0.9,en;q=0.5",
      })) as Loaded;

      expect(result.data.messages.locale).toBe("fr");
    });
  });
});
