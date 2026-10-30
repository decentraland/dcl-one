import { expect, it, vi } from "vitest";
import { renderToString } from "react-dom/server";

import { useChromeAuth, type ChromeAuth } from "@ui/web/frames/chrome-auth";

import ChromeAuthBridge from "./ChromeAuthBridge";

const signedFetch = vi.fn(async (_url: string, _init?: RequestInit) => new Response('{"notifications":[]}'));

vi.mock("react-router", async (original) => ({
  ...(await original<typeof import("react-router")>()),
  useRouteLoaderData: () => ({ wallet: "0x0000000000000000000000000000000000000001" }),
}));
vi.mock("@/data/lib/auth/index", () => ({
  useAuth: () => ({ isConnected: true, address: "0x0000000000000000000000000000000000000001", fetch: signedFetch, disconnect: vi.fn() }),
}));
vi.mock("@/data/lib/auth/use-profile-name", () => ({ useProfileIdentity: () => ({ name: "", avatarUrl: "" }) }));

// The node mounts the inbox at /notifications (services/catalyst/social/src/lib.rs);
// the bare /notifications route is only nginx's alias for upstream clients.
it("reads the bell's inbox from the node's /notifications mount", async () => {
  let auth: ChromeAuth | undefined;
  function Probe() {
    auth = useChromeAuth();
    return null;
  }
  renderToString(
    <ChromeAuthBridge>
      <Probe />
    </ChromeAuthBridge>,
  );
  await expect(auth?.fetchNotifications?.()).resolves.toEqual({ notifications: [] });
  const url = new URL(signedFetch.mock.calls[0][0]);
  expect(url.pathname).toBe("/notifications/notifications");
  expect(url.searchParams.get("limit")).toBe("50");
});
