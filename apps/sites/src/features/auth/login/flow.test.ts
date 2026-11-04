import { describe, expect, it } from "vitest";

import { failureState, isExpectedWalletError, isRejection, PENDING_REQUEST_HINT } from "./flow";

const rejected = Object.assign(new Error("User rejected the request."), { code: 4001 });
const pending = Object.assign(new Error("Request already pending"), { code: -32002 });
const unsupported = Object.assign(new Error("The method personal_sign is not supported"), { code: 4200 });
const locked = Object.assign(new Error("Wallet is locked"), { name: "ErrorUnlockingWallet" });
const broken = new Error("window.ethereum.enable is not a function");

describe("what a wallet failure tells the reader", () => {
  it("recognises a rejection by its code or by the wallet's words", () => {
    expect(isRejection(rejected)).toBe(true);
    expect(isRejection(new Error("MetaMask Tx Signature: User denied message signature."))).toBe(true);
    expect(isRejection(broken)).toBe(false);
    expect(isExpectedWalletError(pending)).toBe(true);
    expect(isExpectedWalletError(broken)).toBe(false);
  });

  it("never says the reader failed to confirm before the wallet asked them anything", () => {
    expect(failureState(rejected, false)).toEqual({ state: "error-connection-failed", detail: null });
    // A request still open in the wallet says what the reader can do about it.
    expect(failureState(pending, false)).toEqual({ state: "error-connection-failed", detail: PENDING_REQUEST_HINT });
    expect(failureState(pending, true)).toEqual({ state: "error-connection-failed", detail: PENDING_REQUEST_HINT });
    expect(failureState(broken, false)).toEqual({ state: "error-connection-failed", detail: broken.message });
  });

  it("names the refused signature, a locked wallet, an unsupported signature and anything else apart", () => {
    expect(failureState(rejected, true)).toEqual({ state: "error", detail: null });
    expect(failureState(locked, true)).toEqual({ state: "error-locked", detail: null });
    expect(failureState(locked, false)).toEqual({ state: "error-locked", detail: null });
    expect(failureState(unsupported, true)).toEqual({ state: "error-signature-unsupported", detail: null });
    expect(failureState(broken, true)).toEqual({ state: "error-generic", detail: broken.message });
    expect(failureState("not an error", true)).toEqual({ state: "error-generic", detail: null });
  });
});
