// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const auth = vi.hoisted(() => ({
  state: { isConnected: true, address: "0x00000000000000000000000000000000000000AA" as string | null },
}));
vi.mock("react-router", async (original) => ({
  ...(await original<typeof import("react-router")>()),
  useRouteLoaderData: () => ({ wallet: "0x00000000000000000000000000000000000000aa" }),
}));
vi.mock("@/data/lib/auth/index", () => ({
  useAuth: () => ({ ...auth.state, fetch: vi.fn(), disconnect: vi.fn() }),
}));
vi.mock("@/data/lib/auth/use-profile-name", () => ({ useProfileIdentity: () => ({ name: "", avatarUrl: "" }) }));

import ChromeAuthBridge from "./ChromeAuthBridge";

const A = "0x00000000000000000000000000000000000000aa";
const B = "0x00000000000000000000000000000000000000bb";

let client: QueryClient;
function mount() {
  const view = render(
    <QueryClientProvider client={client}>
      <ChromeAuthBridge>{null}</ChromeAuthBridge>
    </QueryClientProvider>,
  );
  return () =>
    view.rerender(
      <QueryClientProvider client={client}>
        <ChromeAuthBridge>{null}</ChromeAuthBridge>
      </QueryClientProvider>,
    );
}

beforeEach(() => {
  auth.state = { isConnected: true, address: A.toUpperCase().replace("0X", "0x") };
  client = new QueryClient();
  client.setQueryData(["private", A, "account", "subscription"], "A's subscription");
  client.setQueryData(["events", "list"], "public events");
});
afterEach(() => {
  cleanup();
  client.clear();
});

describe("the Sites chrome's private cache", () => {
  it("keeps the signed-in account's entries through hydration", () => {
    mount();

    expect(client.getQueryData(["private", A, "account", "subscription"])).toBe("A's subscription");
  });

  it("removes the previous account's entries when another account signs in", () => {
    const rerender = mount();
    client.setQueryData(["private", B, "account", "subscription"], "B's subscription");

    auth.state = { isConnected: true, address: B };
    act(() => rerender());

    expect(client.getQueriesData({ queryKey: ["private", A] })).toEqual([]);
    expect(client.getQueryData(["private", B, "account", "subscription"])).toBe("B's subscription");
    expect(client.getQueryData(["events", "list"])).toBe("public events");
  });

  it("removes every private entry on sign-out and keeps public data", () => {
    const rerender = mount();

    auth.state = { isConnected: false, address: null };
    act(() => rerender());

    expect(client.getQueriesData({ queryKey: ["private"] })).toEqual([]);
    expect(client.getQueryData(["events", "list"])).toBe("public events");
  });
});
