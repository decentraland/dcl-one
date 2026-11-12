// @vitest-environment jsdom
import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { useState } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
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
  vi.unstubAllGlobals();
});
describe("when Explorer mounts the real native Shop without a Sites document provider", () => {
  it("should provide agreed defaults, reuse fresh data and recover after one failed attempt", async () => {
    const { client } = await mountNative();
    expect(screen.queryByText(/No QueryClient set/)).toBeNull();
    expect(screen.getByRole("navigation", { name: "Main" })).toBeTruthy();
    expect(client.getDefaultOptions().queries).toMatchObject({
      retry: false,
      staleTime: 30_000,
    });
    const load = vi.fn(async () => "ready");
    const options = { queryKey: ["native-host-fresh"], queryFn: load };
    await client.fetchQuery(options);
    await client.fetchQuery(options);
    expect(load).toHaveBeenCalledOnce();
    const fail = vi.fn(async () => {
      throw new Error("offline");
    });
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
  it("should retain successful cache entries when a route error is recovered", async () => {
    const { mount, client } = await mountNative();
    client.setQueryData(["keep-through-route-error"], "cached");
    fireEvent.click(screen.getByRole("button", { name: "Fail route" }));
    expect(await screen.findByText("Couldn't open the Shop")).toBeTruthy();
    await act(async () => {
      mount.navigate("/shop/__chrome-contract?recover=1");
    });
    expect(await screen.findByText("Native Shop outlet")).toBeTruthy();
    expect(observed).toBe(client);
    expect(observed!.getQueryData(["keep-through-route-error"])).toBe("cached");
  });
  it("should abort in-flight work and clear the host cache exactly once when unmounted", async () => {
    const { mount, client } = await mountNative();
    let signal!: AbortSignal;
    const pending = client
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
    const abort = vi.fn();
    signal.addEventListener("abort", abort);
    act(() => {
      mount.unmount();
      mount.unmount();
    });
    expect(signal.aborted).toBe(true);
    expect(abort).toHaveBeenCalledOnce();
    expect(client.getQueryCache().getAll()).toHaveLength(0);
    await pending;
  });
});
