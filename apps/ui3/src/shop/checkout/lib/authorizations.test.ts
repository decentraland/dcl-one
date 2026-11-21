// The shared MANA approval path's amounts: a grant is in full unless an exact amount is named (the CreditsManager a
// credits + MANA split pays through), a revoke is always zero, and an exact amount is only ever a positive allowance.

import { decodeFunctionData, maxUint256, type Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ERC20_ABI } from "@shop/checkout/chain/abi";
import { CHAIN } from "@shop/model/chain";
import { BOOK } from "@shop/payment/fixtures";
import { requireContracts } from "../chain/contracts";
import type { ChainReader } from "../chain/rpc";
import type { Eip1193Provider } from "../../../data/auth/wallet";
import { createFakeChain, createFakeLocks, createFakeRelay, createFakeWallet, relayedApproval } from "../fakes";
import { encodeExecuteMetaTransaction } from "../chain/metatx";
import { TxRevertedError } from "../chain/wallet";
import {
  AuthorizationKind,
  AuthorizationPendingError,
  encodeAuthorizationCall,
  ensureAuthorization,
  exactAllowanceReady,
  ExactGrantUnavailableError,
  getManaSpendingAuthorization,
  GrantCancelledError,
  GrantNotInEffectError,
  GrantUncheckedError,
  GrantNotRecordedError,
  metaTxTargetFor,
  PENDING_GRANT_HOLD_MS,
  readPendingGrant,
  setAuthorization,
  type ShopAuthorization,
} from "./authorizations";

const MANAGER = requireContracts(CHAIN.POLYGON).creditsManager!.address;
const allowance = getManaSpendingAuthorization(CHAIN.POLYGON, MANAGER);
const approved = (data: Hex) => decodeFunctionData({ abi: ERC20_ABI, data }).args[1] as bigint;

describe("the MANA a grant allows", () => {
  it("a grant is in full unless an exact amount is named; a revoke is zero whatever is named", () => {
    expect(approved(encodeAuthorizationCall(allowance, true))).toBe(maxUint256);
    expect(approved(encodeAuthorizationCall(allowance, true, 13n * 10n ** 17n))).toBe(13n * 10n ** 17n);
    expect(approved(encodeAuthorizationCall(allowance, false, 13n * 10n ** 17n))).toBe(0n);
  });

  it("an exact amount is only a positive allowance, refused before anything is asked of the wallet", async () => {
    const wallet = createFakeWallet();
    const ctx = { provider: wallet, account: BOOK.buyer, walletKind: "web3" as const, reader: createFakeChain() };
    const nft: ShopAuthorization = { kind: AuthorizationKind.Approval, contractAddress: BOOK.collection, spenderAddress: MANAGER, chainId: CHAIN.POLYGON };
    await expect(setAuthorization({ ...ctx, auth: nft, active: true, amount: 1n })).rejects.toThrow(/positive MANA allowance/);
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 0n })).rejects.toThrow(/positive MANA allowance/);
    await expect(ensureAuthorization({ ...ctx, auth: allowance, exact: true })).rejects.toThrow(/positive amount/);
    await expect(ensureAuthorization({ ...ctx, auth: allowance, exact: true, requiredWei: 0n })).rejects.toThrow(/positive amount/);
    expect(wallet.requests).toEqual([]);
  });
});

describe("an exact grant that may still land", () => {
  afterEach(() => {
    sessionStorage.clear();
    localStorage.clear();
    delete (window as { __SHOP_CHAIN__?: unknown }).__SHOP_CHAIN__;
  });
  const GAP = 61n;
  const MANA = requireContracts(CHAIN.POLYGON).mana.address.toLowerCase();
  const allowanceKey = `${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${MANAGER.toLowerCase()}`;
  const nonceKey = `${MANA}:${BOOK.buyer.toLowerCase()}`;
  const recordKey = `dcl-shop-exact-grant:v2:${BOOK.buyer.toLowerCase()}:${CHAIN.POLYGON}:${MANA}:${MANAGER.toLowerCase()}`;

  /**
   * A relay in `mode` whose sends the chain confirms (`relayed` says how, per post), a wallet whose sends it confirms too,
   * and one browser's Web Locks. An approve the relay answered with its hash, and that succeeds, has run: its nonce is
   * spent and the allowance is its amount. One whose answer was lost has not run yet, as far as the reads go.
   */
  function rail(mode: "fail-after-send" | "refuse", relayed: (post: number) => "success" | "reverted" = () => "success") {
    const chain = createFakeChain();
    let posts = 0;
    const relay = createFakeRelay({
      mode,
      onRelay: (hash, body) => {
        const status = relayed(++posts);
        chain.receipts.set(hash.toLowerCase(), status);
        const approval = relayedApproval(body);
        if (status !== "success" || !approval || relay.state.mode !== "ok") return;
        chain.nonces.set(nonceKey, (chain.nonces.get(nonceKey) ?? 0n) + 1n);
        chain.allowances.set(allowanceKey, approval.amount);
      },
    });
    let n = 0;
    const wallet = createFakeWallet({
      onSend: () => {
        const hash = `0x${(++n).toString(16).padStart(64, "0")}` as Hex;
        chain.receipts.set(hash, "success");
        return hash;
      },
    });
    const ctx = {
      provider: wallet as Eip1193Provider,
      account: BOOK.buyer,
      walletKind: "web3" as const,
      reader: chain as ChainReader,
      relayerUrl: "https://node.test/transactions-api/v1",
      fetchImpl: relay.fetchImpl,
      locks: createFakeLocks() as LockManager | null,
    };
    const sent = () => wallet.sent.map((tx) => approved(tx.data));
    const signed = () => wallet.requests.filter((r) => r.method === "eth_signTypedData_v4").map((r) => (JSON.parse(String(r.params![1])) as { message: { nonce: string } }).message.nonce);
    const signatures = () => signed().length;
    return { chain, relay, wallet, ctx, sent, signed, signatures };
  }
  /** A relayed approve of `amount` from the buyer, as this code records one (any signature: only its shape is checked). */
  const signedApprove = (amount: bigint) =>
    encodeExecuteMetaTransaction(metaTxTargetFor(allowance)!.metaTx, BOOK.buyer, encodeAuthorizationCall(allowance, true, amount), `0x${"11".repeat(64)}1b`);
  /** Another tab of this browser: its session storage starts empty; the local storage and the Web Locks are the same. */
  const anotherTab = () => sessionStorage.clear();
  const later = (ms: number) => vi.spyOn(Date, "now").mockReturnValue(Date.now() + ms);
  /** A wallet whose first signature waits until `sign()`: a window whose approval is still in the wallet. */
  function signingLater(wallet: Eip1193Provider) {
    let sign!: () => void;
    const gate = new Promise<void>((resolve) => (sign = resolve));
    let asked = 0;
    const provider: Eip1193Provider = {
      ...wallet,
      request: async (args) => {
        if (args.method === "eth_signTypedData_v4" && ++asked === 1) await gate;
        return wallet.request(args);
      },
    };
    return { provider, sign, asked: () => asked };
  }

  it("a lost answer is recorded with the call it signed, its nonce and its amount; while it can run, a covered allowance does not retire it", async () => {
    const { chain, relay, ctx, sent } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(1);
    const posted = (relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params;
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ nonce: "0", amount: "61", to: posted[0], data: posted[1], attempt: expect.any(String) });
    // Another grant covers the allowance: the recorded approve could still land after a sale spends it, so the sale waits.
    chain.allowances.set(allowanceKey, 10n ** 21n);
    await expect(ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(false);
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
    // Its nonce passes (it ran, or another call took it): it can never run, so it goes, and the allowance decides.
    chain.nonces.set(nonceKey, 1n);
    await ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(true);
    expect(relay.posts).toHaveLength(1);
  });

  it("another tab after a lost answer sends nothing within the hold, then only the same signed approve again: no new signature", async () => {
    const { relay, ctx, sent, signatures } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    anotherTab();
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(1);
    relay.state.mode = "ok";
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    } finally {
      clock.mockRestore();
    }
    expect(relay.posts).toHaveLength(2);
    expect(relay.posts[1]).toEqual(relay.posts[0]);
    expect(signatures()).toBe(1);
    expect(sent()).toEqual([]);
    // Its send confirmed: the record goes.
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("a lost approve landing right after the retry reads its nonce is never signed again at the next nonce: the same call goes, then its nonce settles it", async () => {
    // The resend of a call whose nonce is spent reverts on chain.
    const { chain, relay, ctx, sent, signed } = rail("fail-after-send", (post) => (post === 1 ? "success" : "reverted"));
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    let land = true;
    const reader: ChainReader = {
      ...chain,
      call: async (chainId, to, data) => {
        const out = await chain.call(chainId, to, data);
        if (land && data.startsWith("0x2d0335ab")) {
          land = false;
          chain.nonces.set(nonceKey, 1n);
          chain.allowances.set(allowanceKey, GAP);
        }
        return out;
      },
    };
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, reader, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    } finally {
      clock.mockRestore();
    }
    // Only the call signed at nonce 0 was ever sent, twice; nothing at nonce 1, nothing from the wallet.
    expect(signed()).toEqual(["0"]);
    expect(relay.posts).toHaveLength(2);
    expect(relay.posts[1]).toEqual(relay.posts[0]);
    expect(sent()).toEqual([]);
    // Next try: past its nonce the record goes, and the allowance it left is enough.
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect(signed()).toEqual(["0"]);
    expect(relay.posts).toHaveLength(2);
  });

  it("past its nonce a recorded approve can never run: the record goes and a new one is signed at the next nonce", async () => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    chain.nonces.set(nonceKey, 1n);
    relay.state.mode = "ok";
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(signed()).toEqual(["0", "1"]);
    expect(relay.posts).toHaveLength(2);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("the same terms resend the recorded approve; within the hold, other terms wait", async () => {
    const { relay, ctx, sent, signed } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(signed()).toEqual(["0"]);
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n });
    } finally {
      clock.mockRestore();
    }
    expect(relay.posts).toHaveLength(2);
    expect(relay.posts[1]).toEqual(relay.posts[0]);
    expect(relayedApproval(relay.posts[1])?.amount).toBe(100n);
    expect(sent()).toEqual([]);
  });

  it("a lost approve of 100 never keeps a gap of 1 waiting: past the hold, an approve of 1 is signed at the very same nonce, never the 100 again", async () => {
    const { relay, ctx, sent, signed } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n });
    } finally {
      clock.mockRestore();
    }
    // Both signed at nonce 0: whichever runs, the other never can.
    expect(signed()).toEqual(["0", "0"]);
    expect(relay.posts.map((p) => relayedApproval(p)?.amount)).toEqual([100n, 1n]);
    expect(sent()).toEqual([]);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("a replacement is recorded before it leaves: the new amount at the same nonce, under its own attempt", async () => {
    const { relay, ctx, signed } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    const replaced = readPendingGrant(allowance, BOOK.buyer)!;
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      // Its answer is lost too: the replacement is what stays recorded.
      await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    } finally {
      clock.mockRestore();
    }
    expect(signed()).toEqual(["0", "0"]);
    expect(relay.posts.map((p) => relayedApproval(p)?.amount)).toEqual([100n, 1n]);
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ amount: "1", nonce: "0" });
    expect(readPendingGrant(allowance, BOOK.buyer)!.attempt).not.toBe(replaced.attempt);
  });

  it("if the replaced approve lands first, the replacement reverts, the replaced one is put back, and the next try settles it by nonce and allowance", async () => {
    // The replacement's own receipt: reverted, its nonce taken by the approve of 100 that landed first.
    const { chain, relay, ctx, signed } = rail("fail-after-send", (post) => (post === 2 ? "reverted" : "success"));
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    const replaced = readPendingGrant(allowance, BOOK.buyer)!;
    relay.state.mode = "ok";
    const relayed = ctx.fetchImpl;
    let posts = 0;
    const fetchImpl = (async (...args: Parameters<typeof fetch>) => {
      const answer = await relayed(...args);
      // The approve of 100 lands as the replacement is sent: nonce 0 is spent, and the allowance is 100.
      if (++posts === 1) {
        chain.nonces.set(nonceKey, 1n);
        chain.allowances.set(allowanceKey, 100n);
      }
      return answer;
    }) as typeof fetch;
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, fetchImpl, auth: allowance, active: true, amount: 1n })).rejects.toThrow();
    } finally {
      clock.mockRestore();
    }
    expect(readPendingGrant(allowance, BOOK.buyer)).toEqual(replaced);
    // Next try: past its nonce the record goes, and the 100 it left covers the gap of 1. Nothing more is signed.
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect(signed()).toEqual(["0", "0"]);
  });

  it("a replacement the relay refuses as not sent puts the replaced approve back: that one may still run", async () => {
    const { relay, ctx } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    const replaced = readPendingGrant(allowance, BOOK.buyer)!;
    relay.state.mode = "refuse";
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n })).rejects.toThrow();
    } finally {
      clock.mockRestore();
    }
    expect(readPendingGrant(allowance, BOOK.buyer)).toEqual(replaced);
    expect(await exactAllowanceReady(allowance, BOOK.buyer, 1n, ctx.reader)).toBe(false);
  });

  it("a replacement refused as not sent whose replaced approve can't be written back keeps its own record at that nonce: a reopen still waits", async () => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "refuse";
    // The replacement's record is written; the write putting the approve of 100 back in its place fails.
    const setItem = Storage.prototype.setItem;
    let writes = 0;
    const failing = vi.spyOn(Storage.prototype, "setItem").mockImplementation(function (this: Storage, key: string, value: string) {
      if (key === recordKey && ++writes > 1) throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
      setItem.call(this, key, value);
    });
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n })).rejects.toThrow();
    } finally {
      clock.mockRestore();
      failing.mockRestore();
    }
    expect(writes).toBe(2);
    // The approve of 100 may still run at nonce 0: the replacement's record, at that very nonce, is what holds the sale.
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ amount: "1", nonce: "0" });
    // A reopen, with another grant covering the allowance: the sale still waits, and nothing new is signed.
    anotherTab();
    chain.allowances.set(allowanceKey, 10n ** 21n);
    expect(await exactAllowanceReady(allowance, BOOK.buyer, 1n, ctx.reader)).toBe(false);
    await expect(ensureAuthorization({ ...ctx, auth: allowance, requiredWei: 1n, exact: true })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(signed()).toEqual(["0", "0"]);
  });

  /** A lost approve of `lost` first (none for a first send), so that the approve of GAP is sent anew, again, or over it. */
  const SENDS = [
    ["a first send", null],
    ["a resend", GAP],
    ["a replacement", 100n],
  ] as const;
  const FOREIGN = `0x${"ab".repeat(32)}` as Hex;

  it.each(SENDS)("%s the relay answers with another transaction's hash keeps its record while its nonce can run, a covered allowance notwithstanding", async (_, lost) => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    if (lost !== null) await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: lost })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    // That hash succeeded, and another grant covers the allowance; nonce 0 is still unspent.
    const fetchImpl = (async () => {
      chain.receipts.set(FOREIGN, "success");
      chain.allowances.set(allowanceKey, 10n ** 21n);
      return new Response(JSON.stringify({ ok: true, txHash: FOREIGN }), { status: 200 });
    }) as unknown as typeof fetch;
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, fetchImpl, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    } finally {
      clock.mockRestore();
    }
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ amount: "61", nonce: "0" });
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(false);
    // Its nonce passes with the allowance in place: now the record goes, and nothing more is signed or sent.
    const before = signed();
    chain.nonces.set(nonceKey, 1n);
    await ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect(signed()).toEqual(before);
  });

  it.each(SENDS)("%s whose receipt reads before its nonce does keeps its record, then settles once the nonce read catches up", async (_, lost) => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    if (lost !== null) await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: lost })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    // The approve runs and its receipt reads, but the node the nonce is read from still answers 0.
    const fetchImpl = (async (...args: Parameters<typeof fetch>) => {
      const answer = await relay.fetchImpl(...args);
      chain.nonces.set(nonceKey, 0n);
      return answer;
    }) as typeof fetch;
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, fetchImpl, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    } finally {
      clock.mockRestore();
    }
    expect(chain.allowances.get(allowanceKey)).toBe(GAP);
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ amount: "61", nonce: "0" });
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(false);
    const before = { signed: signed(), posts: relay.posts.length };
    chain.nonces.set(nonceKey, 1n);
    await ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect({ signed: signed(), posts: relay.posts.length }).toEqual(before);
  });

  it.each(SENDS)("%s whose receipt succeeds with its nonce passed but the allowance short (another call took the nonce) didn't take effect: the record goes, and the next try approves anew", async (_, lost) => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    if (lost !== null) await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: lost })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    const fetchImpl = (async () => {
      chain.receipts.set(FOREIGN, "success");
      chain.nonces.set(nonceKey, 1n);
      return new Response(JSON.stringify({ ok: true, txHash: FOREIGN }), { status: 200 });
    }) as unknown as typeof fetch;
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      // Not "still confirming": nothing recorded is left to land, so trying again is what helps.
      await expect(setAuthorization({ ...ctx, fetchImpl, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(GrantNotInEffectError);
    } finally {
      clock.mockRestore();
    }
    // Nonce 0 is spent, so that approve can never run: its record goes, and the allowance says no.
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(false);
    const before = signed().length;
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(signed().slice(before)).toEqual(["1"]);
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(true);
  });

  it("a grant past its nonce whose allowance can't then be read says it couldn't be checked, not that it didn't take effect", async () => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    relay.state.mode = "ok";
    let down = false;
    const reader: ChainReader = {
      ...chain,
      call: async (chainId, to, data) => {
        if (down && data.startsWith("0xdd62ed3e")) throw new Error("upstream unavailable");
        return chain.call(chainId, to, data);
      },
    };
    // That hash succeeded and nonce 0 is spent; the allowance read that follows fails.
    const fetchImpl = (async () => {
      chain.receipts.set(FOREIGN, "success");
      chain.nonces.set(nonceKey, 1n);
      down = true;
      return new Response(JSON.stringify({ ok: true, txHash: FOREIGN }), { status: 200 });
    }) as unknown as typeof fetch;
    await expect(setAuthorization({ ...ctx, reader, fetchImpl, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(GrantUncheckedError);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    // Read again, it is short: a new approve is signed at the next nonce.
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(signed()).toEqual(["0", "1"]);
  });

  it("a successful receipt whose nonce can't be read keeps its record: no answer is no proof", async () => {
    const { chain, relay, ctx } = rail("fail-after-send");
    relay.state.mode = "ok";
    let down = false;
    const reader: ChainReader = {
      ...chain,
      call: async (chainId, to, data) => {
        if (down && data.startsWith("0x2d0335ab")) throw new Error("upstream unavailable");
        return chain.call(chainId, to, data);
      },
    };
    const fetchImpl = (async (...args: Parameters<typeof fetch>) => {
      const answer = await relay.fetchImpl(...args);
      down = true;
      return answer;
    }) as typeof fetch;
    await expect(setAuthorization({ ...ctx, reader, fetchImpl, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ amount: "61", nonce: "0" });
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, reader)).toBe(false);
    down = false;
    await ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it.each(SENDS)("%s the relay answers with another transaction's REVERTED hash keeps its record while its nonce can run: a reopen still waits", async (_, lost) => {
    const { chain, relay, ctx } = rail("fail-after-send");
    if (lost !== null) await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: lost })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    // That hash reverted, another grant covers the allowance, and nonce 0 is unspent: the signed approve may still run.
    const fetchImpl = (async () => {
      chain.receipts.set(FOREIGN, "reverted");
      chain.allowances.set(allowanceKey, 10n ** 21n);
      return new Response(JSON.stringify({ ok: true, txHash: FOREIGN }), { status: 200 });
    }) as unknown as typeof fetch;
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, fetchImpl, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    } finally {
      clock.mockRestore();
    }
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ amount: "61", nonce: "0" });
    // A reopen: the sale still waits on it, and nothing new is signed.
    anotherTab();
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(false);
    await expect(ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true })).rejects.toBeInstanceOf(AuthorizationPendingError);
  });

  it("control: a first send the relay refused and marked as not sent leaves no record: an allowance another grant covers is ready", async () => {
    const { chain, relay, ctx } = rail("refuse");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toThrow();
    expect(relay.posts).toHaveLength(1);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    chain.allowances.set(allowanceKey, 10n ** 21n);
    expect(await exactAllowanceReady(allowance, BOOK.buyer, GAP, ctx.reader)).toBe(true);
  });

  it("control: its own hash reverted with its nonce passed (another call took it) lets the record go: that approve can never run", async () => {
    const { chain, relay, ctx } = rail("fail-after-send", () => "reverted");
    relay.state.mode = "ok";
    const fetchImpl = (async (...args: Parameters<typeof fetch>) => {
      const answer = await relay.fetchImpl(...args);
      chain.nonces.set(nonceKey, 1n);
      return answer;
    }) as typeof fetch;
    await expect(setAuthorization({ ...ctx, fetchImpl, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(TxRevertedError);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("a replacement is signed only at the replaced approve's own nonce: one that moves before the signature asks the wallet nothing", async () => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    const replaced = readPendingGrant(allowance, BOOK.buyer)!;
    relay.state.mode = "ok";
    // The approve of 100 lands between the read that finds it open and the one a replacement would be signed at.
    let reads = 0;
    const reader: ChainReader = {
      ...chain,
      call: async (chainId, to, data) => {
        const out = await chain.call(chainId, to, data);
        if (data.startsWith("0x2d0335ab") && ++reads === 1) {
          chain.nonces.set(nonceKey, 1n);
          chain.allowances.set(allowanceKey, 100n);
        }
        return out;
      },
    };
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, reader, auth: allowance, active: true, amount: 1n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    } finally {
      clock.mockRestore();
    }
    expect(signed()).toEqual(["0"]);
    expect(relay.posts).toHaveLength(1);
    expect(readPendingGrant(allowance, BOOK.buyer)).toEqual(replaced);
    // Next try: past its nonce the record goes, and the 100 it left covers the gap of 1.
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect(signed()).toEqual(["0"]);
  });

  it("a record whose nonce is ahead of the token's is never replaced by a signature at that future nonce", async () => {
    const { relay, ctx, signatures } = rail("fail-after-send");
    relay.state.mode = "ok";
    const record = { at: Date.now() - PENDING_GRANT_HOLD_MS - 1, attempt: "ahead", amount: "100", to: MANA, data: signedApprove(100n), nonce: "5" };
    localStorage.setItem(recordKey, JSON.stringify(record));
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 1n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(signatures()).toBe(0);
    expect(relay.posts).toHaveLength(0);
    expect(readPendingGrant(allowance, BOOK.buyer)).toEqual(record);
  });

  it("with the relay off, a recorded approve that may still run says the approval can't be sent from here, not that it is confirming", async () => {
    const { ctx } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    (window as { __SHOP_CHAIN__?: unknown }).__SHOP_CHAIN__ = { gasless: false };
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(ExactGrantUnavailableError);
  });

  it("a record whose call is not this account's approve of its amount is none: nothing is sent from it", async () => {
    const { relay, ctx, signed } = rail("fail-after-send");
    relay.state.mode = "ok";
    const record = { at: Date.now() - PENDING_GRANT_HOLD_MS - 1, attempt: "odd", amount: "61", to: MANA, nonce: "0" };
    for (const data of ["0x00", signedApprove(62n)]) {
      localStorage.setItem(recordKey, JSON.stringify({ ...record, data }));
      expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    }
    localStorage.setItem(recordKey, JSON.stringify({ ...record, data: signedApprove(61n), to: BOOK.collection }));
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    // A fresh approve goes instead, recorded over it.
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(signed()).toEqual(["0"]);
    expect(relayedApproval(relay.posts[0])?.amount).toBe(GAP);
  });

  it("two windows approving at once: the one holding the grant's lock signs and sends; the other asks the wallet nothing", async () => {
    const { relay, wallet, ctx } = rail("fail-after-send");
    relay.state.mode = "ok";
    const first = signingLater(wallet);
    const pending = setAuthorization({ ...ctx, provider: first.provider, auth: allowance, active: true, amount: GAP });
    await vi.waitFor(() => expect(first.asked()).toBe(1));
    // The first is still in the wallet: the second finds the lock taken and asks nothing.
    const second = signingLater(wallet);
    await expect(setAuthorization({ ...ctx, provider: second.provider, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(second.asked()).toBe(0);
    first.sign();
    await pending;
    expect(relay.posts).toHaveLength(1);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("a record another window left while this one was signing is never overwritten, and this approve is not sent", async () => {
    const { relay, wallet, ctx } = rail("fail-after-send");
    relay.state.mode = "ok";
    const signer = signingLater(wallet);
    const pending = setAuthorization({ ...ctx, provider: signer.provider, auth: allowance, active: true, amount: GAP });
    await vi.waitFor(() => expect(signer.asked()).toBe(1));
    const other = { at: Date.now(), attempt: "other", amount: "61", to: MANA, data: signedApprove(61n), nonce: "0" };
    localStorage.setItem(recordKey, JSON.stringify(other));
    signer.sign();
    await expect(pending).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(0);
    expect(readPendingGrant(allowance, BOOK.buyer)?.attempt).toBe("other");
  });

  it("a purchase cancelled while the wallet signs records and sends nothing: an approval for another gap starts right after", async () => {
    const { relay, wallet, ctx, signed } = rail("fail-after-send");
    relay.state.mode = "ok";
    let live = true;
    const signer = signingLater(wallet);
    const pending = setAuthorization({ ...ctx, provider: signer.provider, live: () => live, auth: allowance, active: true, amount: GAP });
    await vi.waitFor(() => expect(signer.asked()).toBe(1));
    live = false;
    signer.sign();
    await expect(pending).rejects.toBeInstanceOf(GrantCancelledError);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
    expect(relay.posts).toHaveLength(0);
    // Nothing recorded, so nothing waits: another gap is approved at once.
    await setAuthorization({ ...ctx, live: () => true, auth: allowance, active: true, amount: 1n });
    expect(relay.posts.map((p) => relayedApproval(p)?.amount)).toEqual([1n]);
    expect(signed()).toEqual(["0", "0"]);
  });

  it("a replacement cancelled while the wallet signs it leaves the approve it was to replace recorded as it was, and sends nothing", async () => {
    const { relay, wallet, ctx } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: 100n })).rejects.toBeInstanceOf(AuthorizationPendingError);
    const replaced = readPendingGrant(allowance, BOOK.buyer)!;
    relay.state.mode = "ok";
    let live = true;
    const signer = signingLater(wallet);
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      const pending = setAuthorization({ ...ctx, provider: signer.provider, live: () => live, auth: allowance, active: true, amount: 1n });
      await vi.waitFor(() => expect(signer.asked()).toBe(1));
      live = false;
      signer.sign();
      await expect(pending).rejects.toBeInstanceOf(GrantCancelledError);
    } finally {
      clock.mockRestore();
    }
    expect(readPendingGrant(allowance, BOOK.buyer)).toEqual(replaced);
    expect(relay.posts).toHaveLength(1);
  });

  it("a cancelled purchase sends no recorded approve again, nor a replacement: the record stays as it was", async () => {
    const { relay, ctx } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    const recorded = readPendingGrant(allowance, BOOK.buyer);
    relay.state.mode = "ok";
    const clock = later(PENDING_GRANT_HOLD_MS + 1);
    try {
      await expect(setAuthorization({ ...ctx, live: () => false, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(GrantCancelledError);
      await expect(setAuthorization({ ...ctx, live: () => false, auth: allowance, active: true, amount: 1n })).rejects.toBeInstanceOf(GrantCancelledError);
    } finally {
      clock.mockRestore();
    }
    expect(relay.posts).toHaveLength(1);
    expect(readPendingGrant(allowance, BOOK.buyer)).toEqual(recorded);
  });

  it("without Web Locks an exact grant asks the wallet nothing", async () => {
    const { relay, wallet, ctx } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, locks: null, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(ExactGrantUnavailableError);
    expect(wallet.requests).toEqual([]);
    expect(relay.posts).toHaveLength(0);
  });

  it("relay only: a refusal the relay marked as not sent sends nothing from the wallet, and its record goes", async () => {
    const { relay, ctx, sent } = rail("refuse");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toThrow();
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("relay only: with the relay off an exact grant sends nothing", async () => {
    (window as { __SHOP_CHAIN__?: unknown }).__SHOP_CHAIN__ = { gasless: false };
    const { relay, wallet, ctx, sent } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(ExactGrantUnavailableError);
    expect(wallet.requests.filter((r) => r.method === "eth_signTypedData_v4" || r.method === "eth_sendTransaction")).toEqual([]);
    expect(relay.posts).toHaveLength(0);
    expect(sent()).toEqual([]);
  });

  it("a grant this browser cannot record is not sent", async () => {
    const { relay, ctx, sent, signatures } = rail("fail-after-send");
    const setItem = Storage.prototype.setItem;
    const full = vi.spyOn(Storage.prototype, "setItem").mockImplementation(function (this: Storage, key: string, value: string) {
      if (key.startsWith("dcl-shop-exact-grant:")) throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
      setItem.call(this, key, value);
    });
    try {
      await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(GrantNotRecordedError);
    } finally {
      full.mockRestore();
    }
    expect(signatures()).toBe(1);
    expect(relay.posts).toHaveLength(0);
    expect(sent()).toEqual([]);
  });

  it("an ordinary grant keeps its behavior: a lost answer falls back to the wallet, in full", async () => {
    const { ctx, sent } = rail("fail-after-send");
    await setAuthorization({ ...ctx, auth: allowance, active: true });
    expect(sent()).toEqual([maxUint256]);
  });
});
