import { describe, expect, it, vi } from "vitest";

import { canViewContent, faceUrl, membersLabel, membershipState, readAction, roleLabel, signInReturnHref, thumbnailUrl } from "./view";

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

describe("membershipState after a failed read", () => {
  it("offers a retry when the member's read of the community fails", () => {
    expect(membershipState({ ...base, signedIn: true, viewer: undefined, privacy: "public", viewerFailed: true })).toEqual({ kind: "error" });
  });

  it("still shows a member as joined when only the requests read failed", () => {
    expect(
      membershipState({ ...base, signedIn: true, viewer: { role: "member", privacy: "private" }, privacy: "private", requests: undefined, requestsFailed: true }),
    ).toEqual({ kind: "joined" });
  });

  it("offers a retry when a non-member's requests read failed", () => {
    expect(
      membershipState({ ...base, signedIn: true, viewer: { role: "none", privacy: "private" }, privacy: "private", requests: undefined, requestsFailed: true }),
    ).toEqual({ kind: "error" });
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

  it("returns the visitor to the community with the chosen action and its nonce", () => {
    expect(signInReturnHref(ID, "join", "n1")).toBe(`/social/communities/${ID}?action=join&nonce=n1`);
  });

  it("returns the visitor to the plain page when no marker could be recorded", () => {
    expect(signInReturnHref(ID, "join", null)).toBe(`/social/communities/${ID}`);
  });

  it("formats counts and roles as the source does", () => {
    expect(membersLabel(1340)).toBe("1.3K Members");
    expect(membersLabel(0)).toBe("0 Members");
    expect(roleLabel("moderator")).toBe("Moderator");
    expect(roleLabel("")).toBe("");
  });
});

describe("thumbnailUrl", () => {
  const own = `/assets-cdn/social/communities/${ID}/raw-thumbnail.png`;
  const cdn = `/social/communities/${ID}/raw-thumbnail.png`;

  it("falls back to the node's own image when the API names none or a malformed one", () => {
    expect(thumbnailUrl({ id: ID, thumbnailUrl: null })).toBe(own);
    expect(thumbnailUrl({ id: ID, thumbnailUrl: "N/A" })).toBe(own);
    expect(thumbnailUrl({ id: ID, thumbnailUrl: "not a url" })).toBe(own);
    expect(thumbnailUrl({ id: ID, thumbnailUrl: "javascript:alert(1)" })).toBe(own);
  });

  it("reads an https communities CDN image through the site's asset alias", () => {
    expect(thumbnailUrl({ id: ID, thumbnailUrl: `https://assets-cdn.decentraland.org${cdn}` })).toBe(own);
    expect(thumbnailUrl({ id: ID, thumbnailUrl: `https://cdn.decentraland.org${cdn}` })).toBe(own);
  });

  it("rejects http, unknown hosts and other paths", () => {
    const other = `/assets-cdn/social/communities/${ID}/raw-thumbnail.png`;
    expect(thumbnailUrl({ id: ID, thumbnailUrl: `http://assets-cdn.decentraland.org${cdn}` })).toBe(other);
    expect(thumbnailUrl({ id: ID, thumbnailUrl: `https://tracker.example.org${cdn}` })).toBe(other);
    expect(thumbnailUrl({ id: ID, thumbnailUrl: "https://assets-cdn.decentraland.org/anything.png" })).toBe(other);
  });
});

describe("faceUrl", () => {
  const entity = "bafkreifkus6iegxyerri7rbfzmxrj5c726prza7lqobufstyc54yuzxwpy";

  it("keeps a profile image as a path on this site", () => {
    expect(faceUrl(`https://profile-images.decentraland.org/entities/${entity}/face.png`)).toBe(`/profile-images/entities/${entity}/face.png`);
    expect(faceUrl("bafyhash")).toBe("/content/contents/bafyhash");
  });

  it("accepts this site's own absolute https face URL as its path", () => {
    expect(faceUrl(`https://decentraland.org/profile-images/entities/${entity}/face.png`)).toBe(`/profile-images/entities/${entity}/face.png`);
  });

  it("accepts the face a profile read prefixed with the node's own origin, even on http", () => {
    // The creator profile read prefixes faces with the node's origin, which is http on a local node.
    vi.stubEnv("CATALYST_URL", "http://localhost:5414");
    try {
      expect(faceUrl(`http://localhost:5414/profile-images/entities/${entity}/face.png`)).toBe(`/profile-images/entities/${entity}/face.png`);
      expect(faceUrl("http://localhost:5414/elsewhere/face.png")).toBeUndefined();
      expect(faceUrl(`http://localhost:5415/profile-images/entities/${entity}/face.png`)).toBeUndefined();
    } finally {
      vi.unstubAllEnvs();
    }
  });

  it("resolves dot segments before checking the path, so none climbs out of the face paths", () => {
    expect(faceUrl("/content/contents/../../x")).toBeUndefined();
    expect(faceUrl("../../x")).toBeUndefined();
    expect(faceUrl(`/profile-images/entities/../../api/x`)).toBeUndefined();
    expect(faceUrl("/content/contents/%2e%2e/%2e%2e/x")).toBeUndefined();
    expect(faceUrl(`https://decentraland.org/profile-images/entities/../../api/x`)).toBeUndefined();
  });

  it("rejects http, other hosts' images, data URLs and malformed values", () => {
    expect(faceUrl(`http://profile-images.example.org/entities/${entity}/face.png`)).toBeUndefined();
    expect(faceUrl("https://tracker.example.org/pixel.png")).toBeUndefined();
    expect(faceUrl("data:image/png;base64,AAAA")).toBeUndefined();
    expect(faceUrl("//tracker.example.org/x.png")).toBeUndefined();
    expect(faceUrl("")).toBeUndefined();
    expect(faceUrl(null)).toBeUndefined();
  });
});
