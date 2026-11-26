import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "../client";
import {
  CommunityNotFoundError,
  fetchCommunity,
  fetchCommunityEvents,
  fetchCommunityMembers,
  fetchJoinRequests,
  requestToJoin,
} from "./community-detail";

vi.mock("../../auth/signer", () => ({
  signedFetch: (_identity: unknown, url: string, init: RequestInit) => fetch(url, init),
}));

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
let identity: { signer: string; ephemeral: { address: string; privateKey: "0x00" }; expiration: string; authChain: never[] };
let body: unknown;
let status: number;

beforeEach(() => {
  identity = { signer: "0xABC", ephemeral: { address: "0x0", privateKey: "0x00" as const }, expiration: "", authChain: [] };
  status = 200;
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(body), { status })));
});
afterEach(() => {
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

const row = (memberAddress: string, extra: Record<string, unknown> = {}) => ({ memberAddress, role: "member", ...extra });

describe("fetchCommunity", () => {
  describe("when the id is refused (400)", () => {
    beforeEach(() => {
      status = 400;
      body = { error: "Bad request" };
    });

    it("should treat it as not found", async () => {
      await expect(fetchCommunity(ID)).rejects.toBeInstanceOf(CommunityNotFoundError);
    });

    it("should name the community in the error", async () => {
      await expect(fetchCommunity(ID)).rejects.toThrow(`Community not found: ${ID}`);
    });
  });

  describe("when the read fails otherwise", () => {
    beforeEach(() => {
      status = 500;
      body = {};
    });

    it("should keep the failure as unavailable", async () => {
      await expect(fetchCommunity(ID)).rejects.toBeInstanceOf(CatalystError);
    });

    it("should report the status", async () => {
      await expect(fetchCommunity(ID)).rejects.toThrow("500");
    });
  });

  describe("when the read is signed", () => {
    beforeEach(() => {
      body = { data: { id: ID, name: "N", ownerAddress: "0x1", privacy: "private", membersCount: 3, role: "moderator" } };
    });

    it("should read the member's role", async () => {
      expect((await fetchCommunity(ID, { identity })).role).toBe("moderator");
    });
  });

  describe("when the role is unknown", () => {
    beforeEach(() => {
      body = { data: { id: ID, name: "N", ownerAddress: "0x1", privacy: "public", membersCount: 3, role: "superuser" } };
    });

    it("should drop the role instead of failing", async () => {
      expect((await fetchCommunity(ID)).role).toBeNull();
    });
  });
});

describe("fetchCommunityMembers", () => {
  describe("when some rows are malformed", () => {
    let page: Awaited<ReturnType<typeof fetchCommunityMembers>>;

    beforeEach(async () => {
      body = { data: { results: [row("0x1"), { role: "member" }, row("0x2", { name: "Bob" })], total: 3, page: 1, pages: 1 } };
      page = await fetchCommunityMembers(ID, 0);
    });

    it("should keep the valid rows and drop the malformed ones", () => {
      expect(page.members.map((m) => m.memberAddress)).toEqual(["0x1", "0x2"]);
    });

    it("should report no more pages", () => {
      expect(page.hasMore).toBe(false);
    });
  });

  describe("when page and pages are given", () => {
    beforeEach(() => {
      body = { data: { results: [row("0x1")], total: 30, page: 1, pages: 3 } };
    });

    it("should use them to report more pages", async () => {
      expect((await fetchCommunityMembers(ID, 0)).hasMore).toBe(true);
    });
  });

  describe("when page and pages are missing", () => {
    beforeEach(() => {
      body = { data: { results: [row("0x1")], total: 15 } };
    });

    describe("and the offset is below the total", () => {
      it("should fall back to the total and report more pages", async () => {
        expect((await fetchCommunityMembers(ID, 0)).hasMore).toBe(true);
      });
    });

    describe("and the offset reaches the total", () => {
      it("should fall back to the total and report no more pages", async () => {
        expect((await fetchCommunityMembers(ID, 10)).hasMore).toBe(false);
      });
    });
  });

  describe("when the envelope cannot be read", () => {
    beforeEach(() => {
      body = { results: [] };
    });

    it("should fail as unavailable", async () => {
      await expect(fetchCommunityMembers(ID, 0)).rejects.toBeInstanceOf(CatalystError);
    });

    it("should report the validation failure", async () => {
      await expect(fetchCommunityMembers(ID, 0)).rejects.toThrow("Community members failed validation");
    });
  });
});

describe("member requests", () => {
  describe("when listing requests of several types and states", () => {
    beforeEach(() => {
      body = {
        data: {
          results: [
            { id: "a", communityId: ID, type: "request_to_join", status: "pending" },
            { id: "b", communityId: ID, type: "invite", status: "pending" },
            { id: "c", communityId: ID, type: "request_to_join", status: "cancelled" },
            { id: "d" },
          ],
        },
      };
    });

    it("should keep only pending requests to join", async () => {
      expect((await fetchJoinRequests(identity)).map((r) => r.id)).toEqual(["a"]);
    });
  });

  describe("when a new request's reply does not parse", () => {
    beforeEach(() => {
      body = { data: { id: "a" } };
    });

    it("should throw as unavailable", async () => {
      await expect(requestToJoin(identity, ID)).rejects.toBeInstanceOf(CatalystError);
    });

    it("should report the validation failure", async () => {
      await expect(requestToJoin(identity, ID)).rejects.toThrow("Community request failed validation");
    });
  });

  describe("when a new request is sent", () => {
    beforeEach(async () => {
      body = { data: { id: "a", communityId: ID, type: "request_to_join", status: "pending" } };
      await requestToJoin(identity, ID);
    });

    it("should send the member's own address", () => {
      const init = vi.mocked(fetch).mock.calls[0]![1] as RequestInit;
      expect(JSON.parse(String(init.body))).toEqual({ targetedAddress: "0xabc", type: "request_to_join" });
    });
  });
});

describe("fetchCommunityEvents", () => {
  describe("when asking for the community's events", () => {
    let url: URL;

    beforeEach(async () => {
      body = { ok: true, data: { events: [], total: 0 } };
      await fetchCommunityEvents(ID, 12);
      url = new URL(String(vi.mocked(fetch).mock.calls[0]![0]));
    });

    it("should filter by the community", () => {
      expect(url.searchParams.get("community_id")).toBe(ID);
    });

    it("should pass the offset", () => {
      expect(url.searchParams.get("offset")).toBe("12");
    });

    it("should send no list filter, as the source does (upcoming and in progress)", () => {
      expect(url.searchParams.has("list")).toBe(false);
    });
  });
});
