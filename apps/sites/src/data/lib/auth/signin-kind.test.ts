// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";

import { forgetSignIn, recordSignIn, signInKindFor } from "./signin-kind";

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
