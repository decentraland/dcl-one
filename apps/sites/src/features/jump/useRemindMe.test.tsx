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
import { attendanceKeys } from "@/data/lib/catalyst/places/attendance";
import { useRemindMe } from "./useRemindMe";

const identity = { signer: "0xAB" } as unknown as AuthIdentity;

function setup(id: AuthIdentity | null, onSignIn = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const hook = renderHook(({ who }: { who: AuthIdentity | null }) => useRemindMe(who, "e1", onSignIn), {
    wrapper,
    initialProps: { who: id },
  });
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

  it("should show the flag as set while the save is still pending", async () => {
    attendingReply(false);
    let finish: (value: unknown) => void = () => {};
    http.postJSON.mockReturnValue(new Promise((resolve) => (finish = resolve)));
    const { hook } = setup(identity);
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    act(() => hook.result.current.toggle());
    await waitFor(() => expect(hook.result.current.active).toBe(true));
    expect(http.postJSON).toHaveBeenCalledWith("/events/api/events/e1/attendees", undefined, {
      identity,
      method: "POST",
      signal: undefined,
    });
    attendingReply(true);
    await act(async () => finish(undefined));
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    expect(hook.result.current.active).toBe(true);
  });

  it("should remove the reminder with a DELETE and re-read the flag after it settles", async () => {
    attendingReply(true);
    http.postJSON.mockResolvedValue(undefined);
    const { hook } = setup(identity);
    await waitFor(() => expect(hook.result.current.active).toBe(true));
    const readsBefore = http.signedGetJSON.mock.calls.length;
    attendingReply(false);
    act(() => hook.result.current.toggle());
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    expect(http.postJSON).toHaveBeenCalledWith("/events/api/events/e1/attendees", undefined, {
      identity,
      method: "DELETE",
      signal: undefined,
    });
    await waitFor(() => expect(http.signedGetJSON.mock.calls.length).toBeGreaterThan(readsBefore));
    expect(hook.result.current.active).toBe(false);
  });

  it("should restore the previous flag when the save fails", async () => {
    attendingReply(true);
    http.postJSON.mockRejectedValue(new Error("401"));
    const { hook, client } = setup(identity);
    await waitFor(() => expect(hook.result.current.active).toBe(true));
    // The refetch after the failure never answers, so only the rollback can bring `true` back.
    http.signedGetJSON.mockReturnValue(new Promise(() => {}));
    act(() => hook.result.current.toggle());
    await waitFor(() => expect(hook.result.current.failed).toBe(true));
    expect(client.getQueryData(attendanceKeys.attending("0xab", "e1"))).toBe(true);
  });

  it("should not bring back an account's flag removed during a failing save", async () => {
    attendingReply(false);
    let fail: (error: Error) => void = () => {};
    http.postJSON.mockReturnValue(new Promise((_, reject) => (fail = reject)));
    const { hook, client } = setup(identity);
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    act(() => hook.result.current.toggle());
    await waitFor(() => expect(client.getQueryData(attendanceKeys.attending("0xab", "e1"))).toBe(true));
    // Sign-out as the app orders it: the account leaves the context, then usePrivateQueryLifecycle removes its entries.
    hook.rerender({ who: null });
    client.removeQueries({ queryKey: ["private", "0xab"] });
    await act(async () => fail(new Error("401")));
    await waitFor(() => expect(client.getMutationCache().getAll()[0]?.state.status).toBe("error"));
    expect(client.getQueryState(attendanceKeys.attending("0xab", "e1"))).toBeUndefined();
    expect(hook.result.current.failed).toBe(false);
  });

  it("should not show another account's failed reminder after an account switch", async () => {
    attendingReply(false);
    http.postJSON.mockRejectedValue(new Error("401"));
    const { hook } = setup(identity);
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    act(() => hook.result.current.toggle());
    await waitFor(() => expect(hook.result.current.failed).toBe(true));
    hook.rerender({ who: { signer: "0xCD" } as unknown as AuthIdentity });
    expect(hook.result.current.failed).toBe(false);
  });

  it("should not report attending when the flag cannot be read", async () => {
    http.signedGetJSON.mockRejectedValue(new Error("401"));
    const { hook } = setup(identity);
    await waitFor(() => expect(hook.result.current.pending).toBe(false));
    expect(hook.result.current.active).toBe(false);
  });

  it("should key the flag under the member's private entries", () => {
    expect(attendanceKeys.attending("0xAB", "e1")).toEqual(["private", "0xab", "attendance", "e1"]);
  });
});
