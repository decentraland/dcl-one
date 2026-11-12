// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { nativeShopRoutes } from "./native-routes";
vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
describe("when Explorer mounts the native Shop without a Sites document provider", () => {
  it("should render its real chrome and outlet without requiring an outer QueryClient", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response("{}", { status: 503 })),
    );
    const routes = nativeShopRoutes({
      identity: null,
      path: "/shop/__chrome-contract",
      onNavigate() {},
      onSignIn() {},
      onExternalLink() {},
    });
    routes[0]!.children!.push({
      path: "/shop/__chrome-contract",
      element: <p>Native Shop outlet</p>,
    });
    const router = createMemoryRouter(routes, {
      initialEntries: ["/shop/__chrome-contract"],
    });
    try {
      render(<RouterProvider router={router} />);
      expect(await screen.findByText("Native Shop outlet")).toBeTruthy();
      expect(screen.queryByText(/No QueryClient set/)).toBeNull();
      expect(screen.getByRole("navigation", { name: "Main" })).toBeTruthy();
    } finally {
      router.dispose();
    }
  });
});
