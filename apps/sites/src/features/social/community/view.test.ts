import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { canViewContent, faceUrl, membersLabel, membershipState, readAction, roleLabel, signInReturnHref, thumbnailUrl } from "./view";

type MembershipInput = Parameters<typeof membershipState>[0];

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";

describe("membershipState", () => {
  let input: MembershipInput;

  beforeEach(() => {
    input = { communityId: ID, requests: [], signedIn: true, viewer: undefined, privacy: "public" };
  });

  describe("when the visitor is signed out", () => {
    beforeEach(() => {
      input = { ...input, signedIn: false };
    });

    it("should ask the visitor to sign in", () => {
      expect(membershipState(input)).toEqual({ kind: "signed-out" });
    });
  });

  describe("when the member's own read of the community is still running", () => {
    it("should wait before offering anything", () => {
      expect(membershipState(input)).toEqual({ kind: "loading" });
    });
  });

  describe("when the viewer is a member of a private community", () => {
    it.each(["owner", "moderator", "member"] as const)("should show the %s as joined", (role) => {
      expect(membershipState({ ...input, viewer: { role, privacy: "private" }, privacy: "private" })).toEqual({ kind: "joined" });
    });
  });

  describe("when the community is public", () => {
    describe("and the viewer's role is none", () => {
      beforeEach(() => {
        input = { ...input, viewer: { role: "none", privacy: "public" } };
      });

      it("should offer Join", () => {
        expect(membershipState(input)).toEqual({ kind: "join" });
      });
    });

    describe("and the viewer has no role", () => {
      beforeEach(() => {
        input = { ...input, viewer: { role: null, privacy: "public" } };
      });

      it("should offer Join", () => {
        expect(membershipState(input)).toEqual({ kind: "join" });
      });
    });
  });

  describe("when the community is private and the viewer is not a member", () => {
    beforeEach(() => {
      input = { ...input, viewer: { role: "none", privacy: "private" }, privacy: "private" };
    });

    describe("and the member's requests are still loading", () => {
      beforeEach(() => {
        input = { ...input, requests: undefined };
      });

      it("should wait for the requests", () => {
        expect(membershipState(input)).toEqual({ kind: "loading" });
      });
    });

    describe("and there are no requests", () => {
      it("should offer a request", () => {
        expect(membershipState(input)).toEqual({ kind: "request" });
      });
    });

    describe("and the only pending request is for another community", () => {
      beforeEach(() => {
        input = { ...input, requests: [{ id: "r0", communityId: "another", type: "request_to_join", status: "pending" }] };
      });

      it("should offer a request", () => {
        expect(membershipState(input)).toEqual({ kind: "request" });
      });
    });

    describe("and a request is pending for this community", () => {
      beforeEach(() => {
        input = { ...input, requests: [{ id: "r1", communityId: ID, type: "request_to_join", status: "pending" }] };
      });

      it("should offer the cancellation of that request", () => {
        expect(membershipState(input)).toEqual({ kind: "cancel", requestId: "r1" });
      });
    });
  });

  describe("when the member's read of the community failed", () => {
    beforeEach(() => {
      input = { ...input, viewerFailed: true };
    });

    it("should offer a retry", () => {
      expect(membershipState(input)).toEqual({ kind: "error" });
    });
  });

  describe("when only the requests read failed on a private community", () => {
    beforeEach(() => {
      input = { ...input, privacy: "private", requests: undefined, requestsFailed: true };
    });

    describe("and the viewer is a member", () => {
      beforeEach(() => {
        input = { ...input, viewer: { role: "member", privacy: "private" } };
      });

      it("should still show the member as joined", () => {
        expect(membershipState(input)).toEqual({ kind: "joined" });
      });
    });

    describe("and the viewer is not a member", () => {
      beforeEach(() => {
        input = { ...input, viewer: { role: "none", privacy: "private" } };
      });

      it("should offer a retry", () => {
        expect(membershipState(input)).toEqual({ kind: "error" });
      });
    });
  });
});

describe("canViewContent", () => {
  describe("when the community is public", () => {
    it("should show it to anyone", () => {
      expect(canViewContent({ privacy: "public", role: null })).toBe(true);
    });
  });

  describe("when the community is private", () => {
    it("should hide it from a visitor without a role", () => {
      expect(canViewContent({ privacy: "private", role: null })).toBe(false);
    });

    it("should hide it from a non-member", () => {
      expect(canViewContent({ privacy: "private", role: "none" })).toBe(false);
    });

    it("should show it to a member", () => {
      expect(canViewContent({ privacy: "private", role: "member" })).toBe(true);
    });
  });
});

describe("readAction", () => {
  it("should read only the actions a sign-in return can carry", () => {
    expect(readAction("join")).toBe("join");
    expect(readAction("requestToJoin")).toBe("requestToJoin");
    expect(readAction("leave")).toBeNull();
    expect(readAction("")).toBeNull();
    expect(readAction(null)).toBeNull();
  });
});

describe("signInReturnHref", () => {
  describe("when a marker nonce was recorded", () => {
    it("should return the visitor to the community with the chosen action and its nonce", () => {
      expect(signInReturnHref(ID, "join", "n1")).toBe(`/social/communities/${ID}?action=join&nonce=n1`);
    });
  });

  describe("when no marker could be recorded", () => {
    it("should return the visitor to the plain page", () => {
      expect(signInReturnHref(ID, "join", null)).toBe(`/social/communities/${ID}`);
    });
  });
});

describe("membersLabel", () => {
  it("should format counts as the source does", () => {
    expect(membersLabel(1340)).toBe("1.3K Members");
    expect(membersLabel(0)).toBe("0 Members");
  });
});

describe("roleLabel", () => {
  it("should format roles as the source does", () => {
    expect(roleLabel("moderator")).toBe("Moderator");
    expect(roleLabel("")).toBe("");
  });
});

describe("thumbnailUrl", () => {
  const own = `/assets-cdn/social/communities/${ID}/raw-thumbnail.png`;
  const cdn = `/social/communities/${ID}/raw-thumbnail.png`;

  describe("when the API names no image or a malformed one", () => {
    it("should fall back to the node's own image", () => {
      expect(thumbnailUrl({ id: ID, thumbnailUrl: null })).toBe(own);
      expect(thumbnailUrl({ id: ID, thumbnailUrl: "N/A" })).toBe(own);
      expect(thumbnailUrl({ id: ID, thumbnailUrl: "not a url" })).toBe(own);
      expect(thumbnailUrl({ id: ID, thumbnailUrl: "javascript:alert(1)" })).toBe(own);
    });
  });

  describe("when the API names an https communities CDN image", () => {
    it("should read it through the site's asset alias", () => {
      expect(thumbnailUrl({ id: ID, thumbnailUrl: `https://assets-cdn.decentraland.org${cdn}` })).toBe(own);
      expect(thumbnailUrl({ id: ID, thumbnailUrl: `https://cdn.decentraland.org${cdn}` })).toBe(own);
    });
  });

  describe("when the API names an http, unknown-host or other-path image", () => {
    it("should reject it and use the node's own image", () => {
      const other = `/assets-cdn/social/communities/${ID}/raw-thumbnail.png`;
      expect(thumbnailUrl({ id: ID, thumbnailUrl: `http://assets-cdn.decentraland.org${cdn}` })).toBe(other);
      expect(thumbnailUrl({ id: ID, thumbnailUrl: `https://tracker.example.org${cdn}` })).toBe(other);
      expect(thumbnailUrl({ id: ID, thumbnailUrl: "https://assets-cdn.decentraland.org/anything.png" })).toBe(other);
    });
  });
});

describe("faceUrl", () => {
  const entity = "bafkreifkus6iegxyerri7rbfzmxrj5c726prza7lqobufstyc54yuzxwpy";

  describe("when the value is a profile image URL or a content hash", () => {
    it("should keep it as a path on this site", () => {
      expect(faceUrl(`https://profile-images.decentraland.org/entities/${entity}/face.png`)).toBe(`/profile-images/entities/${entity}/face.png`);
      expect(faceUrl("bafyhash")).toBe("/content/contents/bafyhash");
    });
  });

  describe("when the value is this site's own absolute https face URL", () => {
    it("should accept it as its path", () => {
      expect(faceUrl(`https://decentraland.org/profile-images/entities/${entity}/face.png`)).toBe(`/profile-images/entities/${entity}/face.png`);
    });
  });

  describe("when the node's own origin is http", () => {
    // The creator profile read prefixes faces with the node's origin, which is http on a local node.
    beforeEach(() => {
      vi.stubEnv("CATALYST_URL", "http://localhost:5414");
    });

    afterEach(() => {
      vi.unstubAllEnvs();
    });

    describe("and the face is prefixed with that origin", () => {
      it("should accept it as its path", () => {
        expect(faceUrl(`http://localhost:5414/profile-images/entities/${entity}/face.png`)).toBe(`/profile-images/entities/${entity}/face.png`);
      });
    });

    describe("and the URL on that origin is outside the face paths", () => {
      it("should reject it", () => {
        expect(faceUrl("http://localhost:5414/elsewhere/face.png")).toBeUndefined();
      });
    });

    describe("and the face is on another port", () => {
      it("should reject it", () => {
        expect(faceUrl(`http://localhost:5415/profile-images/entities/${entity}/face.png`)).toBeUndefined();
      });
    });
  });

  describe("when the path has dot segments", () => {
    it("should resolve them before checking the path, so none climbs out of the face paths", () => {
      expect(faceUrl("/content/contents/../../x")).toBeUndefined();
      expect(faceUrl("../../x")).toBeUndefined();
      expect(faceUrl(`/profile-images/entities/../../api/x`)).toBeUndefined();
      expect(faceUrl("/content/contents/%2e%2e/%2e%2e/x")).toBeUndefined();
      expect(faceUrl(`https://decentraland.org/profile-images/entities/../../api/x`)).toBeUndefined();
    });
  });

  describe("when the value is http, another host's image, a data URL or malformed", () => {
    it("should reject it", () => {
      expect(faceUrl(`http://profile-images.example.org/entities/${entity}/face.png`)).toBeUndefined();
      expect(faceUrl("https://tracker.example.org/pixel.png")).toBeUndefined();
      expect(faceUrl("data:image/png;base64,AAAA")).toBeUndefined();
      expect(faceUrl("//tracker.example.org/x.png")).toBeUndefined();
      expect(faceUrl("")).toBeUndefined();
      expect(faceUrl(null)).toBeUndefined();
    });
  });
});
