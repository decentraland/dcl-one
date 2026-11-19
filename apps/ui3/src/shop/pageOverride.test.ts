// The page's overrides of the Shop's relay, RPCs and chains (./pageOverride): honoured in tests and local builds,
// ignored by a production build whatever a script on the page sets.
import { afterEach, describe, expect, it, vi } from "vitest";
import { CHAIN } from "@shop/model/chain";
import { settlementChainId } from "./checkout/chain/config";
import { landChainId } from "./land/chain";
import { relayBase } from "./market/lib/metaTx";
import { rpcUrl } from "./market/lib/rpc";

type Page = Window & { __SHOP_CHAIN__?: unknown; __SHOP_RELAY__?: unknown; __SHOP_RPC__?: unknown; __SHOP_ETHEREUM_CHAIN_ID__?: unknown };
const page = window as Page;

function repoint(): void {
  page.__SHOP_CHAIN__ = { settlementChainId: CHAIN.AMOY };
  page.__SHOP_RELAY__ = "https://relay.example/v1";
  page.__SHOP_RPC__ = { [CHAIN.POLYGON]: "https://rpc.example/polygon" };
  page.__SHOP_ETHEREUM_CHAIN_ID__ = CHAIN.SEPOLIA;
}

afterEach(() => {
  delete page.__SHOP_CHAIN__;
  delete page.__SHOP_RELAY__;
  delete page.__SHOP_RPC__;
  delete page.__SHOP_ETHEREUM_CHAIN_ID__;
  vi.unstubAllEnvs();
});

describe("the page's overrides of the Shop's relay, RPCs and chains", () => {
  it("are honoured outside a production build", () => {
    vi.stubEnv("VITE_SHOP_CHAIN_ID", String(CHAIN.POLYGON));
    repoint();
    expect(settlementChainId()).toBe(CHAIN.AMOY);
    expect(relayBase()).toBe("https://relay.example/v1");
    expect(rpcUrl(CHAIN.POLYGON)).toBe("https://rpc.example/polygon");
    expect(landChainId()).toBe(CHAIN.SEPOLIA);
  });

  it("are ignored by a production build, which keeps its own", () => {
    vi.stubEnv("VITE_SHOP_CHAIN_ID", String(CHAIN.POLYGON));
    const own = { chain: settlementChainId(), relay: relayBase(), rpc: rpcUrl(CHAIN.POLYGON), land: landChainId() };
    vi.stubEnv("DEV", false);
    repoint();
    expect(settlementChainId()).toBe(own.chain);
    expect(relayBase()).toBe(own.relay);
    expect(rpcUrl(CHAIN.POLYGON)).toBe(own.rpc);
    expect(landChainId()).toBe(own.land);
    expect([own.chain, own.relay, own.rpc, own.land]).not.toContain(CHAIN.AMOY);
    expect(own.relay).not.toBe("https://relay.example/v1");
  });
});
