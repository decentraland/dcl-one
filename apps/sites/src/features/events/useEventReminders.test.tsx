// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

let ana: AuthIdentity;
let bob: AuthIdentity;
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
  ana = { signer: "0xAB" } as unknown as AuthIdentity;
  bob = { signer: "0xCD" } as unknown as AuthIdentity;
});

afterEach(() => {
  vi.resetAllMocks();
});

describe("useEventReminders", () => {
  describe("when the member is signed out", () => {
    let hook: ReturnType<typeof setup>["hook"];
    let onSignIn: ReturnType<typeof setup>["onSignIn"];

    beforeEach(() => {
      ({ hook, onSignIn } = setup(null));
      act(() => hook.result.current.get("a").toggle());
    });

    it("should ask to sign in on toggle", () => {
      expect(onSignIn).toHaveBeenCalledOnce();
    });

    it("should read nothing", () => {
      expect(http.signedGetJSON).not.toHaveBeenCalled();
    });

    it("should write nothing", () => {
      expect(http.postJSON).not.toHaveBeenCalled();
    });
  });

  describe("when the member is signed in", () => {
    describe("and the attending list flags some events", () => {
      let hook: ReturnType<typeof setup>["hook"];

      beforeEach(async () => {
        http.signedGetJSON.mockResolvedValue(attending("a", "c"));
        ({ hook } = setup(ana));
        await waitFor(() => expect(hook.result.current.get("a").pending).toBe(false));
      });

      it("should mark only the flagged events as reminders", () => {
        expect(["a", "b", "c", "other", "b-not"].map((id) => hook.result.current.get(id).active)).toEqual([
          true,
          false,
          true,
          false,
          false,
        ]);
      });

      it("should read every card's flag with one signed request to the attending list", () => {
        expect(http.signedGetJSON).toHaveBeenCalledOnce();
        expect(http.signedGetJSON.mock.calls[0]?.[0]).toBe("/events/api/events/attending");
      });
    });

    describe("and the member toggles an event on", () => {
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
    });

    describe("and the write fails", () => {
      beforeEach(() => {
        http.signedGetJSON.mockResolvedValue(attending("a"));
        http.postJSON.mockRejectedValue(new Error("401"));
      });

      it("should put the event back and name it as failed", async () => {
        const { hook } = setup(ana);
        await waitFor(() => expect(hook.result.current.get("a").active).toBe(true));
        // The re-read after the failure never answers, so only the rollback can bring the flag back.
        http.signedGetJSON.mockReturnValue(new Promise(() => {}));
        act(() => hook.result.current.get("a").toggle());
        await waitFor(() => expect(hook.result.current.failed).toBe("a"));
        expect(hook.result.current.get("a").active).toBe(true);
      });

      describe("and the account changes", () => {
        it("should not show the previous account's list or failure to the next", async () => {
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
      });
    });

    describe("and writes are settled by the test", () => {
      let writes: ReturnType<typeof deferredWrites>;

      beforeEach(() => {
        http.signedGetJSON.mockResolvedValue(attending());
        writes = deferredWrites();
      });

      describe("and two toggles run concurrently", () => {
        it("should keep them apart: each settles and rolls back on its own", async () => {
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

        it("should re-read the list only after the last write settles", async () => {
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
      });

      describe("and a write failed and the member tries again", () => {
        it("should clear the failure notice", async () => {
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
      });

      describe("and the member signs out during a failing write", () => {
        it("should not bring back the removed list", async () => {
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
    });
  });
});
