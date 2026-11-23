import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({
  sidLoader: () => ({ sid: "s1", wrap: <T,>(value: T) => value }),
}));
const loadInviteReferral = vi.fn();
vi.mock("@/data/lib/catalyst/landings/referral.server", () => ({
  loadInviteReferral: (...args: unknown[]) => loadInviteReferral(...args),
}));

import { loader } from "./invite.($referrer)";

const ADDRESS = "0xabcdef0123456789abcdef0123456789abcdef01";

const read = (referrer?: string, signal?: AbortSignal) =>
  loader({
    request: new Request(`https://site.test/invite${referrer ? `/${referrer}` : ""}`, { signal }),
    context: {},
    params: { referrer },
  } as never);

const status = async (referrer?: string) => {
  try {
    await read(referrer);
  } catch (error) {
    return (error as { init?: { status?: number } }).init?.status;
  }
  return 200;
};

describe("Invite loader", () => {
  beforeEach(() => {
    loadInviteReferral.mockReset();
  });

  describe("when the path names no referrer", () => {
    it("should answer 404 without reading a profile", async () => {
      expect(await status()).toBe(404);
      expect(loadInviteReferral).not.toHaveBeenCalled();
    });
  });

  describe("when the referrer names no wallet", () => {
    it("should answer 404", async () => {
      loadInviteReferral.mockResolvedValue({ referrer: { name: null, address: null }, referral: null });
      expect(await status("nobody")).toBe(404);
    });
  });

  describe("when the referrer resolves to a wallet", () => {
    it("should return the referrer", async () => {
      const referrer = { name: "Jane", address: ADDRESS };
      loadInviteReferral.mockResolvedValue({ referrer, referral: null });
      expect(await read("Jane")).toMatchObject({ referrerParam: "Jane", referrer });
    });
  });

  describe("when the lookup fails", () => {
    let logged: ReturnType<typeof vi.spyOn>;

    beforeEach(() => {
      logged = vi.spyOn(console, "error").mockImplementation(() => {});
      loadInviteReferral.mockRejectedValue(new Error("down"));
    });

    afterEach(() => {
      logged.mockRestore();
    });

    it("should keep the page, with no referrer, and log the failure", async () => {
      expect(await read(ADDRESS)).toMatchObject({ referrerParam: ADDRESS, referrer: null });
      expect(logged).toHaveBeenCalledWith("[invite] referrer lookup failed: down");
    });

    describe("and the request was aborted", () => {
      it("should not log", async () => {
        const aborted = new AbortController();
        aborted.abort();
        expect(await read(ADDRESS, aborted.signal)).toMatchObject({ referrer: null });
        expect(logged).not.toHaveBeenCalled();
      });
    });
  });
});
