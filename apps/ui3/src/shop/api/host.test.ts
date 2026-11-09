// The browser host's wiring: what it reads from the page and passes to the client, which reads nothing itself.
import { afterEach, describe, expect, it } from "vitest";
import { generateEphemeralKey, type AuthIdentity } from "../../data/auth/identity";
import { createShopApi } from "./client";
import { ENDPOINTS } from "@shop/api/endpoints";
import { hostApiOptions, identitySigner, shopApiMode, shopPeers } from "./host";
import type { TransportRequest } from "./signer";

type HostWindow = { __SHOP_PEERS__?: Record<string, string>; __CATALYST_BASE__?: string; __SHOP_API_MODE__?: string };
const w = window as unknown as HostWindow;

afterEach(() => {
  delete w.__SHOP_PEERS__;
  delete w.__CATALYST_BASE__;
  delete w.__SHOP_API_MODE__;
});

describe("the browser host's Shop API wiring", () => {
  it("names the peers the page declares per network, and the node otherwise", () => {
    w.__CATALYST_BASE__ = "https://node.test/";
    expect(shopPeers()).toEqual({ byNetwork: {}, base: "https://node.test" });
    w.__SHOP_PEERS__ = { amoy: "https://peer.decentraland.zone" };
    expect(shopPeers()).toEqual({ byNetwork: { amoy: "https://peer.decentraland.zone" }, base: "https://node.test" });
  });

  it("fills the client's options with the page's base, normalized, and its mode; an override wins", () => {
    w.__CATALYST_BASE__ = "https://node.test/";
    w.__SHOP_API_MODE__ = "legacy";
    const options = hostApiOptions();
    expect([options.base, options.mode, typeof options.transport]).toEqual(["https://node.test", "legacy", "function"]);
    expect(shopApiMode("canonical")).toBe("canonical");
    expect(hostApiOptions({ base: "https://other.test/", mode: "canonical" })).toMatchObject({ base: "https://other.test", mode: "canonical" });
  });
});

function identity(expiresAt: number): AuthIdentity {
  const signer = generateEphemeralKey();
  return {
    signer: signer.address,
    ephemeral: generateEphemeralKey(),
    expiration: new Date(expiresAt).toISOString(),
    authChain: [{ type: "SIGNER", payload: signer.address, signature: "" }],
  };
}

describe("the browser host's transport", () => {
  it("sends through the fetch it is given, and a transport given wins over it", async () => {
    const asked: string[] = [];
    const fetchImpl = (async (url: string) => (asked.push(url), new Response("{}", { status: 200 }))) as unknown as typeof fetch;
    const viaFetch = hostApiOptions({ base: "https://node.test", mode: "canonical", fetchImpl });
    await createShopApi(viaFetch).request(ENDPOINTS.catalogShop);
    expect(asked).toEqual(["https://node.test/shop-api/v3/catalog/shop"]);
    const transport = async () => ({ status: 200, body: "{}" });
    expect(hostApiOptions({ base: "https://node.test", mode: "canonical", fetchImpl, transport }).transport).toBe(transport);
  });
});

describe("the browser host's identity signer", () => {
  it("gives no signer for an identity that has already expired", () => {
    expect(identitySigner(identity(Date.now() - 1))).toBeNull();
  });

  it("checks the expiration again when it signs, before anything reaches the transport", async () => {
    let now = 1_000;
    const signer = identitySigner(identity(2_000), () => now)!;
    const log: TransportRequest[] = [];
    const api = createShopApi({ base: "https://node.test", mode: "canonical", signer, transport: async (req) => (log.push(req), { status: 200, body: "{}" }) });
    await api.request(ENDPOINTS.catalogSuggested);
    expect(log).toHaveLength(1);
    now = 2_000;
    await expect(api.request(ENDPOINTS.catalogSuggested)).rejects.toThrow("The sign-in has expired");
    expect(log).toHaveLength(1);
  });
});
