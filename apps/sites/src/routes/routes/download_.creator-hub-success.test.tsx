import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import CreatorHubDownloadSuccessRoute from "./download_.creator-hub-success";

function render(loaderData: { sid: string; os: string; retryUrl: string }) {
  const router = createMemoryRouter(
    [{ id: "success", path: "/download/creator-hub-success", Component: CreatorHubDownloadSuccessRoute as never }],
    { initialEntries: ["/download/creator-hub-success"], hydrationData: { loaderData: { success: loaderData } } },
  );
  return renderToString(<RouterProvider router={router} />);
}

describe("CreatorHubDownloadSuccessRoute", () => {
  let html: string;

  describe("when the visitor is on macOS", () => {
    beforeEach(() => {
      html = render({ sid: "test", os: "macos", retryUrl: "https://example.com/creator-hub.dmg" });
    });

    it("should show the macOS install steps", () => {
      expect(html).toContain("You&#x27;re almost done!");
      expect(html).toContain("Applications folder");
      expect(html).not.toContain("will automatically start");
    });

    it("should retry the given installer", () => {
      expect(html).toContain('href="https://example.com/creator-hub.dmg"');
    });
  });

  describe("when the visitor is on Windows", () => {
    beforeEach(() => {
      html = render({ sid: "test", os: "windows", retryUrl: "https://example.com/creator-hub.exe" });
    });

    it("should show the Windows install steps", () => {
      expect(html).toContain("will automatically start");
      expect(html).not.toContain("Applications folder");
    });
  });

  describe("when there is no installer to retry", () => {
    beforeEach(() => {
      html = render({ sid: "test", os: "linux", retryUrl: "" });
    });

    it("should show the default install steps", () => {
      expect(html).toContain("Applications folder");
    });

    it("should offer the latest release", () => {
      expect(html).toContain('href="https://github.com/decentraland/creator-hub/releases/latest"');
    });
  });
});
