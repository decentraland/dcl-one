import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { runnable, walletKey, WALLET_KEY_ENV } from "./wallet-key.mts";

const KEY = `0x${"11".repeat(32)}`;

describe("the test wallet's key", () => {
  it("refuses a signed-in page without a well-formed key", () => {
    assert.throws(() => walletKey({}), new RegExp(WALLET_KEY_ENV));
    assert.throws(() => walletKey({ [WALLET_KEY_ENV]: "0x1234" }), new RegExp(WALLET_KEY_ENV));
    assert.equal(walletKey({ [WALLET_KEY_ENV]: ` ${KEY} ` }), KEY);
  });

  it("without it a run leaves out the signed-in pages, says so once, and keeps every other page", () => {
    const pages = [{ id: "cart" }, { id: "cart-signed-in", signedIn: true }, { id: "my-items-signed-in", signedIn: true }];
    const said: string[] = [];
    assert.deepEqual(runnable(pages, {}, (l) => said.push(l)).map((p) => p.id), ["cart"]);
    assert.equal(said.length, 1);
    assert.match(said[0]!, /cart-signed-in, my-items-signed-in/);
    assert.deepEqual(runnable(pages, { [WALLET_KEY_ENV]: KEY }, (l) => said.push(l)).map((p) => p.id), ["cart", "cart-signed-in", "my-items-signed-in"]);
    assert.equal(said.length, 1);
  });
});
