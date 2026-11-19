import { expect, test } from "vitest";
import { loader } from "./discover.$";

const location = (url: string) => {
  const response = loader({ request: new Request(url) });
  expect(response.status).toBe(302);
  return response.headers.get("Location");
};

test("/discover lands on /places with its query", () => {
  expect(location("https://example.org/discover")).toBe("/places");
  expect(location("https://example.org/discover?search=music")).toBe("/places?search=music");
});

test("a path under /discover keeps its subpath under /places", () => {
  expect(location("https://example.org/discover/world/foo.dcl.eth?tab=about")).toBe("/places/world/foo.dcl.eth?tab=about");
});
