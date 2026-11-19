import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchProfileFlags, profileFlagsFrom, REFERRAL_FLAG } from "./flags";

afterEach(() => vi.unstubAllGlobals());

describe("profile flags", () => {
  it("turn Referral Rewards on only where the flag is exactly true", () => {
    expect(profileFlagsFrom({ [REFERRAL_FLAG]: true })).toEqual({ referral: true });
    expect(profileFlagsFrom({ [REFERRAL_FLAG]: "true" })).toEqual({ referral: false });
    expect(profileFlagsFrom({})).toEqual({ referral: false });
    expect(profileFlagsFrom(null)).toEqual({ referral: false });
  });

  it("are read from this site's own flags route", async () => {
    const fetchMock = vi.fn(async () => Response.json({ [REFERRAL_FLAG]: true, "dapps-other": true }));
    vi.stubGlobal("fetch", fetchMock);
    const signal = new AbortController().signal;

    await expect(fetchProfileFlags({ signal })).resolves.toEqual({ referral: true });

    expect(fetchMock).toHaveBeenCalledWith("/internal/dapps-flags", expect.objectContaining({ signal }));
  });

  it("fail when the route does, so a failure is not read as every flag off for a minute", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("no", { status: 502 })));

    await expect(fetchProfileFlags()).rejects.toThrow("502");
  });
});
