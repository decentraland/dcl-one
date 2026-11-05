import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useCreatorProfile, useProfiles } from "../hooks";
import { invalidateShopQueries } from "../query";
import { rememberProfiles } from "./profileCache";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

beforeEach(() => {
  invalidateShopQueries();
  // The profile request never answers: whatever shows comes from what the browser already knew.
  vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
});
afterEach(() => vi.unstubAllGlobals());

describe("a creator this browser already resolved", () => {
  it("shows its name at once on a reload, while the profile request revalidates it", () => {
    rememberProfiles([A], { [A]: { name: "Doki3D", picture: null, claimed: true } });
    const { result } = renderHook(() => useCreatorProfile(A));
    expect(result.current).toEqual({ profile: { name: "Doki3D", picture: null, claimed: true }, loading: false });
    const list = renderHook(() => useProfiles([A]));
    expect(list.result.current[A]?.name).toBe("Doki3D");
  });

  it("still waits for an unknown creator, without inventing a name", () => {
    const { result } = renderHook(() => useCreatorProfile(A));
    expect(result.current).toEqual({ profile: null, loading: true });
  });
});
