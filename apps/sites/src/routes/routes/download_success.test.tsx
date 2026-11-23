// @vitest-environment jsdom
import { StrictMode, type ComponentProps } from "react";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import DownloadSuccessRoute from "./download_success";
import { installerSelection } from "@/features/download/installers";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

function data(search = "") {
  const selection = installerSelection(new URLSearchParams(search));
  return {
    ...selection,
    url: `https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.${selection.os === "windows" ? "exe" : "dmg"}`,
    locale:
      new URLSearchParams(search).get("lang") === "es"
        ? ("es" as const)
        : ("en" as const),
    year: 2026,
  };
}
function show(search = "") {
  const loaderData = data(search);
  const router = createMemoryRouter(
    [
      {
        id: "success",
        path: "*",
        loader: ({ request }) => data(new URL(request.url).search),
        element: (
          <DownloadSuccessRoute
            {...({ loaderData } as ComponentProps<typeof DownloadSuccessRoute>)}
          />
        ),
      },
    ],
    {
      initialEntries: [`/download_success${search}`],
      hydrationData: { loaderData: { success: loaderData } },
    },
  );
  return render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  );
}

describe("Download success", () => {
  let requests: string[];
  beforeEach(() => {
    requests = [];
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
      this: HTMLAnchorElement,
    ) {
      requests.push(this.href);
    });
    // The macOS hold sizes the installer with a HEAD request; no test reaches the network.
    vi.stubGlobal("fetch", vi.fn(async () => new Response(null, { status: 404 })));
  });
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  describe("when opening the macOS install page", () => {
    it("should show all three instructions, screenshots and a working retry destination", async () => {
      show("?os=macos");
      await waitFor(() => expect(requests).toHaveLength(1));
      expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(
        "Your download has started!",
      );
      expect(screen.getByRole("heading", { name: "Install" })).toBeTruthy();
      expect(
        screen.getByText(/Double-click the Decentraland icon/),
      ).toBeTruthy();
      expect(
        screen.getByRole("link", { name: "Try again" }).getAttribute("href"),
      ).toBe(requests[0]);
      expect(requests).toEqual([
        "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.dmg",
      ]);
    });
    it("should render the footer with keyboard-operable mobile disclosures", async () => {
      const user = userEvent.setup();
      show();
      const button = screen.getByRole("button", { name: "Getting Started" });
      button.focus();
      await user.keyboard("{Enter}");
      expect(button.getAttribute("aria-expanded")).toBe("true");
      expect(
        screen
          .getAllByRole("link", { name: "Download Decentraland" })
          .every((link) => link.getAttribute("href") === "/download"),
      ).toBe(true);
    });
  });
  describe("when changing language", () => {
    it("should translate without starting the installer again", async () => {
      const user = userEvent.setup();
      show();
      await user.click(
        screen.getByRole("button", { name: "Change language: English" }),
      );
      await user.click(screen.getByRole("option", { name: "Español" }));
      await screen.findByRole("button", { name: "Change language: Español" });
      await waitFor(() => expect(requests).toHaveLength(1));
    });
  });
  describe("when opening the Windows page", () => {
    it("should use Setup Wizard instructions and request the Windows artifact", async () => {
      show("?os=windows&arch=arm64");
      await waitFor(() => expect(requests).toHaveLength(1));
      expect(screen.getByText(/Follow the Setup Wizard/)).toBeTruthy();
      expect(requests).toEqual([
        "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.exe",
      ]);
    });
  });
  describe("when parameters are invalid", () => {
    it("should choose the source macOS defaults without accepting a download host", async () => {
      show("?os=https://evil.test&arch=invalid&url=https://evil.test");
      await waitFor(() => expect(requests).toHaveLength(1));
      expect(requests).toEqual([
        "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.dmg",
      ]);
    });
  });
});
