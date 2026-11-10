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
const identity: AuthIdentity = {
  signer: SIGNER,
  ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};
const details: SubscriptionDetails = {
  ignore_all_email: false,
  ignore_all_in_app: false,
  message_type: { item_sold: { email: true, in_app: true } },
};
const muted: SubscriptionDetails = { ...details, ignore_all_email: true };
const subscription: SubscriptionResponse = { address: SIGNER, email: "jane.doe@example.com", details };
const subscriptionKey = accountKeys.subscription(SIGNER);
const creditsKey = accountKeys.creditsStatus(SIGNER);

let client: QueryClient;
function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
});
afterEach(() => {
  client.clear();
  vi.resetAllMocks();
});

describe("account keys", () => {
  it("keep every account read under the lowercased account's private prefix", () => {
    expect(subscriptionKey).toEqual(["private", SIGNER.toLowerCase(), "account", "subscription"]);
    expect(creditsKey).toEqual(["private", SIGNER.toLowerCase(), "account", "credits-status"]);
  });

  it("declare their freshness, so loader data and the page agree on it", () => {
    expect(subscriptionQuery(identity).staleTime).toBe(ACCOUNT_STALE_MS);
    expect(creditsStatusQuery(identity).staleTime).toBe(ACCOUNT_STALE_MS);
  });
});

describe("read hooks", () => {
  it("read with the factory's freshness, signed in or not", () => {
    api.fetchSubscription.mockReturnValue(new Promise(() => {}));
    api.fetchCreditsStatus.mockReturnValue(new Promise(() => {}));
    renderHook(() => useSubscription(identity), { wrapper });
    renderHook(() => useCreditsStatus(identity), { wrapper });
    renderHook(() => useSubscription(null), { wrapper });

    const staleTimes = client
      .getQueryCache()
      .getAll()
      .flatMap((query) => query.observers.map((observer) => observer.options.staleTime));
    expect(staleTimes).toEqual([ACCOUNT_STALE_MS, ACCOUNT_STALE_MS, ACCOUNT_STALE_MS]);
  });
});

describe("useSubscription", () => {
  it("requests nothing when signed out", () => {
    const { result } = renderHook(() => useSubscription(null), { wrapper });

    expect(result.current.fetchStatus).toBe("idle");
    expect(api.fetchSubscription).not.toHaveBeenCalled();
  });

  it("reads the signed-in member's subscription", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);

    const { result } = renderHook(() => useSubscription(identity), { wrapper });

    await waitFor(() => expect(result.current.data).toEqual(subscription));
    expect(api.fetchSubscription).toHaveBeenCalledWith(identity, { signal: expect.any(AbortSignal) });
  });
});

const OTHER = "0x1111111111111111111111111111111111111111";
const other: AuthIdentity = { ...identity, signer: OTHER };
const otherSubscription: SubscriptionResponse = { address: OTHER, email: "john.doe@example.com", details };
const otherSubscriptionKey = accountKeys.subscription(OTHER);

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("useUpdateSubscription", () => {
  const both: SubscriptionDetails = { ...muted, ignore_all_in_app: true };

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

  it("shows the new details at once and reads the subscription again once the write settles", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { view, one } = await mountWriters();
    const write = deferred();
    api.putSubscription.mockReturnValue(write.promise);
    api.fetchSubscription.mockResolvedValue({ ...subscription, details: muted });

    act(() => one.result.current.mutate({ identity, details: muted }));

    await waitFor(() => expect(shownDetails(view)).toEqual(muted));
    expect(api.fetchSubscription).toHaveBeenCalledTimes(1);
    await act(async () => write.resolve());
    await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
    expect(api.putSubscription).toHaveBeenCalledWith(identity, muted, { signal: expect.any(AbortSignal) });
    expect(shownDetails(view)).toEqual(muted);
  });

  it("shows the server's details again when the write fails, without writing them into the cache", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { view, one } = await mountWriters();
    api.putSubscription.mockRejectedValue(new Error("500"));
    api.fetchSubscription.mockRejectedValue(new Error("offline"));

    act(() => one.result.current.mutate({ identity, details: muted }));

    await waitFor(() => expect(one.result.current.isError).toBe(true));
    expect(shownDetails(view)).toEqual(details);
    expect(client.getQueryData<SubscriptionResponse>(subscriptionKey)?.details).toEqual(details);
  });

  it("sends the second write only after the first one settles", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { one, two } = await mountWriters();
    const first = deferred();
    api.putSubscription.mockReturnValueOnce(first.promise).mockResolvedValueOnce(undefined);

    act(() => one.result.current.mutate({ identity, details: muted }));
    act(() => two.result.current.mutate({ identity, details: both }));

    await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(1));
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(api.putSubscription).toHaveBeenCalledTimes(1);
    await act(async () => first.resolve());

    await waitFor(() => expect(two.result.current.isSuccess).toBe(true));
    expect(api.putSubscription.mock.calls.map(([, sent]) => sent)).toEqual([muted, both]);
  });

  it("reads the subscription again only once the last pending write settles", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { one, two } = await mountWriters();
    const first = deferred();
    const second = deferred();
    api.putSubscription.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    act(() => one.result.current.mutate({ identity, details: muted }));
    act(() => two.result.current.mutate({ identity, details }));
    await act(async () => first.reject(new Error("500")));
    await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));

    expect(api.fetchSubscription).toHaveBeenCalledTimes(1);
    await act(async () => second.resolve());
    await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
  });

  it("keeps showing a queued write's details when the write before it fails", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { view, one, two } = await mountWriters();
    const first = deferred();
    const second = deferred();
    api.putSubscription.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    act(() => one.result.current.mutate({ identity, details: muted }));
    act(() => two.result.current.mutate({ identity, details: both }));
    await act(async () => first.reject(new Error("500")));

    await waitFor(() => expect(one.result.current.isError).toBe(true));
    expect(shownDetails(view)).toEqual(both);
  });

  it("shows the server's details when every queued write and the re-read fail", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { view, one, two } = await mountWriters();
    const first = deferred();
    const second = deferred();
    api.putSubscription.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    api.fetchSubscription.mockRejectedValue(new Error("offline"));

    act(() => one.result.current.mutate({ identity, details: muted }));
    act(() => two.result.current.mutate({ identity, details: both }));
    await act(async () => first.reject(new Error("500")));
    await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));
    await act(async () => second.reject(new Error("500")));

    await waitFor(() => expect(two.result.current.isError).toBe(true));
    await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
    expect(shownDetails(view)).toEqual(details);
  });

  it("shows the earlier accepted write when only the later one fails and the re-read fails", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { view, one, two } = await mountWriters();
    const first = deferred();
    const second = deferred();
    api.putSubscription.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    api.fetchSubscription.mockRejectedValue(new Error("offline"));

    act(() => one.result.current.mutate({ identity, details: muted }));
    act(() => two.result.current.mutate({ identity, details: both }));
    await act(async () => first.resolve());
    await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));
    await act(async () => second.reject(new Error("500")));

    await waitFor(() => expect(two.result.current.isError).toBe(true));
    await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
    expect(shownDetails(view)).toEqual(muted);
  });

  it("keeps showing the pending details when a re-read lands during the write", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { view, one } = await mountWriters();
    const write = deferred();
    api.putSubscription.mockReturnValue(write.promise);

    act(() => one.result.current.mutate({ identity, details: muted }));
    await waitFor(() => expect(shownDetails(view)).toEqual(muted));
    await act(async () => {
      await view.result.current.refetch();
    });

    expect(api.fetchSubscription).toHaveBeenCalledTimes(2);
    expect(shownDetails(view)).toEqual(muted);
  });

  it("re-reads an account once its own writes settle, while another account's write is still pending", async () => {
    api.fetchSubscription.mockImplementation(async (who: AuthIdentity) =>
      who.signer === OTHER ? otherSubscription : subscription,
    );
    const mineView = renderHook(() => useSubscription(identity), { wrapper });
    const theirView = renderHook(() => useSubscription(other), { wrapper });
    await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
    const theirs = deferred();
    const mine = deferred();
    api.putSubscription.mockReturnValueOnce(theirs.promise).mockReturnValueOnce(mine.promise);
    const one = renderHook(() => useUpdateSubscription(), { wrapper });
    const two = renderHook(() => useUpdateSubscription(), { wrapper });

    const readsOf = (signer: string) =>
      api.fetchSubscription.mock.calls.filter(([who]) => (who as AuthIdentity).signer === signer).length;
    const before = { mine: readsOf(SIGNER), theirs: readsOf(OTHER) };

    act(() => one.result.current.mutate({ identity: other, details: muted }));
    act(() => two.result.current.mutate({ identity, details: muted }));
    await act(async () => theirs.resolve());
    await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));

    await waitFor(() => expect(readsOf(OTHER)).toBe(before.theirs + 1));
    expect(mineView.result.current.data?.details).toEqual(muted);
    await act(async () => mine.reject(new Error("500")));

    await waitFor(() => expect(two.result.current.isError).toBe(true));
    await waitFor(() => expect(readsOf(SIGNER)).toBeGreaterThan(before.mine));
    expect(mineView.result.current.data?.details).toEqual(details);
    expect(theirView.result.current.data?.address).toBe(OTHER);
  });

  it("bounds each write with a timeout signal", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    const { one } = await mountWriters();
    api.putSubscription.mockResolvedValue(undefined);

    act(() => one.result.current.mutate({ identity, details: muted }));

    await waitFor(() => expect(one.result.current.isSuccess).toBe(true));
    const [, , opts] = api.putSubscription.mock.calls[0] as [AuthIdentity, SubscriptionDetails, { signal: AbortSignal }];
    expect(opts.signal).toBeInstanceOf(AbortSignal);
    expect(opts.signal.aborted).toBe(false);
  });

  it("signs as the account in its variables and leaves another account's view alone", async () => {
    api.fetchSubscription.mockImplementation(async (who: AuthIdentity) =>
      who.signer === OTHER ? otherSubscription : subscription,
    );
    const theirView = renderHook(() => useSubscription(other), { wrapper });
    await waitFor(() => expect(theirView.result.current.data).toBeDefined());
    const { one } = await mountWriters();
    const write = deferred();
    api.putSubscription.mockReturnValue(write.promise);

    act(() => one.result.current.mutate({ identity, details: muted }));

    await waitFor(() => expect(api.putSubscription).toHaveBeenCalled());
    expect(api.putSubscription.mock.calls[0]?.[0]).toBe(identity);
    expect(theirView.result.current.data).toEqual(otherSubscription);
    await act(async () => write.reject(new Error("500")));
  });
});

describe("useSetEmail", () => {
  it("reads the subscription again once the server accepts the address", async () => {
    api.fetchSubscription.mockResolvedValue(subscription);
    renderHook(() => useSubscription(identity), { wrapper });
    await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(1));
    api.putEmail.mockResolvedValue(undefined);
    api.fetchSubscription.mockResolvedValue({ ...subscription, unconfirmedEmail: "new@example.com" });

    const { result } = renderHook(() => useSetEmail(), { wrapper });
    act(() => result.current.mutate({ identity, email: "new@example.com" }));

    await waitFor(() =>
      expect(client.getQueryData(subscriptionKey)).toEqual({ ...subscription, unconfirmedEmail: "new@example.com" }),
    );
    expect(api.putEmail).toHaveBeenCalledWith(identity, "new@example.com");
  });

  it("leaves the subscription as it was and reads nothing when the server refuses", async () => {
    client.setQueryData(subscriptionKey, subscription);
    api.putEmail.mockRejectedValue(new Error("invalid email"));

    const { result } = renderHook(() => useSetEmail(), { wrapper });
    act(() => result.current.mutate({ identity, email: "nope" }));

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(client.getQueryData(subscriptionKey)).toEqual(subscription);
    expect(api.fetchSubscription).not.toHaveBeenCalled();
  });

  it("re-reads only the account that sent the address, not the one signed in since", async () => {
    api.fetchSubscription.mockImplementation(async (who: AuthIdentity) =>
      who.signer === OTHER ? otherSubscription : subscription,
    );
    renderHook(() => useSubscription(other), { wrapper });
    await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(1));
    const write = deferred();
    api.putEmail.mockReturnValue(write.promise);

    const { result } = renderHook(() => useSetEmail(), { wrapper });
    act(() => result.current.mutate({ identity, email: "new@example.com" }));
    await act(async () => write.resolve());

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(api.fetchSubscription).toHaveBeenCalledTimes(1);
    expect(client.getQueryData(otherSubscriptionKey)).toEqual(otherSubscription);
  });
});

describe("credits", () => {
  it("reads the status only when signed in", async () => {
    api.fetchCreditsStatus.mockResolvedValue({ status: "enrolled", optedOutAt: null });

    const signedOut = renderHook(() => useCreditsStatus(null), { wrapper });
    expect(signedOut.result.current.fetchStatus).toBe("idle");

    const { result } = renderHook(() => useCreditsStatus(identity), { wrapper });
    await waitFor(() => expect(result.current.data).toEqual({ status: "enrolled", optedOutAt: null }));
    expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(1);
  });

  it("reads the status again for the account that left only", async () => {
    api.fetchCreditsStatus.mockImplementation(async (who: AuthIdentity) =>
      who.signer === OTHER ? { status: "enrolled", optedOutAt: null } : { status: "enrolled", optedOutAt: null },
    );
    const mine = renderHook(() => useCreditsStatus(identity), { wrapper });
    const theirs = renderHook(() => useCreditsStatus(other), { wrapper });
    await waitFor(() => expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(2));
    const left = { status: "opted_out", optedOutAt: "2026-11-10T00:00:00.000Z" };
    api.fetchCreditsStatus.mockResolvedValue(left);
    api.optOutOfCredits.mockResolvedValue(undefined);

    const { result } = renderHook(() => useOptOutOfCredits(), { wrapper });
    act(() => result.current.mutate({ identity }));

    await waitFor(() => expect(mine.result.current.data).toEqual(left));
    expect(api.optOutOfCredits).toHaveBeenCalledWith(identity);
    expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(3);
    expect(theirs.result.current.data).toEqual({ status: "enrolled", optedOutAt: null });
  });

  it("does not bring back an entry removed by an account switch", async () => {
    client.setQueryData(creditsKey, { status: "enrolled", optedOutAt: null });
    const write = deferred();
    api.optOutOfCredits.mockReturnValue(write.promise);

    const { result } = renderHook(() => useOptOutOfCredits(), { wrapper });
    act(() => result.current.mutate({ identity }));
    await waitFor(() => expect(api.optOutOfCredits).toHaveBeenCalled());
    client.removeQueries({ queryKey: ["private", SIGNER.toLowerCase()] });
    await act(async () => write.resolve());

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData(creditsKey)).toBeUndefined();
    expect(api.fetchCreditsStatus).not.toHaveBeenCalled();
  });

  it("keeps the enrolled status when leaving fails", async () => {
    client.setQueryData(creditsKey, { status: "enrolled", optedOutAt: null });
    api.optOutOfCredits.mockRejectedValue(new Error("500"));

    const { result } = renderHook(() => useOptOutOfCredits(), { wrapper });
    act(() => result.current.mutate({ identity }));

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(client.getQueryData(creditsKey)).toEqual({ status: "enrolled", optedOutAt: null });
  });
});
