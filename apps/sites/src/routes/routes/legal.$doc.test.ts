import { createStaticHandler } from "react-router";
import { describe, expect, it } from "vitest";
import { loader } from "./legal.$doc";
import type { Route } from "./+types/legal.$doc";

const args = (doc: string) => ({ params: { doc } }) as Route.LoaderArgs;

async function loadRedirect(doc: string) {
  try {
    await loader(args(doc));
  } catch (response) {
    return response as Response;
  }
  throw new Error("Expected the legacy legal route to redirect");
}

describe("when redirecting legacy legal routes", () => {
  it.each([
    ["credits", "/credits-terms"],
    ["content", "/content"],
    ["rewards", "/rewards-terms"],
  ])("should redirect %s to its canonical route", async (doc, location) => {
    const response = await loadRedirect(doc);
    expect(response.status).toBe(301);
    expect(response.headers.get("location")).toBe(location);
  });

  it.each(["%5Cevil.example", "%2Fevil.example"])(
    "should keep encoded path separators on the same origin for %s",
    async (encodedDoc) => {
      const handler = createStaticHandler([{ path: "/legal/:doc", loader: loader as never }]);
      const response = await handler.query(new Request(`https://sites.test/legal/${encodedDoc}`));

      expect(response).toBeInstanceOf(Response);
      expect((response as Response).status).toBe(301);
      expect((response as Response).headers.get("location")).toBe("/");
      const location = (response as Response).headers.get("location");
      expect(location).toBeTruthy();
      expect(new URL(location!, "https://sites.test").origin).toBe("https://sites.test");
    }
  );

  it.each(["", "missing", "__proto__", "constructor", "\\evil.example", "/evil.example"])(
    "should send unknown or unsafe slug %j to the home page",
    async (doc) => {
      const response = await loadRedirect(doc);
      expect(response.status).toBe(301);
      expect(response.headers.get("location")).toBe("/");
    }
  );
});
