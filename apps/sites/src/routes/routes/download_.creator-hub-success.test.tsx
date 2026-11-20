import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it } from "vitest";
import CreatorHubDownloadSuccessRoute from "./download_.creator-hub-success";

function render(loaderData: { sid: string; os: string; retryUrl: string }) {
  const router = createMemoryRouter(
    [{ id: "success", path: "/download/creator-hub-success", Component: CreatorHubDownloadSuccessRoute as never }],
    { initialEntries: ["/download/creator-hub-success"], hydrationData: { loaderData: { success: loaderData } } },
  );
  return renderToString(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

describe("when the Creator Hub download-success page renders", () => {
  it("should show the macOS install steps and retry the given installer", () => {
    const html = render({ sid: "test", os: "macos", retryUrl: "https://example.com/creator-hub.dmg" });
    expect(html).toContain("You&#x27;re almost done!");
    expect(html).toContain("Applications folder");
    expect(html).not.toContain("will automatically start");
    expect(html).toContain('href="https://example.com/creator-hub.dmg"');
  });

  it("should show the Windows install steps for a Windows visitor", () => {
    const html = render({ sid: "test", os: "windows", retryUrl: "https://example.com/creator-hub.exe" });
    expect(html).toContain("will automatically start");
    expect(html).not.toContain("Applications folder");
  });

  it("should offer the latest release when there is no installer to retry", () => {
    const html = render({ sid: "test", os: "linux", retryUrl: "" });
    expect(html).toContain("Applications folder");
    expect(html).toContain('href="https://github.com/decentraland/creator-hub/releases/latest"');
  });
});
