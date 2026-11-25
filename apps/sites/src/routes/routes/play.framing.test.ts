import { describe, expect, it } from "vitest";

import { headers as appHeaders } from "./play_.app[.]html";
import { headers as shellHeaders } from "./play";

// /play keeps the sign-in key and draws the buttons an in-app wallet signs with no window of its own; a page of
// another site must not frame it, nor the app document it frames itself.
describe("the web Explorer's documents", () => {
  it.each([
    ["/play", shellHeaders],
    ["/play/app.html", appHeaders],
  ])("%s may be framed only by this origin", (_path, headers) => {
    expect(headers()).toMatchObject({
      "Content-Security-Policy": "frame-ancestors 'self'",
      "X-Frame-Options": "SAMEORIGIN",
    });
  });

  it("keeps the app document's isolation alongside", () => {
    expect(appHeaders()).toMatchObject({
      "Cross-Origin-Opener-Policy": "same-origin",
      "Document-Isolation-Policy": "isolate-and-credentialless",
    });
  });
});
