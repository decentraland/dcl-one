// A managed (web2) session registers a NAME with credits: its in-app wallet only signs, so useCredits
// goes out as a relayed meta-transaction on the CreditsManager and the wallet itself sends nothing.

import { decodeFunctionData, type Hex } from "viem";
import { describe, expect, it } from "vitest";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { requireContracts } from "../checkout/chain/contracts";
import { OFFCHAIN_META_TX_ABI } from "../checkout/chain/abi";
import { createFakeChain, createFakeRelay, createFakeWallet } from "../checkout/fakes";
import { CHAIN } from "../model/chain";
import { relayedUseCreditsSubmitter, type NameUseCreditsArgs } from "./names";

const ARGS: NameUseCreditsArgs = {
  credits: [{ value: "1000", expiresAt: 2_000_000_000, salt: `0x${"01".repeat(32)}` }],
  creditsSignatures: ["0x5151"],
  externalCall: { target: "0x000000000000000000000000000000000000e0e0", selector: "0x11223344", data: "0x", expiresAt: 2_000_000_000, salt: "0x01" },
  customExternalCallSignature: "0x",
  maxUncreditedValue: "0",
  maxCreditedValue: "1000",
};

/** Only what an in-app wallet answers: its account and EIP-712 signatures. */
function inAppWallet(): Eip1193Provider & { methods: string[] } {
  const wallet = createFakeWallet();
  const methods: string[] = [];
  return {
    methods,
    async request(args) {
      methods.push(args.method);
      if (args.method === "eth_accounts" || args.method === "eth_signTypedData_v4") return wallet.request(args);
      throw Object.assign(new Error(`unsupported ${args.method}`), { code: 4200 });
    },
  };
}

describe("relayedUseCreditsSubmitter", () => {
  const manager = requireContracts(CHAIN.POLYGON).creditsManager!;

  it("signs useCredits as a meta-transaction and hands it to the relay; nothing is sent by the wallet", async () => {
    const chain = createFakeChain();
    const relay = createFakeRelay({ chain });
    const wallet = inAppWallet();
    const submit = relayedUseCreditsSubmitter(wallet, { reader: chain, relayerUrl: "https://node.test/transactions-api/v1", fetchImpl: relay.fetchImpl });
    const hash = await submit({ chainId: CHAIN.POLYGON, manager: manager.address, buyer: "0x0000000000000000000000000000000000000b0b", args: ARGS });
    expect(hash).toMatch(/^0x[0-9a-f]{64}$/);
    expect(wallet.methods).toEqual(["eth_signTypedData_v4"]);
    expect(relay.posts).toHaveLength(1);
    const [to, data] = (relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params;
    expect(to.toLowerCase()).toBe(manager.address.toLowerCase());
    expect(decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data }).functionName).toBe("executeMetaTransaction");
  });

  it("refuses a credit that names an unknown manager, and reports an unreachable relay as unobservable", async () => {
    const chain = createFakeChain();
    const wallet = inAppWallet();
    const unknown = relayedUseCreditsSubmitter(wallet, { reader: chain, fetchImpl: createFakeRelay().fetchImpl });
    await expect(unknown({ chainId: CHAIN.POLYGON, manager: "0x000000000000000000000000000000000000c0de", buyer: "0x0000000000000000000000000000000000000b0b", args: ARGS })).rejects.toThrow(/Credits manager/);
    const down = relayedUseCreditsSubmitter(wallet, { reader: chain, relayerUrl: "https://node.test/transactions-api/v1", fetchImpl: createFakeRelay({ mode: "down" }).fetchImpl });
    await expect(down({ chainId: CHAIN.POLYGON, manager: manager.address, buyer: "0x0000000000000000000000000000000000000b0b", args: ARGS })).rejects.toMatchObject({ name: "RelayerUnreachableError" });
  });
});
