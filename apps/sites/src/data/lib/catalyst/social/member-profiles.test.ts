import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "../client";
import { asAddress, fetchMemberProfiles, memberName, sameOriginContentUrl } from "./member-profiles";

const BASE = "https://node.example";
const FIRST = "0x1111111111111111111111111111111111111111";
const SECOND = "0x2222222222222222222222222222222222222222";
const THIRD = "0x3333333333333333333333333333333333333333";

function replyWith(body: unknown, init?: ResponseInit) {
  return vi.fn(async () => new Response(typeof body === "string" ? body : JSON.stringify(body), init));
}

describe("asAddress", () => {
  describe("when the value is a wallet in any case with surrounding spaces", () => {
    it("should return it trimmed and lowercased", () => {
      expect(asAddress(`  ${FIRST.toUpperCase().replace("0X", "0x")} `)).toBe(FIRST);
    });
  });

  describe("when the value is not a wallet", () => {
    it.each([[undefined], [null], [42], [""], ["0x123"], ["Decentraland Foundation"]])(
      "should return null for %j",
      (value) => {
        expect(asAddress(value)).toBeNull();
      },
    );
  });
});

describe("memberName", () => {
  describe("when the name is a wallet", () => {
    it("should be no name", () => {
      expect(memberName(FIRST)).toBe("");
    });
  });

  describe("when the name is text", () => {
    it("should keep it trimmed", () => {
      expect(memberName("  Ana ")).toBe("Ana");
    });
  });
});

describe("sameOriginContentUrl", () => {
  describe("when the URL is a content hash on another host", () => {
    it("should re-base it on the node", () => {
      expect(sameOriginContentUrl("https://catalyst.example/content/contents/bafyhash", BASE)).toBe(
        `${BASE}/content/contents/bafyhash`,
      );
    });
  });

  describe("when the URL is an upstream face snapshot", () => {
    it.each([
      "https://profile-images.decentraland.org/entities/bafyface/face.png",
      "https://profile-images.decentraland.org/v1/entities/bafyface/face.png",
      "https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/entities/bafyface/face.png",
      "https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafyface/face.png",
    ])("should read %s through the node's image cache", (source) => {
      expect(sameOriginContentUrl(source, BASE)).toBe(`${BASE}/profile-images/entities/bafyface/face.png`);
    });
  });

  describe("when the URL is anything else", () => {
    it("should leave it unchanged", () => {
      expect(sameOriginContentUrl("https://images.example/face.png", BASE)).toBe("https://images.example/face.png");
    });
  });
});

describe("fetchMemberProfiles", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllEnvs();
  });

  describe("when the node answers with profiles matched by ethAddress or userId", () => {
    let fetchImpl: ReturnType<typeof replyWith>;
    let profiles: Awaited<ReturnType<typeof fetchMemberProfiles>>;

    beforeEach(async () => {
      fetchImpl = replyWith([
        { avatars: [{ ethAddress: SECOND, name: "Second member", avatar: { snapshots: { face256: "second-face-hash" } } }] },
        {
          avatars: [
            {
              userId: FIRST.toUpperCase().replace("0X", "0x"),
              name: FIRST,
              hasClaimedName: true,
              avatar: { snapshots: { face256: "https://images.example/first.png" } },
            },
          ],
        },
      ]);
      profiles = await fetchMemberProfiles([FIRST, SECOND], { base: BASE, fetchImpl });
    });

    it("should post the wallets in one batch to the profiles lambda", () => {
      expect(fetchImpl).toHaveBeenCalledOnce();
      const [url, init] = fetchImpl.mock.calls[0] as unknown as [string, RequestInit];
      expect(url).toBe(`${BASE}/lambdas/profiles`);
      expect(JSON.parse(String(init.body))).toEqual({ ids: [FIRST, SECOND] });
    });

    it("should key each profile by its lowercased wallet, drop a name that is a wallet and resolve faces", () => {
      expect(profiles).toEqual({
        [FIRST]: { name: "", picture: "https://images.example/first.png", claimed: true },
        [SECOND]: { name: "Second member", picture: `${BASE}/content/contents/second-face-hash`, claimed: null },
      });
    });
  });

  describe("when a profile leaves its passport fields, links and snapshots null", () => {
    it("should keep its name and claim with no picture", async () => {
      const fetchImpl = replyWith([
        {
          avatars: [
            { ethAddress: FIRST, name: "MetaWolf", hasClaimedName: false, country: null, birthdate: null, links: null, avatar: { snapshots: null } },
          ],
        },
      ]);
      await expect(fetchMemberProfiles([FIRST], { base: BASE, fetchImpl })).resolves.toEqual({
        [FIRST]: { name: "MetaWolf", picture: null, claimed: false },
      });
    });
  });

  describe("when one envelope of the batch has an unreadable shape", () => {
    it("should skip only that envelope", async () => {
      const fetchImpl = replyWith([
        { avatars: [{ ethAddress: FIRST, name: "First member" }] },
        { avatars: [{ ethAddress: THIRD, name: 7 }] },
        { avatars: [{ ethAddress: SECOND, name: "Second member" }] },
      ]);
      const profiles = await fetchMemberProfiles([FIRST, SECOND, THIRD], { base: BASE, fetchImpl });
      expect(Object.keys(profiles).sort()).toEqual([FIRST, SECOND]);
    });
  });

  describe("when a profile names no wallet", () => {
    it("should leave it out", async () => {
      const fetchImpl = replyWith([{ avatars: [{ name: "Nobody" }, { ethAddress: "not-a-wallet", name: "Bad" }] }]);
      await expect(fetchMemberProfiles([FIRST], { base: BASE, fetchImpl })).resolves.toEqual({});
    });
  });

  describe("when the node answers with something that is not a list", () => {
    it("should return no profiles", async () => {
      const fetchImpl = replyWith({ error: "unexpected" });
      await expect(fetchMemberProfiles([FIRST], { base: BASE, fetchImpl })).resolves.toEqual({});
    });
  });

  describe("when there are more than 100 wallets", () => {
    it("should read them in batches of 100", async () => {
      const fetchImpl = replyWith([]);
      const wallets = Array.from({ length: 150 }, (_, i) => `0x${i.toString(16).padStart(40, "0")}`);
      await fetchMemberProfiles(wallets, { base: BASE, fetchImpl });
      const sizes = fetchImpl.mock.calls.map((call) => {
        const init = (call as unknown as [string, RequestInit])[1];
        return (JSON.parse(String(init.body)) as { ids: string[] }).ids.length;
      });
      expect(sizes).toEqual([100, 50]);
    });
  });

  describe("when there are no wallets", () => {
    it("should not call the node", async () => {
      const fetchImpl = replyWith([]);
      await expect(fetchMemberProfiles([], { base: BASE, fetchImpl })).resolves.toEqual({});
      expect(fetchImpl).not.toHaveBeenCalled();
    });
  });

  describe("when the node has no response body", () => {
    it.each([200, 204])("should return no profiles for HTTP %i", async (status) => {
      const fetchImpl = vi.fn(async () => new Response(null, { status }));
      await expect(fetchMemberProfiles([FIRST], { base: BASE, fetchImpl })).resolves.toEqual({});
    });
  });

  describe("when a build configures a Catalyst URL", () => {
    beforeEach(() => {
      vi.stubEnv("VITE_CATALYST_URL", "https://configured.example/");
    });

    it("should use the configured origin for the request and for faces", async () => {
      const fetchImpl = replyWith([{ avatars: [{ ethAddress: FIRST, avatar: { snapshots: { face256: "bafyface" } } }] }]);
      const profiles = await fetchMemberProfiles([FIRST], { fetchImpl });
      expect(fetchImpl).toHaveBeenCalledWith("https://configured.example/lambdas/profiles", expect.anything());
      expect(profiles[FIRST]?.picture).toBe("https://configured.example/content/contents/bafyface");
    });

    it("should keep an explicit origin ahead of the build configuration", async () => {
      const fetchImpl = replyWith([]);
      await fetchMemberProfiles([FIRST], { base: BASE, fetchImpl });
      expect(fetchImpl).toHaveBeenCalledWith(`${BASE}/lambdas/profiles`, expect.anything());
    });
  });

  describe("when the connection fails", () => {
    it("should preserve the request URL in a Catalyst error", async () => {
      const fetchImpl = vi.fn().mockRejectedValue(new TypeError("offline"));
      const profiles = fetchMemberProfiles([FIRST], { base: BASE, fetchImpl });
      await expect(profiles).rejects.toBeInstanceOf(CatalystError);
      await expect(profiles).rejects.toMatchObject({ status: 0, url: `${BASE}/lambdas/profiles` });
    });
  });

  describe("when the node fails", () => {
    it("should reject", async () => {
      const fetchImpl = replyWith("unavailable", { status: 503 });
      await expect(fetchMemberProfiles([FIRST], { base: BASE, fetchImpl })).rejects.toThrow();
    });
  });
});
