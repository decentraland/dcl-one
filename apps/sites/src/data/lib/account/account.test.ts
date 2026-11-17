// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const signer = vi.hoisted(() => ({ fetch: vi.fn() }));
vi.mock("../auth/signer", () => ({ signedFetch: signer.fetch }));

import type { AuthIdentity } from "../auth/types";
import { fetchCreditsStatus, optOutOfCredits } from "./credits";
import {
  SUBSCRIPTION_GROUPS,
  SUBSCRIPTION_GROUP_COLUMNS,
  fetchSubscription,
  isAllEmailEnabled,
  isTypeEmailEnabled,
  putEmail,
  putSubscription,
  setAllEmail,
  setTypeEmail,
  type SubscriptionDetails,
} from "./notifications";

const SIGNER = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";
const identity: AuthIdentity = {
  signer: SIGNER,
  ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
const details: SubscriptionDetails = {
  ignore_all_email: false,
  ignore_all_in_app: false,
  message_type: { item_sold: { email: true, in_app: true }, bid_received: { email: false, in_app: true } },
};

function lastCall() {
  const [, url, init] = signer.fetch.mock.calls.at(-1) as [AuthIdentity, string, RequestInit];
  return { url: new URL(url), init };
}

beforeEach(() => signer.fetch.mockReset());
afterEach(() => vi.clearAllMocks());

describe("fetchSubscription", () => {
  it("reads the signer's own subscription from the notifications routes", async () => {
    signer.fetch.mockResolvedValue(
      json({ address: SIGNER, email: "jane.doe@example.com", unconfirmedEmail: "new@example.com", details }),
    );

    const subscription = await fetchSubscription(identity);

    expect(lastCall().url.pathname).toBe("/notifications/subscription");
    expect(lastCall().init.method).toBe("GET");
    expect(subscription).toEqual({
      address: SIGNER,
      email: "jane.doe@example.com",
      unconfirmedEmail: "new@example.com",
      details,
    });
  });

  it("rejects a reply whose channels are missing, rather than guessing them", async () => {
    signer.fetch.mockResolvedValue(
      json({ address: SIGNER, details: { ...details, message_type: { item_sold: { email: true } } } }),
    );

    await expect(fetchSubscription(identity)).rejects.toThrow();
  });

  it("surfaces the server's status on a failed read", async () => {
    signer.fetch.mockResolvedValue(json({ error: "unauthorized" }, 401));

    await expect(fetchSubscription(identity)).rejects.toMatchObject({ status: 401 });
  });
});

describe("subscription writes", () => {
  it("PUTs the whole details object", async () => {
    signer.fetch.mockResolvedValue(new Response(null, { status: 204 }));

    await putSubscription(identity, details);

    expect(lastCall().url.pathname).toBe("/notifications/subscription");
    expect(lastCall().init.method).toBe("PUT");
    expect(JSON.parse(String(lastCall().init.body))).toEqual(details);
  });

  it("PUTs only the new email to set-email", async () => {
    signer.fetch.mockResolvedValue(new Response(null, { status: 204 }));

    await putEmail(identity, "jane.doe@example.com");

    expect(lastCall().url.pathname).toBe("/notifications/set-email");
    expect(lastCall().init.method).toBe("PUT");
    expect(JSON.parse(String(lastCall().init.body))).toEqual({ email: "jane.doe@example.com" });
  });

  it("fails when the server refuses the email", async () => {
    signer.fetch.mockResolvedValue(json({ message: "invalid email" }, 400));

    await expect(putEmail(identity, "nope")).rejects.toThrow("invalid email");
  });
});

describe("subscription helpers", () => {
  it("turns one type's email off and mutes email when none is left on", () => {
    const next = setTypeEmail(details, "item_sold", false);

    expect(isTypeEmailEnabled(next, "item_sold")).toBe(false);
    expect(next.message_type.item_sold.in_app).toBe(true);
    expect(next.ignore_all_email).toBe(true);
    expect(details.message_type.item_sold.email).toBe(true);
  });

  it("adds a type the server did not list with in-app on", () => {
    const next = setTypeEmail(details, "tip_received", true);

    expect(next.message_type.tip_received).toEqual({ email: true, in_app: true });
    expect(next.ignore_all_email).toBe(false);
  });

  it("treats an unlisted type as email off", () => {
    expect(isTypeEmailEnabled(details, "tip_received")).toBe(false);
  });

  it("moves only the master switch", () => {
    const muted = setAllEmail(details, false);

    expect(isAllEmailEnabled(muted)).toBe(false);
    expect(muted.message_type).toBe(details.message_type);
    expect(isAllEmailEnabled(setAllEmail(muted, true))).toBe(true);
  });

  it("lays out every group exactly once across the two columns", () => {
    const laid = SUBSCRIPTION_GROUP_COLUMNS.flat();

    expect(new Set(laid).size).toBe(laid.length);
    expect([...laid].sort()).toEqual(Object.keys(SUBSCRIPTION_GROUPS).sort());
  });
});

describe("credits status", () => {
  it("reads the lowercased signer's status from the shop routes", async () => {
    signer.fetch.mockResolvedValue(json({ data: { status: "enrolled", optedOutAt: null } }));

    await expect(fetchCreditsStatus(identity)).resolves.toEqual({ status: "enrolled", optedOutAt: null });
    expect(lastCall().url.pathname).toBe(`/shop-api/v1/users/${SIGNER.toLowerCase()}/status`);
  });

  it("reads the legacy credits route when the node runs the Shop API in legacy mode", async () => {
    signer.fetch.mockResolvedValue(json({ data: { status: "enrolled", optedOutAt: null } }));

    await expect(fetchCreditsStatus(identity, { mode: "legacy" })).resolves.toEqual({
      status: "enrolled",
      optedOutAt: null,
    });
    expect(lastCall().url.pathname).toBe(`/credits/users/${SIGNER.toLowerCase()}/status`);
  });

  it("takes the mode the page was served with when none is given", async () => {
    signer.fetch.mockResolvedValue(json({ data: { status: "enrolled", optedOutAt: null } }));
    (window as { __SHOP_API_MODE__?: string }).__SHOP_API_MODE__ = "legacy";
    try {
      await fetchCreditsStatus(identity);
    } finally {
      delete (window as { __SHOP_API_MODE__?: string }).__SHOP_API_MODE__;
    }

    expect(lastCall().url.pathname).toBe(`/credits/users/${SIGNER.toLowerCase()}/status`);
  });

  it("reads a wallet the program has never seen as not registered", async () => {
    signer.fetch.mockResolvedValue(json({ error: "not found" }, 404));

    await expect(fetchCreditsStatus(identity)).resolves.toEqual({
      status: "not_registered",
      optedOutAt: null,
    });
  });

  it("fails while the node does not serve the route yet", async () => {
    signer.fetch.mockResolvedValue(json({ error: "unavailable" }, 503));

    await expect(fetchCreditsStatus(identity)).rejects.toMatchObject({ status: 503 });
  });

  it("rejects an unknown status value", async () => {
    signer.fetch.mockResolvedValue(json({ data: { status: "paused", optedOutAt: null } }));

    await expect(fetchCreditsStatus(identity)).rejects.toThrow();
  });
});

describe("optOutOfCredits", () => {
  it("DELETEs the signer's credits user", async () => {
    signer.fetch.mockResolvedValue(new Response(null, { status: 204 }));

    await optOutOfCredits(identity);

    expect(lastCall().url.pathname).toBe(`/shop-api/v1/users/${SIGNER.toLowerCase()}`);
    expect(lastCall().init.method).toBe("DELETE");
    expect(lastCall().init.body).toBeUndefined();
  });

  it("DELETEs the legacy credits user in legacy mode", async () => {
    signer.fetch.mockResolvedValue(new Response(null, { status: 204 }));

    await optOutOfCredits(identity, { mode: "legacy" });

    expect(lastCall().url.pathname).toBe(`/credits/users/${SIGNER.toLowerCase()}`);
    expect(lastCall().init.method).toBe("DELETE");
  });

  it("succeeds for a wallet that was never registered", async () => {
    signer.fetch.mockResolvedValue(json({ error: "not found" }, 404));

    await expect(optOutOfCredits(identity)).resolves.toBeUndefined();
  });

  it("fails on any other refusal", async () => {
    signer.fetch.mockResolvedValue(json({ error: "boom" }, 500));

    await expect(optOutOfCredits(identity)).rejects.toMatchObject({ status: 500 });
  });
});
