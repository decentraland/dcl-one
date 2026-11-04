import { beforeEach, describe, expect, it, vi } from "vitest";

import { cachedProfile, rememberProfiles, resetProfileCache } from "./profileCache";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const B = "0x1111111111111111111111111111111111111111";
const KEY = "dcl:shop:profiles:v1";

beforeEach(() => {
  localStorage.removeItem(KEY);
  resetProfileCache();
  vi.useRealTimers();
});

describe("the profiles this browser already resolved", () => {
  it("survive a reload, by lowercased address", () => {
    rememberProfiles([A], { [A]: { name: "Doki3D", picture: "https://example.org/f.png", claimed: true } });
    resetProfileCache();
    expect(cachedProfile(A.toUpperCase().replace("0X", "0x"))).toEqual({ name: "Doki3D", picture: "https://example.org/f.png", claimed: true });
  });

  it("forget an address that came back without a profile", () => {
    rememberProfiles([A], { [A]: { name: "Doki3D", picture: null, claimed: true } });
    rememberProfiles([A], {});
    expect(cachedProfile(A)).toBeNull();
  });

  it("expire after a week", () => {
    vi.useFakeTimers({ now: Date.UTC(2026, 0, 1) });
    rememberProfiles([A], { [A]: { name: "Doki3D", picture: null, claimed: true } });
    vi.setSystemTime(Date.UTC(2026, 0, 9));
    resetProfileCache();
    expect(cachedProfile(A)).toBeNull();
  });

  it("keep the newest 500, and read corrupt storage as empty", () => {
    const many = Array.from({ length: 520 }, (_, i) => `0x${i.toString(16).padStart(40, "0")}`);
    vi.useFakeTimers({ now: Date.UTC(2026, 0, 1) });
    many.forEach((address, i) => {
      vi.setSystemTime(Date.UTC(2026, 0, 1) + i);
      rememberProfiles([address], { [address]: { name: `n${i}`, picture: null, claimed: null } });
    });
    resetProfileCache();
    expect(cachedProfile(many[519])?.name).toBe("n519");
    expect(cachedProfile(many[0])).toBeNull();
    expect(Object.keys(JSON.parse(localStorage.getItem(KEY)!))).toHaveLength(500);

    localStorage.setItem(KEY, "{not json");
    resetProfileCache();
    expect(cachedProfile(B)).toBeNull();
  });
});
