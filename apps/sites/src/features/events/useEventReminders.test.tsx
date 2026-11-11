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

import type { AuthIdentity } from "@/data/lib/auth/types";
import { attendanceKeys } from "@/data/lib/catalyst/places/attendance";
import { useEventReminders } from "./useEventReminders";

const ana = { signer: "0xAB" } as unknown as AuthIdentity;
const bob = { signer: "0xCD" } as unknown as AuthIdentity;
/** The endpoint answers every unfinished event with the member's flag; only the flagged ones are reminders. */
const attending = (...ids: string[]) => ({
  ok: true,
  data: [
    ...ids.map((id) => ({ id, name: id, attending: true })),
    { id: "other", name: "other", attending: false },
    { id: "b-not", name: "b-not", attending: false },
  ],
});

function setup(who: AuthIdentity | null, onSignIn = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const hook = renderHook(({ id }: { id: AuthIdentity | null }) => useEventReminders(id, onSignIn), {
    wrapper,
    initialProps: { id: who },
  });
  return { client, hook, onSignIn };
}

/** Writes that wait until the test settles them, one per event. */
function deferredWrites() {
  const settle = new Map<string, { ok: () => void; fail: () => void }>();
  http.postJSON.mockImplementation(
    (path: string) =>
      new Promise((resolve, reject) => {
        const id = path.split("/")[4]!;
        settle.set(id, { ok: () => resolve(undefined), fail: () => reject(new Error("401")) });
      }),
  );
  return settle;
}

beforeEach(() => {
  http.signedGetJSON.mockReset();
  http.postJSON.mockReset();
});

describe("useEventReminders", () => {
  it("should read every card's flag with one signed request to the attending list", async () => {
    http.signedGetJSON.mockResolvedValue(attending("a", "c"));
    const { hook } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    expect(["a", "b", "c", "other", "b-not"].map((id) => hook.result.current.get(id).active)).toEqual([
      true,
      false,
      true,
      false,
      false,
    ]);
    expect(http.signedGetJSON).toHaveBeenCalledOnce();
    expect(http.signedGetJSON.mock.calls[0]?.[0]).toBe("/events/api/events/attending");
  });

  it("should read nothing signed out and ask to sign in instead of writing", () => {
    const { hook, onSignIn } = setup(null);
    act(() => hook.result.current.get("a").toggle());
    expect(onSignIn).toHaveBeenCalledOnce();
    expect(http.signedGetJSON).not.toHaveBeenCalled();
    expect(http.postJSON).not.toHaveBeenCalled();
  });

  it("should add the event to the list at once and write it with a POST", async () => {
    http.signedGetJSON.mockResolvedValue(attending());
    const writes = deferredWrites();
    const { hook } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    act(() => hook.result.current.get("a").toggle());
    await waitFor(() => expect(hook.result.current.get("a").active).toBe(true));
    expect(hook.result.current.get("a").pending).toBe(true);
    expect(http.postJSON).toHaveBeenCalledWith("/events/api/events/a/attendees", undefined, {
      identity: ana,
      method: "POST",
      signal: undefined,
    });
    http.signedGetJSON.mockResolvedValue(attending("a"));
    await act(async () => writes.get("a")!.ok());
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    expect(hook.result.current.get("a").active).toBe(true);
  });

  it("should put the event back when the write fails and name it as failed", async () => {
    http.signedGetJSON.mockResolvedValue(attending("a"));
    http.postJSON.mockRejectedValue(new Error("401"));
    const { hook } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").active).toBe(true));
    // The re-read after the failure never answers, so only the rollback can bring the flag back.
    http.signedGetJSON.mockReturnValue(new Promise(() => {}));
    act(() => hook.result.current.get("a").toggle());
    await waitFor(() => expect(hook.result.current.failed).toBe("a"));
    expect(hook.result.current.get("a").active).toBe(true);
  });

  it("should keep two concurrent toggles apart: each settles and rolls back on its own", async () => {
    http.signedGetJSON.mockResolvedValue(attending());
    const writes = deferredWrites();
    const { hook, client } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    act(() => hook.result.current.get("a").toggle());
    act(() => hook.result.current.get("b").toggle());
    await waitFor(() => expect(client.getQueryData(attendanceKeys.list("0xab"))).toEqual(["a", "b"]));
    http.signedGetJSON.mockReturnValue(new Promise(() => {}));
    await act(async () => writes.get("a")!.fail());
    await waitFor(() => expect(hook.result.current.failed).toBe("a"));
    // A failed while B is still in flight: only A goes back, and B is still pending.
    expect(client.getQueryData(attendanceKeys.list("0xab"))).toEqual(["b"]);
    expect(hook.result.current.get("a").pending).toBe(false);
    expect(hook.result.current.get("b").pending).toBe(true);
    await act(async () => writes.get("b")!.ok());
    await waitFor(() => expect(hook.result.current.get("b").pending).toBe(false));
    // B's success replaces A's failure notice.
    expect(hook.result.current.failed).toBeNull();
  });

  it("should clear the failure notice when the member tries again", async () => {
    http.signedGetJSON.mockResolvedValue(attending());
    const writes = deferredWrites();
    const { hook } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    http.signedGetJSON.mockReturnValue(new Promise(() => {}));
    act(() => hook.result.current.get("a").toggle());
    await waitFor(() => expect(writes.has("a")).toBe(true));
    await act(async () => writes.get("a")!.fail());
    await waitFor(() => expect(hook.result.current.failed).toBe("a"));
    act(() => hook.result.current.get("c").toggle());
    await waitFor(() => expect(hook.result.current.failed).toBeNull());
  });

  it("should re-read the list only after the last of several writes settles", async () => {
    http.signedGetJSON.mockResolvedValue(attending());
    const writes = deferredWrites();
    const { hook, client } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    const reads = () => http.signedGetJSON.mock.calls.length;
    const before = reads();
    act(() => hook.result.current.get("a").toggle());
    act(() => hook.result.current.get("b").toggle());
    await waitFor(() => expect(client.getQueryData(attendanceKeys.list("0xab"))).toEqual(["a", "b"]));
    await waitFor(() => expect(writes.has("b")).toBe(true));
    await act(async () => writes.get("a")!.ok());
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    // B is still in flight: a re-read now would drop its optimistic flag.
    expect(reads()).toBe(before);
    expect(client.getQueryData(attendanceKeys.list("0xab"))).toEqual(["a", "b"]);
    http.signedGetJSON.mockResolvedValue(attending("a", "b"));
    await act(async () => writes.get("b")!.ok());
    await waitFor(() => expect(reads()).toBe(before + 1));
  });

  it("should not show one account's list or failure to the next", async () => {
    http.signedGetJSON.mockResolvedValue(attending("a"));
    http.postJSON.mockRejectedValue(new Error("401"));
    const { hook } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").active).toBe(true));
    act(() => hook.result.current.get("b").toggle());
    await waitFor(() => expect(hook.result.current.failed).toBe("b"));
    http.signedGetJSON.mockResolvedValue(attending());
    hook.rerender({ id: bob });
    expect(hook.result.current.failed).toBeNull();
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    expect(hook.result.current.get("a").active).toBe(false);
  });

  it("should not bring back a list removed by a sign-out during a failing write", async () => {
    http.signedGetJSON.mockResolvedValue(attending());
    const writes = deferredWrites();
    const { hook, client } = setup(ana);
    await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
    act(() => hook.result.current.get("a").toggle());
    await waitFor(() => expect(client.getQueryData(attendanceKeys.list("0xab"))).toEqual(["a"]));
    hook.rerender({ id: null });
    client.removeQueries({ queryKey: ["private", "0xab"] });
    await act(async () => writes.get("a")!.fail());
    await waitFor(() => expect(client.getMutationCache().getAll()[0]?.state.status).toBe("error"));
    expect(client.getQueryState(attendanceKeys.list("0xab"))).toBeUndefined();
    expect(hook.result.current.failed).toBeNull();
  });
});
