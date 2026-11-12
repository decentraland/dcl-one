// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
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
    let client: QueryClient | undefined;
    function Probe() {
      client = useQueryClient();
      return <p>Native Shop outlet</p>;
    }
    const routes = nativeShopRoutes({
      identity: null,
      path: "/shop/__chrome-contract",
      onNavigate() {},
      onSignIn() {},
      onExternalLink() {},
    });
    routes[0]!.children!.push({
      path: "/shop/__chrome-contract",
      element: <Probe />,
    });
    const router = createMemoryRouter(routes, {
      initialEntries: ["/shop/__chrome-contract"],
    });
    try {
      render(<RouterProvider router={router} />);
      expect(await screen.findByText("Native Shop outlet")).toBeTruthy();
      expect(screen.queryByText(/No QueryClient set/)).toBeNull();
      expect(client!.getDefaultOptions().queries).toMatchObject({
        retry: false,
        staleTime: 30_000,
      });
      const load = vi.fn(async () => "ready");
      const options = { queryKey: ["native-host-fresh"], queryFn: load };
      await client!.fetchQuery(options);
      await client!.fetchQuery(options);
      expect(load).toHaveBeenCalledOnce();
      const fail = vi.fn(async () => {
        throw new Error("offline");
      });
      await expect(
        client!.fetchQuery({ queryKey: ["native-host-error"], queryFn: fail }),
      ).rejects.toThrow("offline");
      expect(fail).toHaveBeenCalledOnce();
      await expect(
        client!.fetchQuery({
          queryKey: ["native-host-error"],
          queryFn: async () => "recovered",
        }),
      ).resolves.toBe("recovered");
      expect(client!.getQueryData(["native-host-fresh"])).toBe("ready");
      expect(screen.getByRole("navigation", { name: "Main" })).toBeTruthy();
    } finally {
      router.dispose();
      client?.clear();
    }
  });
});
