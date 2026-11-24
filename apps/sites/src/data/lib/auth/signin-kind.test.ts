// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { browserWalletRdnsFor, forgetSignIn, recordSignIn, signInKindFor, UNRECORDED_BROWSER_WALLET } from "./signin-kind";

const A = "0x" + "ab".repeat(20);
const B = "0x" + "cd".repeat(20);

afterEach(() => forgetSignIn());

describe("the sign-in's record", () => {
  it("answers how this account signed in, for that account only, until it is forgotten", () => {
    expect(signInKindFor(A)).toBeNull();
    recordSignIn(A.toUpperCase().replace("0X", "0x"), "web3");
    expect(signInKindFor(A)).toBe("web3");
    expect(signInKindFor(B)).toBeNull();
    recordSignIn(B, "managed");
    expect(signInKindFor(B)).toBe("managed");
    expect(signInKindFor(A)).toBeNull();
    forgetSignIn();
    expect(signInKindFor(B)).toBeNull();
  });

  it("reads a damaged record as none", () => {
    localStorage.setItem("dcl:auth:signin-kind:v1", "{not json");
    expect(signInKindFor(A)).toBeNull();
    localStorage.setItem("dcl:auth:signin-kind:v1", JSON.stringify({ signer: A, kind: "other" }));
    expect(signInKindFor(A)).toBeNull();
  });
});

describe("when a browser wallet provider is recorded", () => {
  beforeEach(() => recordSignIn(A, "web3", "com.coinbase.wallet"));

  it("should identify the provider for that wallet only", () => {
    expect(browserWalletRdnsFor(A)).toBe("com.coinbase.wallet");
    expect(browserWalletRdnsFor(B)).toBeNull();
  });
});

describe("when a legacy Web3 sign-in comes without a provider", () => {
  beforeEach(() => localStorage.setItem("dcl:auth:signin-kind:v1", JSON.stringify({ signer: A, kind: "web3" })));

  it("should indicate that the browser wallet needs to be verified", () => {
    expect(browserWalletRdnsFor(A)).toBe(UNRECORDED_BROWSER_WALLET);
  });
});
