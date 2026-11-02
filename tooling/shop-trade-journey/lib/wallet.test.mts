// The testnet wallet: what it signs, what it sends and, above all, what it refuses. No network: the
// chain RPC is a recorded fake.
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { custom, keccak256, parseTransaction, recoverMessageAddress, recoverTypedDataAddress, type Hex } from "viem";
import { generatePrivateKey } from "viem/accounts";
import { answer, keyFrom, testnetWallet, WalletRefusal } from "./wallet.mts";

// A fresh key per run that only ever signs here.
const KEY = generatePrivateKey();

function fakeRpc() {
  const calls: Array<{ chainId: number; method: string; params: unknown }> = [];
  const transport = (chainId: number) =>
    custom({
      async request({ method, params }: { method: string; params?: unknown }) {
        calls.push({ chainId, method, params });
        switch (method) {
          case "eth_chainId":
            return `0x${chainId.toString(16)}`;
          case "eth_getTransactionCount":
            return "0x3";
          case "eth_estimateGas":
            return "0x5208";
          case "eth_gasPrice":
          case "eth_maxPriorityFeePerGas":
            return "0x3b9aca00";
          case "eth_getBlockByNumber":
            return { baseFeePerGas: "0x3b9aca00", number: "0x10" };
          case "eth_sendRawTransaction":
            return keccak256((params as [Hex])[0]);
          case "eth_getBalance":
            return "0xde0b6b3a7640000";
          default:
            throw new Error(`unexpected ${method}`);
        }
      },
    });
  return { calls, transport };
}

const typed = (chainId: number) =>
  JSON.stringify({
    domain: { name: "DecentralandMarketplace", version: "1.0.0", chainId, verifyingContract: "0x0000000000000000000000000000000000000001" },
    types: { EIP712Domain: [{ name: "name", type: "string" }], Trade: [{ name: "salt", type: "bytes32" }] },
    primaryType: "Trade",
    message: { salt: `0x${"11".repeat(32)}` },
  });

describe("testnetWallet", () => {
  it("starts on Amoy and refuses to be built for mainnet or Polygon", () => {
    const { transport } = fakeRpc();
    assert.equal(testnetWallet({ key: KEY, transport }).chainId(), 80002);
    assert.throws(() => testnetWallet({ key: KEY, chainId: 1, transport }), WalletRefusal);
    assert.throws(() => testnetWallet({ key: KEY, chainId: 137, transport }), WalletRefusal);
  });

  it("switches between the two testnets, tells the page, and refuses a switch to mainnet", async () => {
    const { transport } = fakeRpc();
    const w = testnetWallet({ key: KEY, transport });
    const told: number[] = [];
    w.onChainChanged = (id) => told.push(id);
    await w.request({ method: "wallet_switchEthereumChain", params: [{ chainId: "0xaa36a7" }] });
    assert.equal(await w.request({ method: "eth_chainId" }), "0xaa36a7");
    await assert.rejects(w.request({ method: "wallet_switchEthereumChain", params: [{ chainId: "0x89" }] }), WalletRefusal);
    assert.equal(w.chainId(), 11155111);
    assert.deepEqual(told, [11155111]);
  });

  it("signs a message and testnet typed data that recover to its address", async () => {
    const { transport } = fakeRpc();
    const w = testnetWallet({ key: KEY, transport });
    const message = "0x68656c6c6f" as Hex;
    const sig = (await w.request({ method: "personal_sign", params: [message, w.address] })) as Hex;
    assert.equal(await recoverMessageAddress({ message: { raw: message }, signature: sig }), w.address);
    const data = JSON.parse(typed(80002));
    const { EIP712Domain: _d, ...types } = data.types;
    const tsig = (await w.request({ method: "eth_signTypedData_v4", params: [w.address, typed(80002)] })) as Hex;
    assert.equal(await recoverTypedDataAddress({ ...data, types, signature: tsig }), w.address);
  });

  it("refuses typed data for a mainnet domain, and acting for another address", async () => {
    const { transport } = fakeRpc();
    const w = testnetWallet({ key: KEY, transport });
    await assert.rejects(w.request({ method: "eth_signTypedData_v4", params: [w.address, typed(137)] }), WalletRefusal);
    await assert.rejects(w.request({ method: "personal_sign", params: ["0x00", "0x000000000000000000000000000000000000dEaD"] }), WalletRefusal);
    await assert.rejects(w.request({ method: "eth_sign", params: [w.address, "0x00"] }), WalletRefusal);
  });

  it("signs a transaction locally for the current testnet and broadcasts it there", async () => {
    const { calls, transport } = fakeRpc();
    const w = testnetWallet({ key: KEY, transport });
    const hash = await w.request({ method: "eth_sendTransaction", params: [{ from: w.address, to: "0x0000000000000000000000000000000000000002", value: "0x1", data: "0x" }] });
    const raw = calls.find((c) => c.method === "eth_sendRawTransaction")!;
    assert.equal(raw.chainId, 80002);
    const tx = parseTransaction((raw.params as [Hex])[0]);
    assert.equal(tx.chainId, 80002);
    assert.deepEqual(w.sent, [hash]);
    await assert.rejects(w.request({ method: "eth_sendTransaction", params: [{ from: w.address, to: "0x0000000000000000000000000000000000000002", chainId: "0x1" }] }), WalletRefusal);
  });

  it("passes reads through to the testnet RPC", async () => {
    const { calls, transport } = fakeRpc();
    const w = testnetWallet({ key: KEY, transport });
    assert.equal(await w.request({ method: "eth_getBalance", params: [w.address, "latest"] }), "0xde0b6b3a7640000");
    assert.equal(calls.at(-1)!.chainId, 80002);
  });

  it("answers the page with an EIP-1193 error code on refusal", async () => {
    const { transport } = fakeRpc();
    const w = testnetWallet({ key: KEY, transport });
    const a = await answer(w, { method: "wallet_switchEthereumChain", params: [{ chainId: "0x1" }] });
    assert.equal(a.error?.code, 4001);
  });
});

describe("keyFrom", () => {
  it("reads a hex key with or without 0x, and names the variable, never the value, when it is wrong", () => {
    assert.equal(keyFrom({ K: KEY.slice(2) }, "K"), KEY);
    assert.throws(() => keyFrom({}, "K"), /K is not set/);
    assert.throws(
      () => keyFrom({ K: "0xnot-a-key-secret" }, "K"),
      (e: Error) => /K is not a 32-byte/.test(e.message) && !e.message.includes("secret"),
    );
  });
});
