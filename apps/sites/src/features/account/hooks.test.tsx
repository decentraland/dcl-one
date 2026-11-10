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

describe("useUpdateSubscription", () => {
  it("shows the new details at once and keeps them after the write", async () => {
    client.setQueryData(subscriptionKey, subscription);
    let finish!: () => void;
    api.putSubscription.mockImplementation(() => new Promise<void>((resolve) => (finish = resolve)));

    const { result } = renderHook(() => useUpdateSubscription(identity), { wrapper });
    act(() => result.current.mutate(muted));

    await waitFor(() =>
      expect(client.getQueryData<SubscriptionResponse>(subscriptionKey)?.details).toEqual(muted),
    );
    await act(async () => finish());
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData<SubscriptionResponse>(subscriptionKey)?.details).toEqual(muted);
    expect(api.putSubscription).toHaveBeenCalledWith(identity, muted);
  });

  it("reads the subscription again when the write fails", async () => {
    client.setQueryData(subscriptionKey, subscription);
    api.putSubscription.mockRejectedValue(new Error("500"));
    api.fetchSubscription.mockResolvedValue(subscription);
    renderHook(() => useSubscription(identity), { wrapper });

    const { result } = renderHook(() => useUpdateSubscription(identity), { wrapper });
    act(() => result.current.mutate(muted));

    await waitFor(() => expect(result.current.isError).toBe(true));
    await waitFor(() =>
      expect(client.getQueryData<SubscriptionResponse>(subscriptionKey)?.details).toEqual(details),
    );
    expect(api.fetchSubscription).toHaveBeenCalled();
  });

  it("refuses to write when signed out", async () => {
    const { result } = renderHook(() => useUpdateSubscription(null), { wrapper });
    act(() => result.current.mutate(muted));

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.error?.message).toBe("Sign in to manage your account.");
    expect(api.putSubscription).not.toHaveBeenCalled();
  });
});

describe("useSetEmail", () => {
  it("marks the new address as pending once the server accepts it", async () => {
    client.setQueryData(subscriptionKey, subscription);
    api.putEmail.mockResolvedValue(undefined);

    const { result } = renderHook(() => useSetEmail(identity), { wrapper });
    act(() => result.current.mutate("new@example.com"));

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData(subscriptionKey)).toEqual({ ...subscription, unconfirmedEmail: "new@example.com" });
  });

  it("leaves the subscription as it was when the server refuses", async () => {
    client.setQueryData(subscriptionKey, subscription);
    api.putEmail.mockRejectedValue(new Error("invalid email"));

    const { result } = renderHook(() => useSetEmail(identity), { wrapper });
    act(() => result.current.mutate("nope"));

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(client.getQueryData(subscriptionKey)).toEqual(subscription);
  });

  it("does not bring back an entry removed by an account switch", async () => {
    let finish!: () => void;
    api.putEmail.mockImplementation(() => new Promise<void>((resolve) => (finish = resolve)));
    client.setQueryData(subscriptionKey, subscription);

    const { result } = renderHook(() => useSetEmail(identity), { wrapper });
    act(() => result.current.mutate("new@example.com"));
    await waitFor(() => expect(api.putEmail).toHaveBeenCalled());
    client.removeQueries({ queryKey: ["private", SIGNER.toLowerCase()] });
    await act(async () => finish());

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData(subscriptionKey)).toBeUndefined();
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

  it("shows the member as opted out after leaving", async () => {
    client.setQueryData(creditsKey, { status: "enrolled", optedOutAt: null });
    api.optOutOfCredits.mockResolvedValue(undefined);

    const { result } = renderHook(() => useOptOutOfCredits(identity), { wrapper });
    act(() => result.current.mutate());

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData(creditsKey)).toMatchObject({ status: "opted_out", optedOutAt: expect.any(String) });
  });

  it("keeps the enrolled status when leaving fails", async () => {
    client.setQueryData(creditsKey, { status: "enrolled", optedOutAt: null });
    api.optOutOfCredits.mockRejectedValue(new Error("500"));

    const { result } = renderHook(() => useOptOutOfCredits(identity), { wrapper });
    act(() => result.current.mutate());

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(client.getQueryData(creditsKey)).toEqual({ status: "enrolled", optedOutAt: null });
  });
});
