// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { toShared } from "./single-sign-on";
import type { AuthIdentity } from "./types";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const B = "0x1111111111111111111111111111111111111111";

function identity(signer: string, daysLeft: number): AuthIdentity {
  return {
    signer,
    ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}` },
    expiration: new Date(Date.now() + daysLeft * 86_400_000).toISOString(),
    authChain: [{ type: "SIGNER", payload: signer, signature: "" }],
  };
}

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
});
afterEach(() => localStorage.clear());

describe("a session shared with decentraland.org's dapps", () => {
  it("is mirrored when this site signs in, and ended everywhere when it signs out", async () => {
    const session = await import("./session");
    session.setIdentity(identity(A, 30));
    expect(localStorage.getItem(`single-sign-on-${A}`)).not.toBeNull();
    expect(JSON.parse(localStorage.getItem(session.SESSION_STORAGE_KEY)!).signer).toBe(A);

    session.setIdentity(identity(B, 30));
    expect(localStorage.getItem(`single-sign-on-${A}`)).toBeNull();
    expect(localStorage.getItem(`single-sign-on-${B}`)).not.toBeNull();

    session.clearIdentity();
    expect(localStorage.getItem(`single-sign-on-${B}`)).toBeNull();
    expect(localStorage.getItem(session.SESSION_STORAGE_KEY)).toBeNull();
  });

  it("is adopted when a reader signed in on another decentraland.org dapp", async () => {
    localStorage.setItem(`single-sign-on-${B}`, JSON.stringify(toShared(identity(B, 30))));
    const session = await import("./session");
    expect(session.getIdentity()?.signer).toBe(B);
  });

  it("leaves this site's own record in charge while it holds a session", async () => {
    // Even against a later-expiring entry: session lengths differ between dapps, so expiry says nothing of recency.
    localStorage.setItem("dcl:auth:identity:v1", JSON.stringify(identity(A, 7)));
    localStorage.setItem(`single-sign-on-${B}`, JSON.stringify(toShared(identity(B, 30))));
    expect((await import("./session")).getIdentity()?.signer).toBe(A);
  });

  it("brings no account back after a sign-out, whoever else was stored", async () => {
    localStorage.setItem(`single-sign-on-${B}`, JSON.stringify(toShared(identity(B, 30))));
    const session = await import("./session");
    session.setIdentity(identity(A, 7));
    session.clearIdentity();
    expect(Object.keys(localStorage).filter((k) => k.startsWith("single-sign-on-"))).toEqual([]);
    vi.resetModules();
    expect((await import("./session")).getIdentity()).toBeNull();
  });

  it("follows a sign-in another tab makes", async () => {
    const session = await import("./session");
    const listener = vi.fn();
    const unsubscribe = session.subscribe(listener);
    localStorage.setItem(`single-sign-on-${B}`, JSON.stringify(toShared(identity(B, 30))));
    window.dispatchEvent(new StorageEvent("storage", { key: `single-sign-on-${B}` }));
    expect(listener).toHaveBeenCalled();
    expect(session.getIdentity()?.signer).toBe(B);
    unsubscribe();
  });

  it("signs this account out when another dapp signs it out, leaving other accounts' sessions", async () => {
    const session = await import("./session");
    const unsubscribe = session.subscribe(() => {});
    session.setIdentity(identity(A, 30));
    localStorage.setItem(`single-sign-on-${B}`, JSON.stringify(toShared(identity(B, 30))));
    localStorage.removeItem(`single-sign-on-${A}`);
    window.dispatchEvent(new StorageEvent("storage", { key: `single-sign-on-${A}`, newValue: null }));
    expect(localStorage.getItem(session.SESSION_STORAGE_KEY)).toBeNull();
    expect(localStorage.getItem(`single-sign-on-${B}`)).not.toBeNull();
    expect(session.getIdentity()?.signer).toBe(B);
    unsubscribe();
  });

  it("keeps this account when another dapp ends some other account's session", async () => {
    const session = await import("./session");
    const unsubscribe = session.subscribe(() => {});
    session.setIdentity(identity(A, 30));
    window.dispatchEvent(new StorageEvent("storage", { key: `single-sign-on-${B}`, newValue: null }));
    expect(session.getIdentity()?.signer).toBe(A);
    expect(localStorage.getItem(session.SESSION_STORAGE_KEY)).not.toBeNull();
    unsubscribe();
  });
});
