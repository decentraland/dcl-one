// The shared MANA approval path's amounts: a grant is in full unless an exact amount is named (the CreditsManager a
// credits + MANA split pays through), a revoke is always zero, and an exact amount is only ever a positive allowance.

import { decodeFunctionData, maxUint256, type Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ERC20_ABI } from "@shop/checkout/chain/abi";
import { CHAIN } from "@shop/model/chain";
import { BOOK } from "@shop/payment/fixtures";
import { requireContracts } from "../chain/contracts";
import type { ChainReader } from "../chain/rpc";
import { createFakeChain, createFakeRelay, createFakeWallet } from "../fakes";
import {
  AuthorizationKind,
  AuthorizationPendingError,
  encodeAuthorizationCall,
  ensureAuthorization,
  getManaSpendingAuthorization,
  GrantNotRecordedError,
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
  });
  const GAP = 61n;
  const MANA = requireContracts(CHAIN.POLYGON).mana.address.toLowerCase();
  const allowanceKey = `${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${MANAGER.toLowerCase()}`;
  const nonceKey = `${MANA}:${BOOK.buyer.toLowerCase()}`;

  /**
   * A relay in `mode` whose sends the chain confirms (but whose approves it does not apply), and a wallet whose sends it
   * confirms too. `reader` is the chain unless a test puts a hook on its reads.
   */
  function rail(mode: "fail-after-send" | "refuse", relayed: (post: number) => "success" | "reverted" = () => "success") {
    const chain = createFakeChain();
    let posts = 0;
    const relay = createFakeRelay({ mode, onRelay: (hash) => chain.receipts.set(hash.toLowerCase(), relayed(++posts)) });
    let n = 0;
    const wallet = createFakeWallet({
      onSend: () => {
        const hash = `0x${(++n).toString(16).padStart(64, "0")}` as Hex;
        chain.receipts.set(hash, "success");
        return hash;
      },
    });
    const ctx = { provider: wallet, account: BOOK.buyer, walletKind: "web3" as const, reader: chain as ChainReader, relayerUrl: "https://node.test/transactions-api/v1", fetchImpl: relay.fetchImpl };
    const sent = () => wallet.sent.map((tx) => approved(tx.data));
    const signed = () => wallet.requests.filter((r) => r.method === "eth_signTypedData_v4").map((r) => (JSON.parse(String(r.params![1])) as { message: { nonce: string } }).message.nonce);
    const signatures = () => signed().length;
    return { chain, relay, wallet, ctx, sent, signed, signatures };
  }
  /** Another tab of this browser: its session storage starts empty, the local storage is the same. */
  const anotherTab = () => sessionStorage.clear();
  const later = (ms: number) => vi.spyOn(Date, "now").mockReturnValue(Date.now() + ms);

  it("a lost answer is recorded with the call it signed and its nonce, never doubled from the wallet; it holds until the allowance shows", async () => {
    const { chain, relay, ctx, sent } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
    const posted = (relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params;
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ via: "relay", nonce: "0", to: posted[0], data: posted[1] });
    await expect(ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
    chain.allowances.set(allowanceKey, GAP);
    await ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true });
    // Covered, so nothing is granted; the record stays while its nonce is unspent, since that allowance may be another's.
    expect(readPendingGrant(allowance, BOOK.buyer)).not.toBeNull();
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
  });

  it("another tab after a lost answer sends nothing within the hold, then only the same signed approve again: no new signature, never the wallet", async () => {
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

  it("the lost approve landing between the retry's reads is never signed again at the next nonce: the same call goes, then its nonce settles it", async () => {
    // The resend of a call whose nonce is spent reverts on chain.
    const { chain, relay, ctx, sent, signed, signatures } = rail("fail-after-send", (post) => (post === 1 ? "success" : "reverted"));
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    relay.state.mode = "ok";
    // The retry reads the nonce (still 0), then the allowance (short); right after that read the lost approve lands.
    let land = true;
    const reader: ChainReader = {
      ...chain,
      call: async (chainId, to, data) => {
        const out = await chain.call(chainId, to, data);
        if (land && data.startsWith("0xdd62ed3e")) {
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
    expect(signatures()).toBe(1);
    expect(relay.posts).toHaveLength(2);
  });

  it("past its nonce a recorded approve can never run: the record goes and the allowance decides whether to grant anew", async () => {
    const { chain, relay, ctx, signed } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    // Its nonce was spent (by it or by another call of this account), and the allowance is short: a new one is signed,
    // at the next nonce.
    chain.nonces.set(nonceKey, 1n);
    relay.state.mode = "ok";
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(signed()).toEqual(["0", "1"]);
    expect(relay.posts).toHaveLength(2);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("a refusal the relay marked as not sent lets the wallet grant exactly the amount", async () => {
    const { relay, ctx, sent } = rail("refuse");
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([GAP]);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("an approve the wallet lost the answer to (4900 after sending) is pending in every tab: no second one, from the wallet or the relay", async () => {
    const { relay, wallet, ctx, sent } = rail("refuse");
    wallet.next = "lost";
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(sent()).toEqual([GAP]);
    expect(readPendingGrant(allowance, BOOK.buyer)).toMatchObject({ via: "wallet", txHash: null });
    anotherTab();
    await expect(ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(sent()).toEqual([GAP]);
    expect(relay.posts).toHaveLength(1);
  });

  it("an approve the wallet sent with a hash holds until its receipt is in; then the allowance decides", async () => {
    const { chain, relay, ctx, sent } = rail("fail-after-send");
    const hash = `0x${"ab".repeat(32)}`;
    localStorage.setItem(
      `dcl-shop-exact-grant:v2:${BOOK.buyer.toLowerCase()}:${CHAIN.POLYGON}:${MANA}:${MANAGER.toLowerCase()}`,
      JSON.stringify({ at: Date.now(), via: "wallet", txHash: hash }),
    );
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(0);
    chain.receipts.set(hash, "success");
    chain.allowances.set(allowanceKey, GAP);
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
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
