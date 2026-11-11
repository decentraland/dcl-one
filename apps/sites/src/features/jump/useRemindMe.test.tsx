// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const http = vi.hoisted(() => ({ signedGetJSON: vi.fn(), postJSON: vi.fn() }));
vi.mock("@/data/lib/catalyst/client", async (actual) => ({
  ...(await actual<typeof import("@/data/lib/catalyst/client")>()),
  signedGetJSON: http.signedGetJSON,
  postJSON: http.postJSON,
}));

const attendingReply = (attending: boolean) => http.signedGetJSON.mockResolvedValue({ ok: true, data: { attending } });

import type { AuthIdentity } from "@/data/lib/auth/types";
import { jumpKeys } from "@/data/lib/catalyst/jump/attendance";
import { useRemindMe } from "./useRemindMe";

const identity = { signer: "0xAB" } as unknown as AuthIdentity;

function setup(id: AuthIdentity | null, onSignIn = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const hook = renderHook(() => useRemindMe(id, "e1", onSignIn), { wrapper });
  return { client, hook, onSignIn };
}

beforeEach(() => {
  http.signedGetJSON.mockReset();
  http.postJSON.mockReset();
});

describe("useRemindMe", () => {
  it("should ask a signed-out visitor to sign in and read nothing", () => {
    const { hook, onSignIn } = setup(null);
    act(() => hook.result.current.toggle());
    expect(onSignIn).toHaveBeenCalledOnce();
    expect(http.signedGetJSON).not.toHaveBeenCalled();
    expect(hook.result.current.active).toBe(false);
  });

  it("should flip the flag optimistically and save it", async () => {
    attendingReply(false);
    http.postJSON.mockResolvedValue(undefined);
    const { hook, client } = setup(identity);
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    attendingReply(true);
    act(() => hook.result.current.toggle());
    await waitFor(() => expect(client.getQueryData(jumpKeys.attending("0xab", "e1"))).toBe(true));
    expect(http.postJSON).toHaveBeenCalledWith("/events/api/events/e1/attendees", undefined, { identity, method: "POST" });
    await waitFor(() => expect(hook.result.current.active).toBe(true));
  });

  it("should roll back and report a failed save", async () => {
    attendingReply(false);
    http.postJSON.mockRejectedValue(new Error("401"));
    const { hook } = setup(identity);
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    act(() => hook.result.current.toggle());
    await waitFor(() => expect(hook.result.current.failed).toBe(true));
    expect(hook.result.current.active).toBe(false);
  });

  it("should not report attending when the flag cannot be read", async () => {
    http.signedGetJSON.mockRejectedValue(new Error("401"));
    const { hook } = setup(identity);
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    expect(hook.result.current.active).toBe(false);
  });

  it("should key the flag under the member's private entries", () => {
    expect(jumpKeys.attending("0xAB", "e1")).toEqual(["private", "0xab", "jump", "attending", "e1"]);
  });
});
