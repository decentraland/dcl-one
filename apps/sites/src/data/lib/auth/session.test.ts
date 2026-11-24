import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { AuthIdentity } from "./types";

const site: AuthIdentity = {
  signer: `0x${"a".repeat(40)}`,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"1".repeat(64)}` },
  expiration: new Date(Date.now() + 86400000).toISOString(), authChain: [],
};
const explorer = { ...site, signer: `0x${"b".repeat(40)}` };
let storage: Map<string, string>;
beforeEach(() => {
  vi.resetModules();
  storage = new Map([["dcl:auth:identity:v1", JSON.stringify(site)]]);
  const browser = Object.assign(new EventTarget(), {
    localStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
    },
  });
  vi.stubGlobal("window", browser);
  vi.stubGlobal("document", { cookie: "unchanged" });
});
afterEach(() => vi.unstubAllGlobals());

it("uses the explorer identity without modifying the separate website session", async () => {
  const session = await import("./session");
  expect(session.getIdentity()?.signer).toBe(site.signer);
  const listener = vi.fn();
  const unsubscribe = session.subscribe(listener);
  session.setHostIdentity(explorer);
  expect(session.getIdentity()).toBe(explorer);
  expect(listener).toHaveBeenCalledOnce();
  expect(storage.get(session.SESSION_STORAGE_KEY)).toBe(JSON.stringify(site));
  expect(document.cookie).toBe("unchanged");
  session.setHostIdentity(null);
  expect(session.getIdentity()).toBeNull();
  expect(() => session.setIdentity(site)).toThrow("Use the explorer");
  session.setHostIdentity(undefined);
  expect(session.getIdentity()?.signer).toBe(site.signer);
  unsubscribe();
});

it("does not let a website storage event replace the active explorer wallet", async () => {
  const session = await import("./session");
  const unsubscribe = session.subscribe(vi.fn());
  session.setHostIdentity(explorer);
  window.dispatchEvent(Object.assign(new Event("storage"), { key: session.SESSION_STORAGE_KEY }));
  expect(session.getIdentity()).toBe(explorer);
  unsubscribe();
  session.setHostIdentity(undefined);
});

it("leaves the account out of page views when another tab signs it out", async () => {
  const session = await import("./session");
  const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
  const { sessionChangePending } = await import("./session-change");
  const unsubscribe = session.subscribe(vi.fn());
  expect(session.getIdentity()?.signer).toBe(site.signer);
  storage.delete(session.SESSION_STORAGE_KEY);
  window.dispatchEvent(Object.assign(new Event("storage"), { key: session.SESSION_STORAGE_KEY, newValue: null }));
  expect(session.getIdentity()).toBeNull();
  expect(trackedUserDropped()).toBe(true);
  expect(sessionChangePending()).toBe(true);
  unsubscribe();
});

it("keeps the account in page views when another tab's change leaves it signed in, or signs one in", async () => {
  const session = await import("./session");
  const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
  const unsubscribe = session.subscribe(vi.fn());
  window.dispatchEvent(Object.assign(new Event("storage"), { key: session.SESSION_STORAGE_KEY }));
  expect(session.getIdentity()?.signer).toBe(site.signer);
  expect(trackedUserDropped()).toBe(false);
  unsubscribe();

  vi.resetModules();
  storage.clear();
  const fresh = await import("./session");
  const tracked = await import("@/core/lib/telemetry/tracked-user");
  const stop = fresh.subscribe(vi.fn());
  storage.set(fresh.SESSION_STORAGE_KEY, JSON.stringify(site));
  window.dispatchEvent(Object.assign(new Event("storage"), { key: fresh.SESSION_STORAGE_KEY }));
  expect(fresh.getIdentity()?.signer).toBe(site.signer);
  expect(tracked.trackedUserDropped()).toBe(false);
  stop();
});

it("does not treat the Explorer's account as signed out by a website storage event", async () => {
  const session = await import("./session");
  const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
  const unsubscribe = session.subscribe(vi.fn());
  session.setHostIdentity(explorer);
  storage.delete(session.SESSION_STORAGE_KEY);
  window.dispatchEvent(Object.assign(new Event("storage"), { key: session.SESSION_STORAGE_KEY, newValue: null }));
  expect(trackedUserDropped()).toBe(false);
  unsubscribe();
  session.setHostIdentity(undefined);
});
