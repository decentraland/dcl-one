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
let rerender: () => void;
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
  vi.resetAllMocks();
});

describe("the Sites chrome's private cache", () => {
  describe("when the signed-in account hydrates", () => {
    beforeEach(() => {
      mount();
    });

    it("should keep the signed-in account's entries", () => {
      expect(client.getQueryData(["private", A, "account", "subscription"])).toBe("A's subscription");
    });
  });

  describe("when another account signs in", () => {
    beforeEach(() => {
      rerender = mount();
      client.setQueryData(["private", B, "account", "subscription"], "B's subscription");

      auth.state = { isConnected: true, address: B };
      act(() => rerender());
    });

    it("should remove the previous account's entries", () => {
      expect(client.getQueriesData({ queryKey: ["private", A] })).toEqual([]);
    });

    it("should keep the new account's entries", () => {
      expect(client.getQueryData(["private", B, "account", "subscription"])).toBe("B's subscription");
    });

    it("should keep public data", () => {
      expect(client.getQueryData(["events", "list"])).toBe("public events");
    });
  });

  describe("when the account signs out", () => {
    beforeEach(() => {
      rerender = mount();

      auth.state = { isConnected: false, address: null };
      act(() => rerender());
    });

    it("should remove every private entry", () => {
      expect(client.getQueriesData({ queryKey: ["private"] })).toEqual([]);
    });

    it("should keep public data", () => {
      expect(client.getQueryData(["events", "list"])).toBe("public events");
    });
  });
});
