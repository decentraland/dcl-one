import { beforeEach, describe, expect, it, vi } from "vitest";

import { cachedProfile, flushProfileCache, PROFILE_CACHE_KEY, rememberProfiles, resetProfileCache } from "./profileCache";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const B = "0x1111111111111111111111111111111111111111";

/** A reload: what was written survives, the page's in-memory copy does not. */
function reload() {
  flushProfileCache();
  resetProfileCache({ keepStorage: true });
}

beforeEach(() => {
  resetProfileCache();
  vi.useRealTimers();
});

describe("the profiles this browser already resolved", () => {
  it("survive a reload, by lowercased address", () => {
    rememberProfiles([A], { [A]: { name: "Doki3D", picture: "https://example.org/f.png", claimed: true } });
    reload();
    expect(cachedProfile(A.toUpperCase().replace("0X", "0x"))).toEqual({ name: "Doki3D", picture: "https://example.org/f.png", claimed: true });
  });

  it("are written once per tick, however many answers land in it", async () => {
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    rememberProfiles([A], { [A]: { name: "a", picture: null, claimed: null } });
    rememberProfiles([B], { [B]: { name: "b", picture: null, claimed: null } });
    expect(setItem.mock.calls.filter(([key]) => key === PROFILE_CACHE_KEY)).toHaveLength(0);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(setItem.mock.calls.filter(([key]) => key === PROFILE_CACHE_KEY)).toHaveLength(1);
    setItem.mockRestore();
  });

  it("forget an address that came back without a profile", () => {
    rememberProfiles([A], { [A]: { name: "Doki3D", picture: null, claimed: true } });
    rememberProfiles([A], {});
    expect(cachedProfile(A)).toBeNull();
  });

  it("expire after a week", () => {
    vi.useFakeTimers({ now: Date.UTC(2026, 0, 1) });
    rememberProfiles([A], { [A]: { name: "Doki3D", picture: null, claimed: true } });
    reload();
    vi.setSystemTime(Date.UTC(2026, 0, 9));
    resetProfileCache({ keepStorage: true });
    expect(cachedProfile(A)).toBeNull();
  });

  it("keep the newest 500, in memory as in storage, and read corrupt storage as empty", () => {
    const many = Array.from({ length: 520 }, (_, i) => `0x${i.toString(16).padStart(40, "0")}`);
    vi.useFakeTimers({ now: Date.UTC(2026, 0, 1) });
    many.forEach((address, i) => {
      vi.setSystemTime(Date.UTC(2026, 0, 1) + i);
      rememberProfiles([address], { [address]: { name: `n${i}`, picture: null, claimed: null } });
    });
    expect(cachedProfile(many[0])).toBeNull();
    expect(cachedProfile(many[519])?.name).toBe("n519");
    reload();
    expect(cachedProfile(many[519])?.name).toBe("n519");
    expect(Object.keys(JSON.parse(localStorage.getItem(PROFILE_CACHE_KEY)!))).toHaveLength(500);

    localStorage.setItem(PROFILE_CACHE_KEY, "{not json");
    resetProfileCache({ keepStorage: true });
    expect(cachedProfile(B)).toBeNull();
  });

  it("only ever keep a face that is an http(s) URL", () => {
    rememberProfiles([A], { [A]: { name: "x", picture: "javascript:alert(1)", claimed: null } });
    expect(cachedProfile(A)).toBeNull();
    localStorage.setItem(PROFILE_CACHE_KEY, JSON.stringify({ [B]: { name: "y", picture: "data:image/svg+xml,<svg/>", claimed: null, at: Date.now() } }));
    resetProfileCache({ keepStorage: true });
    expect(cachedProfile(B)).toBeNull();
  });
});
