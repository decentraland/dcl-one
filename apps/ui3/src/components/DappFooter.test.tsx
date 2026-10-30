import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import DappFooter from "./DappFooter";

describe("DappFooter", () => {
  it("renders identical same-origin policy links on the server and browser", () => {
    const browser = renderToStaticMarkup(<DappFooter />);
    let server: string;
    vi.stubGlobal("window", undefined);
    try {
      server = renderToStaticMarkup(<DappFooter />);
    } finally {
      vi.unstubAllGlobals();
    }
    expect(server).toBe(browser);
    const document = new DOMParser().parseFromString(server, "text/html");
    expect(Array.from(document.querySelectorAll(".dappfooter__links a"), link => link.getAttribute("href")))
      .toEqual(["/", "/privacy", "/terms", "/content-policy", "/ethics"]);
  });
});
