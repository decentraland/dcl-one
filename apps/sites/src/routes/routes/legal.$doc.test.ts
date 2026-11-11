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
});
