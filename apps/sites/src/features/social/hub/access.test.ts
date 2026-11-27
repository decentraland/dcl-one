import { describe, expect, it } from "vitest";

import { CatalystError } from "@/data/lib/catalyst/client";
import type { HubCommunity } from "@/data/lib/catalyst/social/community-hub";

import { canManageMember, canReadPosts, failureText, manages } from "./access";

const viewer = "0x1111111111111111111111111111111111111111";
const member = (role: string, memberAddress = "0x2222222222222222222222222222222222222222") => ({
  memberAddress,
  role,
  name: null,
  profilePictureUrl: null,
  hasClaimedName: false,
});
const community = (privacy: "public" | "private", role: string | null): HubCommunity => ({
  id: "club",
  name: "Club",
  description: null,
  thumbnailUrl: null,
  visibility: null,
  privacy,
  isLive: false,
  membersCount: 1,
  role,
  ownerName: null,
});

describe("canManageMember", () => {
  it.each([
    ["owner", "member", true],
    ["owner", "moderator", true],
    ["owner", "mod", true],
    ["moderator", "member", true],
    ["mod", "member", true],
    ["owner", "owner", false],
    ["moderator", "owner", false],
    ["moderator", "moderator", false],
    ["mod", "mod", false],
    ["member", "member", false],
    ["admin", "member", false],
    ["owner", "admin", false],
    ["owner", "banned", false],
  ])("should let a %s manage a %s: %s", (role, target, expected) => {
    expect(canManageMember(role, viewer, member(target))).toBe(expected);
  });

  it("should never let the reader manage themselves, nor a visitor manage anyone", () => {
    expect(canManageMember("owner", viewer, member("member", viewer.toUpperCase().replace("0X", "0x")))).toBe(false);
    expect(canManageMember("owner", null, member("member"))).toBe(false);
  });
});

describe("manages", () => {
  it("should be owners and moderators only", () => {
    expect(["owner", "moderator", "mod", "member", "admin", null].map(manages)).toEqual([true, true, true, false, false, false]);
  });
});

describe("canReadPosts", () => {
  it("should open a public community to anyone and a private one to its members", () => {
    expect(canReadPosts(community("public", null))).toBe(true);
    expect(canReadPosts(community("private", null))).toBe(false);
    expect(canReadPosts(community("private", "member"))).toBe(true);
  });

  it("should close them to a banned reader and when nothing is open", () => {
    expect(canReadPosts(community("public", "banned"))).toBe(false);
    expect(canReadPosts(null)).toBe(false);
  });
});

describe("failureText", () => {
  it("should use the server's words, else the fallback", () => {
    expect(failureText(new CatalystError("Name taken", "/x", 409, true), "Try again.")).toBe("Name taken");
    expect(failureText(new CatalystError("Catalyst returned 500", "/x", 500), "Try again.")).toBe("Try again.");
  });

  it("should explain a refusal when one is given", () => {
    expect(failureText(new CatalystError("cannot kick", "/x", 401, true), "Try again.", "No permission.")).toBe("No permission.");
    expect(failureText(new CatalystError("cannot kick", "/x", 403, true), "Try again.")).toBe("cannot kick");
  });

  it("should fall back for an error with no message", () => {
    expect(failureText(new Error(""), "Try again.")).toBe("Try again.");
    expect(failureText("boom", "Try again.")).toBe("Try again.");
  });
});
