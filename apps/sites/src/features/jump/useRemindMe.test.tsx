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

const attendingReply = (attending: boolean) => http.signedGetJSON.mockResolvedValue({ ok: true, data: { attending } });

import type { AuthIdentity } from "@/data/lib/auth/types";
import { attendanceKeys } from "@/data/lib/catalyst/places/attendance";
import { useRemindMe } from "./useRemindMe";

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

type Setup = ReturnType<typeof setup>;

describe("useRemindMe", () => {
  let identity: AuthIdentity;
  let hook: Setup["hook"];
  let client: Setup["client"];

  beforeEach(() => {
    identity = { signer: "0xAB" } as unknown as AuthIdentity;
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  describe("when the visitor is signed out", () => {
    let onSignIn: Setup["onSignIn"];

    beforeEach(() => {
      ({ hook, onSignIn } = setup(null));
      act(() => hook.result.current.toggle());
    });

    it("should ask the visitor to sign in", () => {
      expect(onSignIn).toHaveBeenCalledOnce();
    });

    it("should read nothing", () => {
      expect(http.signedGetJSON).not.toHaveBeenCalled();
    });

    it("should not show the reminder as set", () => {
      expect(hook.result.current.active).toBe(false);
    });
  });

  describe("when a signed-in member sets the reminder", () => {
    let finish: (value: unknown) => void;

    beforeEach(async () => {
      attendingReply(false);
      finish = () => {};
      http.postJSON.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
      ({ hook } = setup(identity));
      await waitFor(() => expect(hook.result.current.pending).toBe(false));
      act(() => hook.result.current.toggle());
      await waitFor(() => expect(hook.result.current.active).toBe(true));
    });

    describe("and the save is still pending", () => {
      it("should show the flag as set", () => {
        expect(hook.result.current.active).toBe(true);
      });

      it("should save the reminder with a POST", () => {
        expect(http.postJSON).toHaveBeenCalledWith("/events/api/events/e1/attendees", undefined, {
          identity,
          method: "POST",
          signal: undefined,
        });
      });
    });

    describe("and the save settles", () => {
      beforeEach(async () => {
        attendingReply(true);
        await act(async () => finish(undefined));
        await waitFor(() => expect(hook.result.current.pending).toBe(false));
      });

      it("should keep the flag set", () => {
        expect(hook.result.current.active).toBe(true);
      });
    });
  });

  describe("when a signed-in member removes the reminder", () => {
    let readsBefore: number;

    beforeEach(async () => {
      attendingReply(true);
      http.postJSON.mockResolvedValueOnce(undefined);
      ({ hook } = setup(identity));
      await waitFor(() => expect(hook.result.current.active).toBe(true));
      readsBefore = http.signedGetJSON.mock.calls.length;
      attendingReply(false);
      act(() => hook.result.current.toggle());
      await waitFor(() => expect(hook.result.current.pending).toBe(false));
    });

    it("should remove the reminder with a DELETE", () => {
      expect(http.postJSON).toHaveBeenCalledWith("/events/api/events/e1/attendees", undefined, {
        identity,
        method: "DELETE",
        signal: undefined,
      });
    });

    it("should re-read the flag after it settles", async () => {
      await waitFor(() => expect(http.signedGetJSON.mock.calls.length).toBeGreaterThan(readsBefore));
      expect(hook.result.current.active).toBe(false);
    });
  });

  describe("when the save fails", () => {
    beforeEach(async () => {
      attendingReply(true);
      http.postJSON.mockRejectedValueOnce(new Error("401"));
      ({ hook, client } = setup(identity));
      await waitFor(() => expect(hook.result.current.active).toBe(true));
      // The refetch after the failure never answers, so only the rollback can bring `true` back.
      http.signedGetJSON.mockReturnValue(new Promise(() => {}));
      act(() => hook.result.current.toggle());
      await waitFor(() => expect(hook.result.current.failed).toBe(true));
    });

    it("should restore the previous flag", () => {
      expect(client.getQueryData(attendanceKeys.attending("0xab", "e1"))).toBe(true);
    });
  });

  describe("when the account signs out during a failing save", () => {
    let fail: (error: Error) => void;

    beforeEach(async () => {
      attendingReply(false);
      fail = () => {};
      http.postJSON.mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
      ({ hook, client } = setup(identity));
      await waitFor(() => expect(hook.result.current.pending).toBe(false));
      act(() => hook.result.current.toggle());
      await waitFor(() => expect(client.getQueryData(attendanceKeys.attending("0xab", "e1"))).toBe(true));
      // Sign-out as the app orders it: the account leaves the context, then usePrivateQueryLifecycle removes its entries.
      hook.rerender({ who: null });
      client.removeQueries({ queryKey: ["private", "0xab"] });
      await act(async () => fail(new Error("401")));
      await waitFor(() => expect(client.getMutationCache().getAll()[0]?.state.status).toBe("error"));
    });

    it("should not bring back the account's removed flag", () => {
      expect(client.getQueryState(attendanceKeys.attending("0xab", "e1"))).toBeUndefined();
    });

    it("should not report the failure", () => {
      expect(hook.result.current.failed).toBe(false);
    });
  });

  describe("when the member switches account after a failed save", () => {
    let otherIdentity: AuthIdentity;

    beforeEach(async () => {
      otherIdentity = { signer: "0xCD" } as unknown as AuthIdentity;
      attendingReply(false);
      http.postJSON.mockRejectedValueOnce(new Error("401"));
      ({ hook } = setup(identity));
      await waitFor(() => expect(hook.result.current.pending).toBe(false));
      act(() => hook.result.current.toggle());
      await waitFor(() => expect(hook.result.current.failed).toBe(true));
      hook.rerender({ who: otherIdentity });
    });

    it("should not show the other account's failed reminder", () => {
      expect(hook.result.current.failed).toBe(false);
    });
  });

  describe("when the flag cannot be read", () => {
    beforeEach(async () => {
      http.signedGetJSON.mockRejectedValue(new Error("401"));
      ({ hook } = setup(identity));
      await waitFor(() => expect(hook.result.current.pending).toBe(false));
    });

    it("should not report attending", () => {
      expect(hook.result.current.active).toBe(false);
    });
  });

  describe("when keying the flag", () => {
    it("should key it under the member's private entries", () => {
      expect(attendanceKeys.attending("0xAB", "e1")).toEqual(["private", "0xab", "attendance", "e1"]);
    });
  });
});
