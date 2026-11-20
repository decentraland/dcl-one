// The shared MANA approval path's amounts: a grant is in full unless an exact amount is named (the CreditsManager a
// credits + MANA split pays through), a revoke is always zero, and an exact amount is only ever a positive allowance.

import { decodeFunctionData, maxUint256, type Hex } from "viem";
import { describe, expect, it } from "vitest";

import { ERC20_ABI } from "@shop/checkout/chain/abi";
import { CHAIN } from "@shop/model/chain";
import { BOOK } from "@shop/payment/fixtures";
import { requireContracts } from "../chain/contracts";
import { createFakeChain, createFakeWallet } from "../fakes";
import { AuthorizationKind, encodeAuthorizationCall, ensureAuthorization, getManaSpendingAuthorization, setAuthorization, type ShopAuthorization } from "./authorizations";

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
