import { afterEach, describe, expect, it } from "vitest";

import { CatalystError } from "../../data/catalyst/client";
import {
  chainName,
  deliverTip,
  forwardTip,
  TIP_DELIVERY_WINDOW_MS,
  encodeManaTransfer,
  manaToWei,
  sendManaTip,
  shouldForwardTip,
  tipTargetFromConfig,
} from "./manaTip";

const OWNER = "0x92de52247aeae00fcfb18072c8564f3549b64f9c";
const MANA_POLYGON = "0xA1c57f48F0Deb89f569dFbE6E2B7f46D33606fD4";

type Call = { method: string; params?: unknown[] };

function fakeProvider(chainId: string, hash: unknown = "0xhash") {
  const calls: Call[] = [];
  let current = chainId;
  const provider = {
    request: async ({ method, params }: { method: string; params?: unknown[] }) => {
      calls.push({ method, params });
      if (method === "eth_chainId") return current;
      if (method === "wallet_switchEthereumChain") {
        current = (params?.[0] as { chainId: string }).chainId;
        return null;
      }
      if (method === "eth_sendTransaction") return hash;
      throw new Error(`unexpected ${method}`);
    },
  };
  return { provider, calls };
}

describe("manaToWei", () => {
  it("parses whole and fractional MANA into wei and rejects zero, negatives and non-numbers", () => {
    expect(manaToWei("1")).toBe(1_000_000_000_000_000_000n);
    expect(manaToWei("1.5")).toBe(1_500_000_000_000_000_000n);
    expect(manaToWei(" 0.000001 ")).toBe(1_000_000_000_000n);
    expect(manaToWei("0")).toBeNull();
    expect(manaToWei("-3")).toBeNull();
    expect(manaToWei("abc")).toBeNull();
    expect(manaToWei("")).toBeNull();
    expect(manaToWei("1e3")).toBeNull();
  });
});

describe("encodeManaTransfer", () => {
  it("emits the ERC-20 transfer selector with the padded recipient and amount, and refuses a malformed recipient", () => {
    const wei = 5_000_000_000_000_000_000n;
    const data = encodeManaTransfer(OWNER, wei);
    expect(data.slice(0, 10)).toBe("0xa9059cbb");
    expect(data.slice(10, 74)).toBe("0".repeat(24) + OWNER.slice(2));
    expect(BigInt("0x" + data.slice(74))).toBe(wei);
    expect(data.length).toBe(2 + 8 + 64 + 64);
    expect(() => encodeManaTransfer("0x1234", 1n)).toThrow(/valid wallet address/);
  });
});

describe("tipTargetFromConfig and chainName", () => {
  it("accepts a configured chain and MANA token, refuses a missing or malformed config, and names the chains MANA lives on", () => {
    expect(
      tipTargetFromConfig({ chainId: 137, enabled: true, manaToken: MANA_POLYGON, payTo: null }),
    ).toEqual({ chainId: 137, manaToken: MANA_POLYGON });
    expect(tipTargetFromConfig({ chainId: 137, enabled: false, manaToken: null, payTo: null })).toBeNull();
    expect(tipTargetFromConfig(null)).toBeNull();
    expect(tipTargetFromConfig({ chainId: "137", manaToken: MANA_POLYGON })).toBeNull();
    expect(chainName(1)).toBe("Ethereum");
    expect(chainName(137)).toBe("Polygon");
    expect(chainName(4242)).toBe("chain 4242");
  });
});

describe("sendManaTip", () => {
  const target = { chainId: 137, manaToken: MANA_POLYGON };
  const from = "0x0000000000000000000000000000000000000001";

  it("sends straight away on the right chain, switches chains first otherwise, and fails loudly without a hash", async () => {
    const direct = fakeProvider("0x89");
    const hash = await sendManaTip(direct.provider, { from, to: OWNER, wei: 1n, target });
    expect(hash).toBe("0xhash");
    expect(direct.calls.map((c) => c.method)).toEqual(["eth_chainId", "eth_sendTransaction"]);
    const tx = direct.calls[1]?.params?.[0] as { from: string; to: string; data: string };
    expect(tx.from).toBe(from);
    expect(tx.to).toBe(MANA_POLYGON);
    expect(tx.data.startsWith("0xa9059cbb")).toBe(true);

    const elsewhere = fakeProvider("0x1");
    await sendManaTip(elsewhere.provider, { from, to: OWNER, wei: 1n, target });
    expect(elsewhere.calls.map((c) => c.method)).toEqual([
      "eth_chainId",
      "wallet_switchEthereumChain",
      "eth_sendTransaction",
    ]);
    expect(elsewhere.calls[1]?.params?.[0]).toEqual({ chainId: "0x89" });

    const { provider } = fakeProvider("0x89", null);
    await expect(sendManaTip(provider, { from, to: OWNER, wei: 1n, target })).rejects.toThrow(
      /no transaction hash/,
    );
  });
});

describe("shouldForwardTip", () => {
  const hash = `0x${"ab".repeat(32)}`;
  const from = "0x00000000000000000000000000000000000000AA";
  const polygon = { chainId: 137, manaToken: MANA_POLYGON };

  it("forwards a well-formed Polygon or Amoy tip signed by the tipping wallet", () => {
    expect(shouldForwardTip({ target: polygon, hash, from, signer: from.toLowerCase() })).toBe(true);
    expect(shouldForwardTip({ target: { chainId: 80002, manaToken: MANA_POLYGON }, hash, from, signer: from })).toBe(true);
  });

  it("skips other chains, malformed hashes, guests and a signer other than the tipping wallet", () => {
    expect(shouldForwardTip({ target: { chainId: 1, manaToken: MANA_POLYGON }, hash, from, signer: from })).toBe(false);
    expect(shouldForwardTip({ target: polygon, hash: "0xdeadbeef", from, signer: from })).toBe(false);
    expect(shouldForwardTip({ target: polygon, hash, from, signer: null })).toBe(false);
    expect(shouldForwardTip({ target: polygon, hash, from, signer: OWNER })).toBe(false);
  });
});

describe("deliverTip", () => {
  const hash = `0x${"cd".repeat(32)}`;

  // A wallet whose receipt reads return `receipts` in turn (the last repeats), on a fake clock.
  function harness(receipts: unknown[], outcomes: (number | null | { status: number; retryAfter: number })[]) {
    let clock = 0;
    const slept: number[] = [];
    const methods: string[] = [];
    const forwards: number[] = [];
    let r = 0;
    const provider = {
      request: async ({ method }: { method: string }) => {
        methods.push(method);
        const next = receipts[Math.min(r++, receipts.length - 1)];
        if (next instanceof Error) throw next;
        return next;
      },
    };
    let f = 0;
    const forward = async () => {
      forwards.push(clock);
      const outcome = outcomes[Math.min(f++, outcomes.length - 1)];
      if (outcome == null) return;
      const status = typeof outcome === "number" ? outcome : outcome.status;
      const err = new CatalystError(`Catalyst returned ${status}`, "url", status);
      if (typeof outcome !== "number") err.retryAfterSecs = outcome.retryAfter;
      throw err;
    };
    const opts = {
      forward,
      now: () => clock,
      sleep: async (ms: number) => {
        slept.push(ms);
        clock += ms;
      },
    };
    return { provider, opts, slept, methods, forwards };
  }

  it("forwards only once the receipt is mined, polling past pending and failed reads", async () => {
    const h = harness([null, new Error("rpc down"), { status: "0x1" }], [null]);
    expect(await deliverTip(h.provider, hash, h.opts)).toBe("notified");
    expect(h.methods).toEqual(Array(3).fill("eth_getTransactionReceipt"));
    expect(h.forwards).toEqual([8_000]);
  });

  it("never forwards a reverted transfer", async () => {
    const h = harness([{ status: "0x0" }], [null]);
    expect(await deliverTip(h.provider, hash, h.opts)).toBe("reverted");
    expect(h.forwards).toEqual([]);
  });

  it("retries 429, 5xx and network failures with growing backoff and stops at the first answer", async () => {
    const h = harness([{ status: "0x1" }], [429, 503, 0, null]);
    expect(await deliverTip(h.provider, hash, h.opts)).toBe("notified");
    expect(h.forwards).toHaveLength(4);
    expect(h.slept).toEqual([5_000, 10_000, 20_000]);
  });

  it("waits at least the server's Retry-After, bounded by the longest backoff", async () => {
    const h = harness([{ status: "0x1" }], [{ status: 429, retryAfter: 2 }, { status: 429, retryAfter: 60 }, { status: 503, retryAfter: 600 }, null]);
    expect(await deliverTip(h.provider, hash, h.opts)).toBe("notified");
    expect(h.slept).toEqual([5_000, 60_000, 120_000]);
  });

  it("stops without retrying when the server refuses the tip", async () => {
    const h = harness([{ status: "0x1" }], [400, null]);
    expect(await deliverTip(h.provider, hash, h.opts)).toBe("refused");
    expect(h.forwards).toHaveLength(1);
  });

  it("gives up once the delivery window closes, with bounded waits", async () => {
    const h = harness([{ status: "0x1" }], [429]);
    expect(await deliverTip(h.provider, hash, h.opts)).toBe("gave-up");
    expect(h.slept.reduce((a, b) => a + b, 0)).toBeLessThanOrEqual(TIP_DELIVERY_WINDOW_MS);
    expect(Math.max(...h.slept)).toBe(120_000);
    const unmined = harness([null], [null]);
    expect(await deliverTip(unmined.provider, hash, unmined.opts)).toBe("gave-up");
    expect(unmined.forwards).toEqual([]);
  });
});

describe("forwardTip", () => {
  afterEach(() => {
    delete (window as unknown as { dclBridge?: unknown }).dclBridge;
  });

  it("carries the server's retryAfter from a signed 429 body onto the error", async () => {
    const listeners = new Set<(push: unknown) => void>();
    (window as unknown as { dclBridge: unknown }).dclBridge = {
      send: (_action: string, payload: { id: string }) =>
        queueMicrotask(() => {
          const body = JSON.stringify({ error: "rate limit exceeded", retryAfter: 7 });
          for (const cb of listeners) cb({ kind: "signedFetchResult", id: payload.id, status: 429, body });
        }),
      onState: (cb: (push: unknown) => void) => {
        listeners.add(cb);
        return () => listeners.delete(cb);
      },
    };
    await expect(forwardTip(`0x${"ab".repeat(32)}`)).rejects.toMatchObject({ status: 429, retryAfterSecs: 7 });
  });
});
