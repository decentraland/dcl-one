import { describe, expect, it } from "vitest";

import { canViewContent, membersLabel, membershipState, readAction, roleLabel, signInReturnHref } from "./view";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const base = { communityId: ID, requests: [] as { id: string; communityId: string; type: string; status: string }[] };

describe("membershipState", () => {
  it("asks a signed-out visitor to sign in", () => {
    expect(membershipState({ ...base, signedIn: false, viewer: undefined, privacy: "public" })).toEqual({ kind: "signed-out" });
  });

  it("waits for the member's own read before offering anything", () => {
    expect(membershipState({ ...base, signedIn: true, viewer: undefined, privacy: "public" })).toEqual({ kind: "loading" });
  });

  it("shows a member as joined, whatever the privacy", () => {
    for (const role of ["owner", "moderator", "member"] as const) {
      expect(membershipState({ ...base, signedIn: true, viewer: { role, privacy: "private" }, privacy: "private" })).toEqual({ kind: "joined" });
    }
  });

  it("offers Join on a public community", () => {
    expect(membershipState({ ...base, signedIn: true, viewer: { role: "none", privacy: "public" }, privacy: "public" })).toEqual({ kind: "join" });
    expect(membershipState({ ...base, signedIn: true, viewer: { role: null, privacy: "public" }, privacy: "public" })).toEqual({ kind: "join" });
  });

  it("waits for the member's requests on a private community", () => {
    expect(
      membershipState({ ...base, signedIn: true, viewer: { role: "none", privacy: "private" }, privacy: "private", requests: undefined }),
    ).toEqual({ kind: "loading" });
  });

  it("offers a request, or its cancellation when one is pending for this community", () => {
    const viewer = { role: "none" as const, privacy: "private" as const };
    expect(membershipState({ ...base, signedIn: true, viewer, privacy: "private" })).toEqual({ kind: "request" });
    const other = { id: "r0", communityId: "another", type: "request_to_join", status: "pending" };
    expect(membershipState({ ...base, signedIn: true, viewer, privacy: "private", requests: [other] })).toEqual({ kind: "request" });
    const mine = { id: "r1", communityId: ID, type: "request_to_join", status: "pending" };
    expect(membershipState({ ...base, signedIn: true, viewer, privacy: "private", requests: [mine] })).toEqual({ kind: "cancel", requestId: "r1" });
  });
});

describe("canViewContent", () => {
  it("shows a public community to anyone and a private one to members only", () => {
    expect(canViewContent({ privacy: "public", role: null })).toBe(true);
    expect(canViewContent({ privacy: "private", role: null })).toBe(false);
    expect(canViewContent({ privacy: "private", role: "none" })).toBe(false);
    expect(canViewContent({ privacy: "private", role: "member" })).toBe(true);
  });
});

describe("helpers", () => {
  it("reads only the actions a sign-in return can carry", () => {
    expect(readAction("join")).toBe("join");
    expect(readAction("requestToJoin")).toBe("requestToJoin");
    expect(readAction("leave")).toBeNull();
    expect(readAction("")).toBeNull();
    expect(readAction(null)).toBeNull();
  });

  it("returns the visitor to the community with the chosen action", () => {
    expect(signInReturnHref(ID, "join")).toBe(`/social/communities/${ID}?action=join`);
  });

  it("formats counts and roles as the source does", () => {
    expect(membersLabel(1340)).toBe("1.3K Members");
    expect(membersLabel(0)).toBe("0 Members");
    expect(roleLabel("moderator")).toBe("Moderator");
    expect(roleLabel("")).toBe("");
  });
});
