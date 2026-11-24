// @vitest-environment jsdom
import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { useState } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ShopHost, ShopMount } from "@ui/shop/native-contract";
import { mountShop } from "./native.client";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
vi.mock("@/data/lib/feature-flags.browser", () => ({
  primeDappsFlags: async () => {},
}));
vi.mock("./native-routes", async (original) => {
  const actual = await original<typeof import("./native-routes")>();
  return {
    ...actual,
    nativeShopRoutes(host: ShopHost) {
      const routes = actual.nativeShopRoutes(host);
      routes[0]!.children!.push({
        path: "/shop/__chrome-contract",
        element: <Probe />,
      });
      return routes;
    },
  };
});
let observed: QueryClient | undefined;
const clients = new Set<QueryClient>();
const fixtures: { mount: ShopMount; container: HTMLElement }[] = [];
function Probe() {
  const client = useQueryClient();
  observed = client;
  clients.add(client);
  const [failed, setFailed] = useState(false);
  if (failed) throw new Error("route failed");
  return (
    <>
      <p>Native Shop outlet</p>
      <button onClick={() => setFailed(true)}>Fail route</button>
    </>
  );
}
async function mountNative() {
  observed = undefined;
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => new Response("{}", { status: 503 })),
  );
  const container = document.createElement("div");
  document.body.append(container);
  let mount!: ShopMount;
  await act(async () => {
    mount = mountShop(container, {
      identity: null,
      path: "/shop/__chrome-contract",
      onNavigate() {},
      onSignIn() {},
      onExternalLink() {},
    });
  });
  fixtures.push({ mount, container });
  expect(await screen.findByText("Native Shop outlet")).toBeTruthy();
  return { mount, client: observed! };
}
describe("when Explorer mounts the real native Shop without a Sites document provider", () => {
  let mount: ShopMount;
  let client: QueryClient;

  beforeEach(async () => {
    ({ mount, client } = await mountNative());
  });

  afterEach(() => {
    act(() =>
      fixtures.splice(0).forEach(({ mount, container }) => {
        mount.unmount();
        container.remove();
      }),
    );
    cleanup();
    clients.forEach((client) => client.clear());
    clients.clear();
    vi.resetAllMocks();
    vi.unstubAllGlobals();
  });

  it("should render the Shop chrome with a query client", () => {
    expect(screen.queryByText(/No QueryClient set/)).toBeNull();
    expect(screen.getByTestId("subnav-tabs").closest("[data-site-layout]")?.querySelector("[data-site-layout-navigation]")).toBeNull();
  });

  it("should provide the agreed query defaults", () => {
    expect(client.getDefaultOptions().queries).toMatchObject({
      retry: false,
      staleTime: 30_000,
    });
  });

  describe("and the same query is fetched twice", () => {
    let load: () => Promise<string>;
    let options: { queryKey: string[]; queryFn: () => Promise<string> };

    beforeEach(async () => {
      load = vi.fn(async () => "ready");
      options = { queryKey: ["native-host-fresh"], queryFn: load };
      await client.fetchQuery(options);
      await client.fetchQuery(options);
    });

    it("should reuse the fresh data", () => {
      expect(load).toHaveBeenCalledOnce();
    });

    describe("and another query fails once", () => {
      let fail: () => Promise<never>;

      beforeEach(() => {
        fail = vi.fn(async () => {
          throw new Error("offline");
        });
      });

      it("should reject with the error, attempt once and recover on the next fetch, keeping the fresh data", async () => {
        await expect(
          client.fetchQuery({ queryKey: ["native-host-error"], queryFn: fail }),
        ).rejects.toThrow("offline");
        expect(fail).toHaveBeenCalledOnce();
        await expect(
          client.fetchQuery({
            queryKey: ["native-host-error"],
            queryFn: async () => "recovered",
          }),
        ).resolves.toBe("recovered");
        expect(client.getQueryData(["native-host-fresh"])).toBe("ready");
      });
    });
  });

  describe("and a route error is recovered", () => {
    beforeEach(async () => {
      client.setQueryData(["keep-through-route-error"], "cached");
      fireEvent.click(screen.getByRole("button", { name: "Fail route" }));
      expect(await screen.findByText("Couldn't open the Shop")).toBeTruthy();
      await act(async () => {
        mount.navigate("/shop/__chrome-contract?recover=1");
      });
    });

    it("should render the route again with the same client and its successful cache entries", async () => {
      expect(await screen.findByText("Native Shop outlet")).toBeTruthy();
      expect(observed).toBe(client);
      expect(observed!.getQueryData(["keep-through-route-error"])).toBe("cached");
    });
  });

  describe("and it is unmounted twice with work in flight", () => {
    let signal: AbortSignal;
    let pending: Promise<unknown>;
    let abort: () => void;

    beforeEach(() => {
      pending = client
        .fetchQuery({
          queryKey: ["pending-native-host"],
          queryFn: ({ signal: next }) => {
            signal = next;
            return new Promise((_resolve, reject) =>
              next.addEventListener("abort", () => reject(new Error("aborted")), {
                once: true,
              }),
            );
          },
        })
        .catch(() => undefined);
      abort = vi.fn();
      signal.addEventListener("abort", abort);
      act(() => {
        mount.unmount();
        mount.unmount();
      });
    });

    afterEach(async () => {
      await pending;
    });

    it("should abort the in-flight work", () => {
      expect(signal.aborted).toBe(true);
    });

    it("should abort exactly once", () => {
      expect(abort).toHaveBeenCalledOnce();
    });

    it("should clear the host cache", () => {
      expect(client.getQueryCache().getAll()).toHaveLength(0);
    });
  });
});
