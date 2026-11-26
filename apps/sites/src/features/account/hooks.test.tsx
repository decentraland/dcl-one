// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const api = vi.hoisted(() => ({
  fetchSubscription: vi.fn(),
  putSubscription: vi.fn(),
  putEmail: vi.fn(),
  fetchCreditsStatus: vi.fn(),
  optOutOfCredits: vi.fn(),
}));
vi.mock("@/data/lib/account/notifications", () => ({
  fetchSubscription: api.fetchSubscription,
  putSubscription: api.putSubscription,
  putEmail: api.putEmail,
}));
vi.mock("@/data/lib/account/credits", () => ({
  fetchCreditsStatus: api.fetchCreditsStatus,
  optOutOfCredits: api.optOutOfCredits,
}));

import type { AuthIdentity } from "@/data/lib/auth/types";
import type { SubscriptionDetails, SubscriptionResponse } from "@/data/lib/account/notifications";
import {
  ACCOUNT_STALE_MS,
  accountKeys,
  creditsStatusQuery,
  subscriptionQuery,
} from "@/data/lib/account/queries";
import {
  useCreditsStatus,
  useOptOutOfCredits,
  useSetEmail,
  useSubscription,
  useUpdateSubscription,
} from "./hooks";

const SIGNER = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";
const OTHER = "0x1111111111111111111111111111111111111111";

let identity: AuthIdentity;
let other: AuthIdentity;
let details: SubscriptionDetails;
let muted: SubscriptionDetails;
let subscription: SubscriptionResponse;
let otherSubscription: SubscriptionResponse;
let subscriptionKey: ReturnType<typeof accountKeys.subscription>;
let otherSubscriptionKey: ReturnType<typeof accountKeys.subscription>;
let creditsKey: ReturnType<typeof accountKeys.creditsStatus>;

let client: QueryClient;
function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

type Deferred = ReturnType<typeof deferred>;

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  identity = {
    signer: SIGNER,
    ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
    expiration: "2999-01-01T00:00:00.000Z",
    authChain: [],
  };
  other = { ...identity, signer: OTHER };
  details = {
    ignore_all_email: false,
    ignore_all_in_app: false,
    message_type: { item_sold: { email: true, in_app: true } },
  };
  muted = { ...details, ignore_all_email: true };
  subscription = { address: SIGNER, email: "jane.doe@example.com", details };
  otherSubscription = { address: OTHER, email: "john.doe@example.com", details };
  subscriptionKey = accountKeys.subscription(SIGNER);
  otherSubscriptionKey = accountKeys.subscription(OTHER);
  creditsKey = accountKeys.creditsStatus(SIGNER);
});
afterEach(() => {
  client.clear();
  vi.resetAllMocks();
});

describe("account keys", () => {
  describe("when built for a mixed-case account", () => {
    it("should keep the subscription under the lowercased account's private prefix", () => {
      expect(subscriptionKey).toEqual(["private", SIGNER.toLowerCase(), "account", "subscription"]);
    });

    it("should keep the credits status under the lowercased account's private prefix", () => {
      expect(creditsKey).toEqual(["private", SIGNER.toLowerCase(), "account", "credits-status"]);
    });
  });

  describe("when their queries are built", () => {
    it("should declare the subscription's freshness, so loader data and the page agree on it", () => {
      expect(subscriptionQuery(identity).staleTime).toBe(ACCOUNT_STALE_MS);
    });

    it("should declare the credits status's freshness, so loader data and the page agree on it", () => {
      expect(creditsStatusQuery(identity).staleTime).toBe(ACCOUNT_STALE_MS);
    });
  });
});

describe("read hooks", () => {
  describe("when they read signed in and signed out", () => {
    beforeEach(() => {
      api.fetchSubscription.mockReturnValue(new Promise(() => {}));
      api.fetchCreditsStatus.mockReturnValue(new Promise(() => {}));
      renderHook(() => useSubscription(identity), { wrapper });
      renderHook(() => useCreditsStatus(identity), { wrapper });
      renderHook(() => useSubscription(null), { wrapper });
    });

    it("should read with the factory's freshness", () => {
      const staleTimes = client
        .getQueryCache()
        .getAll()
        .flatMap((query) => query.observers.map((observer) => observer.options.staleTime));
      expect(staleTimes).toEqual([ACCOUNT_STALE_MS, ACCOUNT_STALE_MS, ACCOUNT_STALE_MS]);
    });
  });
});

describe("useSubscription", () => {
  describe("when signed out", () => {
    let result: { current: ReturnType<typeof useSubscription> };

    beforeEach(() => {
      ({ result } = renderHook(() => useSubscription(null), { wrapper }));
    });

    it("should stay idle", () => {
      expect(result.current.fetchStatus).toBe("idle");
    });

    it("should request nothing", () => {
      expect(api.fetchSubscription).not.toHaveBeenCalled();
    });
  });

  describe("when signed in", () => {
    let result: { current: ReturnType<typeof useSubscription> };

    beforeEach(() => {
      api.fetchSubscription.mockResolvedValue(subscription);
      ({ result } = renderHook(() => useSubscription(identity), { wrapper }));
    });

    it("should read the signed-in member's subscription with an abort signal", async () => {
      await waitFor(() => expect(result.current.data).toEqual(subscription));
      expect(api.fetchSubscription).toHaveBeenCalledWith(identity, { signal: expect.any(AbortSignal) });
    });
  });
});

describe("useUpdateSubscription", () => {
  let both: SubscriptionDetails;

  /** The member's view of the subscription and two independent writers, as two toggles would use them. */
  async function mountWriters(who: AuthIdentity = identity) {
    const view = renderHook(() => useSubscription(who), { wrapper });
    await waitFor(() => expect(view.result.current.data).toBeDefined());
    const one = renderHook(() => useUpdateSubscription(), { wrapper });
    const two = renderHook(() => useUpdateSubscription(), { wrapper });
    return { view, one, two };
  }
  const shownDetails = (view: { result: { current: { data?: SubscriptionResponse } } }) =>
    view.result.current.data?.details;

  type Writers = Awaited<ReturnType<typeof mountWriters>>;
  let view: Writers["view"];
  let one: Writers["one"];
  let two: Writers["two"];

  beforeEach(() => {
    both = { ...muted, ignore_all_in_app: true };
  });

  describe("when the member's subscription is shown with two writers", () => {
    beforeEach(async () => {
      api.fetchSubscription.mockResolvedValue(subscription);
      ({ view, one, two } = await mountWriters());
    });

    describe("and a write is pending", () => {
      let write: Deferred;

      beforeEach(async () => {
        write = deferred();
        api.putSubscription.mockReturnValue(write.promise);
        act(() => one.result.current.mutate({ identity, details: muted }));
        await waitFor(() => expect(shownDetails(view)).toEqual(muted));
      });

      it("should show the new details at once", () => {
        expect(shownDetails(view)).toEqual(muted);
      });

      it("should not read the subscription again yet", () => {
        expect(api.fetchSubscription).toHaveBeenCalledTimes(1);
      });

      describe("and the write settles", () => {
        beforeEach(async () => {
          api.fetchSubscription.mockResolvedValue({ ...subscription, details: muted });
          await act(async () => write.resolve());
          await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
        });

        it("should send the details with a signal", () => {
          expect(api.putSubscription).toHaveBeenCalledWith(identity, muted, { signal: expect.any(AbortSignal) });
        });

        it("should show the details read again", () => {
          expect(shownDetails(view)).toEqual(muted);
        });
      });

      describe("and a re-read lands during the write", () => {
        beforeEach(async () => {
          await act(async () => {
            await view.result.current.refetch();
          });
        });

        it("should read the subscription again", () => {
          expect(api.fetchSubscription).toHaveBeenCalledTimes(2);
        });

        it("should keep showing the pending details", () => {
          expect(shownDetails(view)).toEqual(muted);
        });
      });
    });

    describe("and the write fails and so does the re-read", () => {
      beforeEach(async () => {
        api.putSubscription.mockRejectedValue(new Error("500"));
        api.fetchSubscription.mockRejectedValue(new Error("offline"));
        act(() => one.result.current.mutate({ identity, details: muted }));
        await waitFor(() => expect(one.result.current.isError).toBe(true));
      });

      it("should show the server's details again", () => {
        expect(shownDetails(view)).toEqual(details);
      });

      it("should not write the failed details into the cache", () => {
        expect(client.getQueryData<SubscriptionResponse>(subscriptionKey)?.details).toEqual(details);
      });
    });

    describe("and the write succeeds", () => {
      let opts: { signal: AbortSignal };

      beforeEach(async () => {
        api.putSubscription.mockResolvedValue(undefined);
        act(() => one.result.current.mutate({ identity, details: muted }));
        await waitFor(() => expect(one.result.current.isSuccess).toBe(true));
        [, , opts] = api.putSubscription.mock.calls[0] as [AuthIdentity, SubscriptionDetails, { signal: AbortSignal }];
      });

      it("should bound the write with a signal", () => {
        expect(opts.signal).toBeInstanceOf(AbortSignal);
      });

      it("should leave the signal unaborted", () => {
        expect(opts.signal.aborted).toBe(false);
      });
    });

    describe("and a second write is sent while the first is pending", () => {
      let first: Deferred;

      beforeEach(async () => {
        first = deferred();
        api.putSubscription.mockReturnValueOnce(first.promise).mockResolvedValueOnce(undefined);
        act(() => one.result.current.mutate({ identity, details: muted }));
        act(() => two.result.current.mutate({ identity, details: both }));
        await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(1));
        await new Promise((resolve) => setTimeout(resolve, 20));
      });

      it("should hold the second write", () => {
        expect(api.putSubscription).toHaveBeenCalledTimes(1);
      });

      describe("and the first settles", () => {
        beforeEach(async () => {
          await act(async () => first.resolve());
          await waitFor(() => expect(two.result.current.isSuccess).toBe(true));
        });

        it("should send the second write after it", () => {
          expect(api.putSubscription.mock.calls.map(([, sent]) => sent)).toEqual([muted, both]);
        });
      });
    });

    describe("and two writes are queued", () => {
      let first: Deferred;
      let second: Deferred;

      beforeEach(() => {
        first = deferred();
        second = deferred();
        api.putSubscription.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
      });

      describe("and the first fails while the second, restoring the details, is pending", () => {
        beforeEach(async () => {
          act(() => one.result.current.mutate({ identity, details: muted }));
          act(() => two.result.current.mutate({ identity, details }));
          await act(async () => first.reject(new Error("500")));
          await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));
        });

        it("should not read the subscription again yet", () => {
          expect(api.fetchSubscription).toHaveBeenCalledTimes(1);
        });

        describe("and the second settles", () => {
          beforeEach(async () => {
            await act(async () => second.resolve());
          });

          it("should read the subscription again", async () => {
            await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
          });
        });
      });

      describe("and the first fails", () => {
        beforeEach(async () => {
          act(() => one.result.current.mutate({ identity, details: muted }));
          act(() => two.result.current.mutate({ identity, details: both }));
          await act(async () => first.reject(new Error("500")));
          await waitFor(() => expect(one.result.current.isError).toBe(true));
        });

        it("should keep showing the queued write's details", () => {
          expect(shownDetails(view)).toEqual(both);
        });
      });

      describe("and the re-read fails", () => {
        beforeEach(() => {
          api.fetchSubscription.mockRejectedValue(new Error("offline"));
          act(() => one.result.current.mutate({ identity, details: muted }));
          act(() => two.result.current.mutate({ identity, details: both }));
        });

        describe("and every queued write fails", () => {
          beforeEach(async () => {
            await act(async () => first.reject(new Error("500")));
            await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));
            await act(async () => second.reject(new Error("500")));
            await waitFor(() => expect(two.result.current.isError).toBe(true));
            await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
          });

          it("should show the server's details", () => {
            expect(shownDetails(view)).toEqual(details);
          });
        });

        describe("and only the later write fails", () => {
          beforeEach(async () => {
            await act(async () => first.resolve());
            await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));
            await act(async () => second.reject(new Error("500")));
            await waitFor(() => expect(two.result.current.isError).toBe(true));
            await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
          });

          it("should show the earlier accepted write", () => {
            expect(shownDetails(view)).toEqual(muted);
          });
        });
      });
    });
  });

  describe("when another account's subscription is shown too", () => {
    beforeEach(() => {
      api.fetchSubscription.mockImplementation(async (who: AuthIdentity) =>
        who.signer === OTHER ? otherSubscription : subscription,
      );
    });

    describe("and each account has a pending write", () => {
      let mineView: Writers["view"];
      let theirView: Writers["view"];
      let mine: Deferred;
      let theirs: Deferred;
      let before: { mine: number; theirs: number };
      const readsOf = (signer: string) =>
        api.fetchSubscription.mock.calls.filter(([who]) => (who as AuthIdentity).signer === signer).length;

      beforeEach(async () => {
        mineView = renderHook(() => useSubscription(identity), { wrapper });
        theirView = renderHook(() => useSubscription(other), { wrapper });
        await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
        theirs = deferred();
        mine = deferred();
        api.putSubscription.mockReturnValueOnce(theirs.promise).mockReturnValueOnce(mine.promise);
        one = renderHook(() => useUpdateSubscription(), { wrapper });
        two = renderHook(() => useUpdateSubscription(), { wrapper });
        before = { mine: readsOf(SIGNER), theirs: readsOf(OTHER) };

        act(() => one.result.current.mutate({ identity: other, details: muted }));
        act(() => two.result.current.mutate({ identity, details: muted }));
      });

      describe("and the other account's write settles", () => {
        beforeEach(async () => {
          await act(async () => theirs.resolve());
          await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));
        });

        it("should re-read the other account once its own writes settle", async () => {
          await waitFor(() => expect(readsOf(OTHER)).toBe(before.theirs + 1));
        });

        it("should keep showing my pending details", async () => {
          await waitFor(() => expect(readsOf(OTHER)).toBe(before.theirs + 1));
          expect(mineView.result.current.data?.details).toEqual(muted);
        });

        describe("and my write fails", () => {
          beforeEach(async () => {
            await waitFor(() => expect(readsOf(OTHER)).toBe(before.theirs + 1));
            await act(async () => mine.reject(new Error("500")));
            await waitFor(() => expect(two.result.current.isError).toBe(true));
          });

          it("should re-read my account and show its server details", async () => {
            await waitFor(() => expect(readsOf(SIGNER)).toBeGreaterThan(before.mine));
            expect(mineView.result.current.data?.details).toEqual(details);
          });

          it("should leave the other account's view on its own account", () => {
            expect(theirView.result.current.data?.address).toBe(OTHER);
          });
        });
      });
    });

    describe("and my write is pending", () => {
      let theirView: Writers["view"];
      let write: Deferred;

      beforeEach(async () => {
        theirView = renderHook(() => useSubscription(other), { wrapper });
        await waitFor(() => expect(theirView.result.current.data).toBeDefined());
        ({ one } = await mountWriters());
        write = deferred();
        api.putSubscription.mockReturnValue(write.promise);

        act(() => one.result.current.mutate({ identity, details: muted }));
        await waitFor(() => expect(api.putSubscription).toHaveBeenCalled());
      });

      afterEach(async () => {
        await act(async () => write.reject(new Error("500")));
      });

      it("should sign as the account in its variables", () => {
        expect(api.putSubscription.mock.calls[0]?.[0]).toBe(identity);
      });

      it("should leave the other account's view alone", () => {
        expect(theirView.result.current.data).toEqual(otherSubscription);
      });
    });
  });
});

describe("useSetEmail", () => {
  let result: { current: ReturnType<typeof useSetEmail> };

  describe("when the server accepts the address", () => {
    beforeEach(async () => {
      api.fetchSubscription.mockResolvedValue(subscription);
      renderHook(() => useSubscription(identity), { wrapper });
      await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(1));
      api.putEmail.mockResolvedValue(undefined);
      api.fetchSubscription.mockResolvedValue({ ...subscription, unconfirmedEmail: "new@example.com" });

      ({ result } = renderHook(() => useSetEmail(), { wrapper }));
      act(() => result.current.mutate({ identity, email: "new@example.com" }));
    });

    it("should send the address and read the subscription again", async () => {
      await waitFor(() =>
        expect(client.getQueryData(subscriptionKey)).toEqual({ ...subscription, unconfirmedEmail: "new@example.com" }),
      );
      expect(api.putEmail).toHaveBeenCalledWith(identity, "new@example.com");
    });
  });

  describe("when the server refuses the address", () => {
    beforeEach(async () => {
      client.setQueryData(subscriptionKey, subscription);
      api.putEmail.mockRejectedValue(new Error("invalid email"));

      ({ result } = renderHook(() => useSetEmail(), { wrapper }));
      act(() => result.current.mutate({ identity, email: "nope" }));
      await waitFor(() => expect(result.current.isError).toBe(true));
    });

    it("should leave the subscription as it was", () => {
      expect(client.getQueryData(subscriptionKey)).toEqual(subscription);
    });

    it("should read nothing", () => {
      expect(api.fetchSubscription).not.toHaveBeenCalled();
    });
  });

  describe("when another account is signed in since the address was sent", () => {
    beforeEach(async () => {
      api.fetchSubscription.mockImplementation(async (who: AuthIdentity) =>
        who.signer === OTHER ? otherSubscription : subscription,
      );
      renderHook(() => useSubscription(other), { wrapper });
      await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(1));
      const write = deferred();
      api.putEmail.mockReturnValue(write.promise);

      ({ result } = renderHook(() => useSetEmail(), { wrapper }));
      act(() => result.current.mutate({ identity, email: "new@example.com" }));
      await act(async () => write.resolve());
      await waitFor(() => expect(result.current.isSuccess).toBe(true));
    });

    it("should not re-read the account signed in since", () => {
      expect(api.fetchSubscription).toHaveBeenCalledTimes(1);
    });

    it("should leave the other account's subscription untouched", () => {
      expect(client.getQueryData(otherSubscriptionKey)).toEqual(otherSubscription);
    });
  });
});

describe("credits", () => {
  let enrolled: { status: string; optedOutAt: string | null };

  beforeEach(() => {
    enrolled = { status: "enrolled", optedOutAt: null };
  });

  describe("when signed out", () => {
    let result: { current: ReturnType<typeof useCreditsStatus> };

    beforeEach(() => {
      api.fetchCreditsStatus.mockResolvedValue(enrolled);
      ({ result } = renderHook(() => useCreditsStatus(null), { wrapper }));
    });

    it("should stay idle", () => {
      expect(result.current.fetchStatus).toBe("idle");
    });

    it("should not read the status", () => {
      expect(api.fetchCreditsStatus).not.toHaveBeenCalled();
    });
  });

  describe("when signed in", () => {
    let result: { current: ReturnType<typeof useCreditsStatus> };

    beforeEach(() => {
      api.fetchCreditsStatus.mockResolvedValue(enrolled);
      ({ result } = renderHook(() => useCreditsStatus(identity), { wrapper }));
    });

    it("should read the status once", async () => {
      await waitFor(() => expect(result.current.data).toEqual({ status: "enrolled", optedOutAt: null }));
      expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(1);
    });
  });

  describe("when the member leaves while another account's status is shown", () => {
    let mine: { result: { current: ReturnType<typeof useCreditsStatus> } };
    let theirs: { result: { current: ReturnType<typeof useCreditsStatus> } };
    let left: { status: string; optedOutAt: string | null };

    beforeEach(async () => {
      api.fetchCreditsStatus.mockImplementation(async (who: AuthIdentity) =>
        who.signer === OTHER ? { status: "enrolled", optedOutAt: null } : { status: "enrolled", optedOutAt: null },
      );
      mine = renderHook(() => useCreditsStatus(identity), { wrapper });
      theirs = renderHook(() => useCreditsStatus(other), { wrapper });
      await waitFor(() => expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(2));
      left = { status: "opted_out", optedOutAt: "2026-11-10T00:00:00.000Z" };
      api.fetchCreditsStatus.mockResolvedValue(left);
      api.optOutOfCredits.mockResolvedValue(undefined);

      const { result } = renderHook(() => useOptOutOfCredits(), { wrapper });
      act(() => result.current.mutate({ identity }));
      await waitFor(() => expect(mine.result.current.data).toEqual(left));
    });

    it("should show the status read again for the account that left", () => {
      expect(mine.result.current.data).toEqual(left);
    });

    it("should opt that account out", () => {
      expect(api.optOutOfCredits).toHaveBeenCalledWith(identity);
    });

    it("should read the status again only once", () => {
      expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(3);
    });

    it("should leave the other account's status alone", () => {
      expect(theirs.result.current.data).toEqual({ status: "enrolled", optedOutAt: null });
    });
  });

  describe("when an account switch removes the entry while leaving is pending", () => {
    beforeEach(async () => {
      client.setQueryData(creditsKey, enrolled);
      const write = deferred();
      api.optOutOfCredits.mockReturnValue(write.promise);

      const { result } = renderHook(() => useOptOutOfCredits(), { wrapper });
      act(() => result.current.mutate({ identity }));
      await waitFor(() => expect(api.optOutOfCredits).toHaveBeenCalled());
      client.removeQueries({ queryKey: ["private", SIGNER.toLowerCase()] });
      await act(async () => write.resolve());
      await waitFor(() => expect(result.current.isSuccess).toBe(true));
    });

    it("should not bring back the removed entry", () => {
      expect(client.getQueryData(creditsKey)).toBeUndefined();
    });

    it("should not read the status again", () => {
      expect(api.fetchCreditsStatus).not.toHaveBeenCalled();
    });
  });

  describe("when leaving fails", () => {
    beforeEach(async () => {
      client.setQueryData(creditsKey, enrolled);
      api.optOutOfCredits.mockRejectedValue(new Error("500"));

      const { result } = renderHook(() => useOptOutOfCredits(), { wrapper });
      act(() => result.current.mutate({ identity }));
      await waitFor(() => expect(result.current.isError).toBe(true));
    });

    it("should keep the enrolled status", () => {
      expect(client.getQueryData(creditsKey)).toEqual({ status: "enrolled", optedOutAt: null });
    });
  });
});
