// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "../catalyst/client";
import { accountKeys } from "./queries";
import {
  fetchManaBalance,
  fetchManaTransfers,
  formatManaAmount,
  isHistoryUnavailable,
  MANA_TRANSFERS_PAGE_SIZE,
} from "./wallet";

const ACCOUNT = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";
const OTHER = "0x1111111111111111111111111111111111111111";
const HASH = `0x${"ab".repeat(32)}`;
const ONE_MANA = `0x${(10n ** 18n).toString(16)}`;

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

function stubFetch(reply: () => Response | Promise<Response>) {
  const fetchMock = vi.fn((_url: string | URL | Request, _init?: RequestInit) => Promise.resolve(reply()));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function requestOf(fetchMock: ReturnType<typeof stubFetch>, call = 0) {
  const [url, init] = fetchMock.mock.calls[call]!;
  return { url: new URL(String(url)), init };
}

/** A wallet that lists `accounts`, sits on `chainId` and answers `eth_call` with `result`. */
function wallet(accounts: string[], chainId: number, result: unknown = ONE_MANA) {
  return {
    request: vi.fn(async ({ method }: { method: string }) => {
      if (method === "eth_accounts") return accounts;
      if (method === "eth_chainId") return `0x${chainId.toString(16)}`;
      if (method === "eth_call") return result;
      throw new Error(`unexpected ${method}`);
    }),
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("fetchManaBalance", () => {
  it.each([
    ["ethereum", "/rpc/mainnet", "0x0f5d2fb29fb7d3cfee444a200298f468908cc942"],
    ["polygon", "/rpc/polygon", "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4"],
  ] as const)("reads the %s balance of the MANA token through the node's relay", async (network, path, token) => {
    const fetchMock = stubFetch(() => json({ jsonrpc: "2.0", id: 1, result: ONE_MANA }));

    await expect(fetchManaBalance(ACCOUNT, network)).resolves.toBe("1000000000000000000");

    const { url, init } = requestOf(fetchMock);
    expect(url.pathname).toBe(path);
    expect(init?.method).toBe("POST");
    const body = JSON.parse(String(init?.body)) as { method: string; params: [{ to: string; data: string }, string] };
    expect(body.method).toBe("eth_call");
    expect(body.params[0].to.toLowerCase()).toBe(token);
    expect(body.params[0].data).toBe(`0x70a08231${"0".repeat(24)}${ACCOUNT.slice(2).toLowerCase()}`);
    expect(body.params[1]).toBe("latest");
  });

  it.each([
    ["a network the relay has no upstream for", () => json({ jsonrpc: "2.0", id: 1, error: { code: -32602, message: "unsupported network" } })],
    ["an upstream failure", () => json({ message: "bad gateway" }, 502)],
    ["a reply with no amount", () => json({ jsonrpc: "2.0", id: 1, result: "0x" })],
  ])("fails, giving no amount, on %s when no wallet can answer", async (_case, reply) => {
    stubFetch(reply);

    await expect(fetchManaBalance(ACCOUNT, "ethereum")).rejects.toThrow();
    await expect(fetchManaBalance(ACCOUNT, "ethereum", { ownWallet: () => null })).rejects.toThrow();
  });

  it("asks the member's own wallet when the relay cannot answer, without connecting or switching it", async () => {
    stubFetch(() => json({ jsonrpc: "2.0", id: 1, error: { message: "unsupported network" } }));
    const own = wallet([ACCOUNT.toLowerCase()], 137, "0x0de0b6b3a7640000");

    await expect(fetchManaBalance(ACCOUNT, "polygon", { ownWallet: () => own })).resolves.toBe("1000000000000000000");

    expect(own.request.mock.calls.map(([args]) => args.method)).toEqual(["eth_accounts", "eth_chainId", "eth_call"]);
  });

  it("does not ask the wallet while the relay answers", async () => {
    stubFetch(() => json({ jsonrpc: "2.0", id: 1, result: "0x0" }));
    const own = wallet([ACCOUNT], 1);

    await expect(fetchManaBalance(ACCOUNT, "ethereum", { ownWallet: () => own })).resolves.toBe("0");
    expect(own.request).not.toHaveBeenCalled();
  });

  it.each([
    ["is on another network", wallet([ACCOUNT], 137)],
    ["lists another account", wallet([OTHER], 1)],
    ["returns no amount", wallet([ACCOUNT], 1, "0x")],
  ])("keeps the relay's failure when the wallet %s", async (_case, own) => {
    stubFetch(() => json({ jsonrpc: "2.0", id: 1, error: { message: "unsupported network" } }));

    await expect(fetchManaBalance(ACCOUNT, "ethereum", { ownWallet: () => own })).rejects.toThrow("unsupported network");
    expect(own.request.mock.calls.some(([args]) => args.method === "eth_requestAccounts")).toBe(false);
  });

  describe("when the read is cancelled during the wallet fallback", () => {
    let controller: AbortController;
    let own: ReturnType<typeof wallet>;

    beforeEach(() => {
      controller = new AbortController();
      own = wallet([ACCOUNT], 1);
      stubFetch(() => json({ jsonrpc: "2.0", id: 1, error: { message: "unsupported network" } }));
    });

    it.each([
      ["eth_accounts", [ACCOUNT], 1],
      ["eth_chainId", "0x1", 2],
      ["eth_call", ONE_MANA, 3],
    ] as const)("should stop after %s and reject the cancelled result", async (method, result, callCount) => {
      const request = own.request.getMockImplementation()!;
      own.request.mockImplementation(async (args) => {
        if (args.method !== method) return request(args);
        controller.abort();
        return result;
      });

      await expect(
        fetchManaBalance(ACCOUNT, "ethereum", { signal: controller.signal, ownWallet: () => own }),
      ).rejects.toMatchObject({ name: "AbortError" });
      expect(own.request).toHaveBeenCalledTimes(callCount);
    });
  });

  it("refuses a value that is not an address before any request", async () => {
    const fetchMock = stubFetch(() => json({}));

    await expect(fetchManaBalance("not-an-address", "ethereum")).rejects.toThrow(RangeError);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("fetchManaTransfers", () => {
  const row = (over: Record<string, unknown> = {}) => ({
    blockNumber: 20_000_000,
    logIndex: 7,
    transactionHash: HASH,
    from: ACCOUNT.toLowerCase(),
    to: OTHER,
    amount: "2500000000000000000",
    timestamp: 1_760_000_000,
    ...over,
  });

  it.each([
    ["ethereum", "ethereum"],
    ["polygon", "matic"],
  ] as const)("reads a page of the %s transfers from the node's transfer index", async (network, indexName) => {
    const fetchMock = stubFetch(() =>
      json({ network: indexName, decimals: 18, indexedThrough: 20_000_100, data: [row()], next: null }),
    );

    const page = await fetchManaTransfers(ACCOUNT, network);

    const { url } = requestOf(fetchMock);
    expect(url.pathname).toBe("/marketplace-api/v1/mana/transfers");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      network: indexName,
      address: ACCOUNT.toLowerCase(),
      limit: String(MANA_TRANSFERS_PAGE_SIZE),
    });
    expect(page).toEqual({
      transfers: [
        { id: "20000000:7", hash: HASH, direction: "sent", counterparty: OTHER, amount: "2500000000000000000", at: 1_760_000_000_000 },
      ],
      next: null,
    });
  });

  it("names a transfer by the account's side of it", async () => {
    stubFetch(() =>
      json({ data: [row({ from: OTHER, to: ACCOUNT.toLowerCase(), logIndex: 1 }), row({ to: ACCOUNT.toLowerCase(), logIndex: 2 })] }),
    );

    const { transfers } = await fetchManaTransfers(ACCOUNT, "ethereum");

    expect(transfers.map((transfer) => [transfer.direction, transfer.counterparty])).toEqual([
      ["received", OTHER],
      ["sent", ACCOUNT.toLowerCase()],
    ]);
  });

  it("sends the cursor of the page before and returns the next one", async () => {
    const fetchMock = stubFetch(() => json({ data: [row()], next: { beforeBlock: 19_999_000, beforeLog: 3 } }));

    const page = await fetchManaTransfers(ACCOUNT, "polygon", { cursor: { beforeBlock: 20_000_000, beforeLog: 7 } });

    const { url } = requestOf(fetchMock);
    expect(url.searchParams.get("beforeBlock")).toBe("20000000");
    expect(url.searchParams.get("beforeLog")).toBe("7");
    expect(page.next).toEqual({ beforeBlock: 19_999_000, beforeLog: 3 });
  });

  it("reports an index that is off or still synchronizing as unavailable, not as an empty history", async () => {
    stubFetch(() => json({ error: "MANA transfer history is synchronizing" }, 503));

    await expect(fetchManaTransfers(ACCOUNT, "ethereum")).rejects.toBeInstanceOf(CatalystError);
    await expect(fetchManaTransfers(ACCOUNT, "ethereum")).rejects.toSatisfy(isHistoryUnavailable);
    expect(isHistoryUnavailable(new CatalystError("boom", "u", 500))).toBe(false);
    expect(isHistoryUnavailable(new Error("offline"))).toBe(false);
  });

  describe("when the transfer endpoint returns an unclassified 503", () => {
    beforeEach(() => {
      stubFetch(() => new Response(null, { status: 503 }));
    });

    it("should classify it as a retryable request failure", async () => {
      await expect(fetchManaTransfers(ACCOUNT, "ethereum")).rejects.toSatisfy(
        (error: unknown) => error instanceof CatalystError && error.status === 503 && !isHistoryUnavailable(error),
      );
    });
  });

  it("rejects a reply that is not the index's shape", async () => {
    stubFetch(() => json({ data: [row({ amount: "-1" })] }));

    await expect(fetchManaTransfers(ACCOUNT, "ethereum")).rejects.toThrow();
  });
});

describe("formatManaAmount", () => {
  it.each([
    ["0", "0"],
    ["1000000000000000000", "1"],
    ["100595000000000000000000", "100,595"],
    ["1239900000000000000", "1.23"],
    ["10000000000000000", "0.01"],
    ["9999999999999999", "< 0.01"],
    ["1", "< 0.01"],
  ])("shows %s wei as %s", (wei, shown) => {
    expect(formatManaAmount(wei)).toBe(shown);
  });
});

describe("the wallet query keys", () => {
  it("are private to the account, whatever its case, and differ by network and by datum", () => {
    expect(accountKeys.manaBalance(ACCOUNT, "ethereum")).toEqual(["private", ACCOUNT.toLowerCase(), "account", "mana-balance", "ethereum"]);
    expect(accountKeys.manaBalance(ACCOUNT, "polygon")).not.toEqual(accountKeys.manaBalance(ACCOUNT, "ethereum"));
    expect(accountKeys.manaTransfers(ACCOUNT, "polygon")).toEqual(["private", ACCOUNT.toLowerCase(), "account", "mana-transfers", "polygon"]);
    expect(accountKeys.manaTransfers(ACCOUNT, "polygon").slice(0, 3)).toEqual(accountKeys.all(ACCOUNT));
  });
});
