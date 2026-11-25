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

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

function lastCall() {
  const [, url, init] = signer.fetch.mock.calls.at(-1) as [AuthIdentity, string, RequestInit];
  return { url: new URL(url), init };
}

let identity: AuthIdentity;
let details: SubscriptionDetails;

beforeEach(() => {
  identity = {
    signer: SIGNER,
    ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
    expiration: "2999-01-01T00:00:00.000Z",
    authChain: [],
  };
  details = {
    ignore_all_email: false,
    ignore_all_in_app: false,
    message_type: { item_sold: { email: true, in_app: true }, bid_received: { email: false, in_app: true } },
  };
});
afterEach(() => vi.resetAllMocks());

describe("fetchSubscription", () => {
  describe("when the server answers the signer's subscription", () => {
    let subscription: Awaited<ReturnType<typeof fetchSubscription>>;

    beforeEach(async () => {
      signer.fetch.mockResolvedValueOnce(
        json({ address: SIGNER, email: "jane.doe@example.com", unconfirmedEmail: "new@example.com", details }),
      );
      subscription = await fetchSubscription(identity);
    });

    it("should read it from the notifications routes", () => {
      expect(lastCall().url.pathname).toBe("/notifications/subscription");
    });

    it("should read it with a GET", () => {
      expect(lastCall().init.method).toBe("GET");
    });

    it("should answer the subscription", () => {
      expect(subscription).toEqual({
        address: SIGNER,
        email: "jane.doe@example.com",
        unconfirmedEmail: "new@example.com",
        details,
      });
    });
  });

  describe("when the reply's channels are missing", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(
        json({ address: SIGNER, details: { ...details, message_type: { item_sold: { email: true } } } }),
      );
    });

    it("should reject it rather than guess them", async () => {
      await expect(fetchSubscription(identity)).rejects.toThrow(/in_app/);
    });
  });

  describe("when the read fails", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ error: "unauthorized" }, 401));
    });

    it("should surface the server's status and error", async () => {
      await expect(fetchSubscription(identity)).rejects.toMatchObject({ status: 401, message: "unauthorized" });
    });
  });
});

describe("subscription writes", () => {
  describe("when writing the details", () => {
    beforeEach(async () => {
      signer.fetch.mockResolvedValueOnce(new Response(null, { status: 204 }));
      await putSubscription(identity, details);
    });

    it("should write to the subscription route", () => {
      expect(lastCall().url.pathname).toBe("/notifications/subscription");
    });

    it("should PUT", () => {
      expect(lastCall().init.method).toBe("PUT");
    });

    it("should send the whole details object", () => {
      expect(JSON.parse(String(lastCall().init.body))).toEqual(details);
    });
  });

  describe("when writing the email", () => {
    beforeEach(async () => {
      signer.fetch.mockResolvedValueOnce(new Response(null, { status: 204 }));
      await putEmail(identity, "jane.doe@example.com");
    });

    it("should write to set-email", () => {
      expect(lastCall().url.pathname).toBe("/notifications/set-email");
    });

    it("should PUT", () => {
      expect(lastCall().init.method).toBe("PUT");
    });

    it("should send only the new email", () => {
      expect(JSON.parse(String(lastCall().init.body))).toEqual({ email: "jane.doe@example.com" });
    });
  });

  describe("when the server refuses the email", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ message: "invalid email" }, 400));
    });

    it("should fail with the server's message", async () => {
      await expect(putEmail(identity, "nope")).rejects.toThrow("invalid email");
    });
  });
});

describe("subscription helpers", () => {
  describe("when one type's email is turned off and none is left on", () => {
    let next: SubscriptionDetails;

    beforeEach(() => {
      next = setTypeEmail(details, "item_sold", false);
    });

    it("should turn the type's email off", () => {
      expect(isTypeEmailEnabled(next, "item_sold")).toBe(false);
    });

    it("should keep the type's in-app channel", () => {
      expect(next.message_type.item_sold.in_app).toBe(true);
    });

    it("should mute email", () => {
      expect(next.ignore_all_email).toBe(true);
    });

    it("should not change the original details", () => {
      expect(details.message_type.item_sold.email).toBe(true);
    });
  });

  describe("when a type the server did not list is turned on", () => {
    let next: SubscriptionDetails;

    beforeEach(() => {
      next = setTypeEmail(details, "tip_received", true);
    });

    it("should add it with in-app on", () => {
      expect(next.message_type.tip_received).toEqual({ email: true, in_app: true });
    });

    it("should keep email unmuted", () => {
      expect(next.ignore_all_email).toBe(false);
    });
  });

  describe("when a type is not listed", () => {
    it("should treat its email as off", () => {
      expect(isTypeEmailEnabled(details, "tip_received")).toBe(false);
    });
  });

  describe("when email is muted", () => {
    let muted: SubscriptionDetails;

    beforeEach(() => {
      muted = setAllEmail(
        { ...details, message_type: { item_sold: { email: true, in_app: true }, bid_received: { email: true, in_app: true } } },
        false,
      );
    });

    describe("and a type is turned off", () => {
      it("should keep email muted", () => {
        expect(setTypeEmail(muted, "item_sold", false).ignore_all_email).toBe(true);
      });
    });

    describe("and a type is turned on", () => {
      it("should keep email muted", () => {
        expect(setTypeEmail(muted, "tip_received", true).ignore_all_email).toBe(true);
      });
    });
  });

  describe("when the master switch moves", () => {
    let muted: SubscriptionDetails;

    beforeEach(() => {
      muted = setAllEmail(details, false);
    });

    it("should turn all email off", () => {
      expect(isAllEmailEnabled(muted)).toBe(false);
    });

    it("should leave the per-type settings untouched", () => {
      expect(muted.message_type).toBe(details.message_type);
    });

    it("should turn all email back on", () => {
      expect(isAllEmailEnabled(setAllEmail(muted, true))).toBe(true);
    });
  });

  describe("when laying out the groups in two columns", () => {
    let laid: string[];

    beforeEach(() => {
      laid = SUBSCRIPTION_GROUP_COLUMNS.flat();
    });

    it("should lay out no group twice", () => {
      expect(new Set(laid).size).toBe(laid.length);
    });

    it("should lay out every group", () => {
      expect([...laid].sort()).toEqual(Object.keys(SUBSCRIPTION_GROUPS).sort());
    });
  });
});

describe("credits status", () => {
  describe("when the program knows the signer", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ data: { status: "enrolled", optedOutAt: null } }));
    });

    describe("and no mode is given", () => {
      it("should answer the status", async () => {
        await expect(fetchCreditsStatus(identity)).resolves.toEqual({ status: "enrolled", optedOutAt: null });
      });

      it("should read the lowercased signer from the shop routes", async () => {
        await fetchCreditsStatus(identity);
        expect(lastCall().url.pathname).toBe(`/shop-api/v1/users/${SIGNER.toLowerCase()}/status`);
      });
    });

    describe("and the node runs the Shop API in legacy mode", () => {
      it("should answer the status", async () => {
        await expect(fetchCreditsStatus(identity, { mode: "legacy" })).resolves.toEqual({
          status: "enrolled",
          optedOutAt: null,
        });
      });

      it("should read the legacy credits route", async () => {
        await fetchCreditsStatus(identity, { mode: "legacy" });
        expect(lastCall().url.pathname).toBe(`/credits/users/${SIGNER.toLowerCase()}/status`);
      });
    });

    describe("and the page was served in legacy mode", () => {
      beforeEach(() => {
        (window as { __SHOP_API_MODE__?: string }).__SHOP_API_MODE__ = "legacy";
      });

      afterEach(() => {
        delete (window as { __SHOP_API_MODE__?: string }).__SHOP_API_MODE__;
      });

      it("should default to the page's mode", async () => {
        await fetchCreditsStatus(identity);
        expect(lastCall().url.pathname).toBe(`/credits/users/${SIGNER.toLowerCase()}/status`);
      });
    });
  });

  describe("when the program has never seen the wallet", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ error: "not found" }, 404));
    });

    it("should read it as not registered", async () => {
      await expect(fetchCreditsStatus(identity)).resolves.toEqual({
        status: "not_registered",
        optedOutAt: null,
      });
    });
  });

  describe("when the node does not serve the route yet", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ error: "unavailable" }, 503));
    });

    it("should fail with the status and error", async () => {
      await expect(fetchCreditsStatus(identity)).rejects.toMatchObject({ status: 503, message: "unavailable" });
    });
  });

  describe("when the status value is unknown", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ data: { status: "paused", optedOutAt: null } }));
    });

    it("should reject it", async () => {
      await expect(fetchCreditsStatus(identity)).rejects.toThrow(/status/);
    });
  });
});

describe("optOutOfCredits", () => {
  describe("when the signer leaves the program", () => {
    beforeEach(async () => {
      signer.fetch.mockResolvedValueOnce(new Response(null, { status: 204 }));
      await optOutOfCredits(identity);
    });

    it("should target the lowercased signer's credits user", () => {
      expect(lastCall().url.pathname).toBe(`/shop-api/v1/users/${SIGNER.toLowerCase()}`);
    });

    it("should DELETE", () => {
      expect(lastCall().init.method).toBe("DELETE");
    });

    it("should send no body", () => {
      expect(lastCall().init.body).toBeUndefined();
    });
  });

  describe("when the node runs in legacy mode", () => {
    beforeEach(async () => {
      signer.fetch.mockResolvedValueOnce(new Response(null, { status: 204 }));
      await optOutOfCredits(identity, { mode: "legacy" });
    });

    it("should target the legacy credits user", () => {
      expect(lastCall().url.pathname).toBe(`/credits/users/${SIGNER.toLowerCase()}`);
    });

    it("should DELETE", () => {
      expect(lastCall().init.method).toBe("DELETE");
    });
  });

  describe("when the wallet was never registered", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ error: "not found" }, 404));
    });

    it("should succeed", async () => {
      await expect(optOutOfCredits(identity)).resolves.toBeUndefined();
    });
  });

  describe("when the wallet already left the program", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ ok: false, message: "User already unregistered" }, 400));
    });

    it("should succeed", async () => {
      await expect(optOutOfCredits(identity)).resolves.toBeUndefined();
    });
  });

  describe("when the server refuses with another 400, such as credits claimed this week", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ ok: false, message: "Cannot unregister with credits claimed this week" }, 400));
    });

    it("should fail with the status and the server's message", async () => {
      await expect(optOutOfCredits(identity)).rejects.toMatchObject({ status: 400, message: "Cannot unregister with credits claimed this week" });
    });
  });

  describe("when the server refuses otherwise", () => {
    beforeEach(() => {
      signer.fetch.mockResolvedValueOnce(json({ error: "boom" }, 500));
    });

    it("should fail with the status and error", async () => {
      await expect(optOutOfCredits(identity)).rejects.toMatchObject({ status: 500, message: "boom" });
    });
  });
});
