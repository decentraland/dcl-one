// The shared MANA approval path's amounts: a grant is in full unless an exact amount is named (the CreditsManager a
// credits + MANA split pays through), a revoke is always zero, and an exact amount is only ever a positive allowance.

import { decodeFunctionData, maxUint256, type Hex } from "viem";
import { afterEach, describe, expect, it } from "vitest";

import { ERC20_ABI } from "@shop/checkout/chain/abi";
import { CHAIN } from "@shop/model/chain";
import { BOOK } from "@shop/payment/fixtures";
import { requireContracts } from "../chain/contracts";
import { createFakeChain, createFakeRelay, createFakeWallet } from "../fakes";
import {
  AuthorizationKind,
  AuthorizationPendingError,
  encodeAuthorizationCall,
  ensureAuthorization,
  getManaSpendingAuthorization,
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

describe("an exact grant whose relayed approve may still land", () => {
  afterEach(() => sessionStorage.clear());
  const GAP = 61n;

  /** A relay in `mode` that does not apply what it is sent, and a wallet whose approves the chain confirms. */
  function rail(mode: "fail-after-send" | "refuse") {
    const chain = createFakeChain();
    const relay = createFakeRelay({ mode });
    let n = 0;
    const wallet = createFakeWallet({
      onSend: () => {
        const hash = `0x${(++n).toString(16).padStart(64, "0")}` as Hex;
        chain.receipts.set(hash, "success");
        return hash;
      },
    });
    const ctx = { provider: wallet, account: BOOK.buyer, walletKind: "web3" as const, reader: chain, relayerUrl: "https://node.test/transactions-api/v1", fetchImpl: relay.fetchImpl };
    const sent = () => wallet.sent.map((tx) => approved(tx.data));
    return { chain, relay, wallet, ctx, sent };
  }

  it("a lost answer is pending, never doubled from the wallet; a quick retry sends nothing; it resolves once the allowance shows", async () => {
    const { chain, relay, ctx, sent } = rail("fail-after-send");
    await expect(setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
    expect(readPendingGrant(allowance, BOOK.buyer)).not.toBeNull();
    await expect(ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true })).rejects.toBeInstanceOf(AuthorizationPendingError);
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
    chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer}:${MANAGER.toLowerCase()}`, GAP);
    await ensureAuthorization({ ...ctx, auth: allowance, requiredWei: GAP, exact: true });
    // Covered, so nothing is granted; the record stays, since that allowance may be another grant's.
    expect(readPendingGrant(allowance, BOOK.buyer)).not.toBeNull();
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([]);
  });

  it("a refusal the relay marked as not sent lets the wallet grant exactly the amount", async () => {
    const { relay, ctx, sent } = rail("refuse");
    await setAuthorization({ ...ctx, auth: allowance, active: true, amount: GAP });
    expect(relay.posts).toHaveLength(1);
    expect(sent()).toEqual([GAP]);
    expect(readPendingGrant(allowance, BOOK.buyer)).toBeNull();
  });

  it("an ordinary grant keeps its behavior: a lost answer falls back to the wallet, in full", async () => {
    const { ctx, sent } = rail("fail-after-send");
    await setAuthorization({ ...ctx, auth: allowance, active: true });
    expect(sent()).toEqual([maxUint256]);
  });
});
