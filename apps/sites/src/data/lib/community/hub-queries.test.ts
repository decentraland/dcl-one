import { describe, expect, it } from "vitest";

import type { AuthIdentity } from "../auth/types";

import { hubCommunitiesQuery, hubDetailQuery, hubKeys, hubLatestPostQuery, ownRequestsQuery } from "./hub-queries";

const member = { signer: "0xAbC" } as AuthIdentity;

describe("hubKeys", () => {
  it("should keep a member's reads under their account, lower case", () => {
    expect(hubDetailQuery(member, "club").queryKey).toEqual(["private", "0xabc", "community", "hub", "detail", "club"]);
  });

  it("should keep a visitor's reads under no account", () => {
    expect(hubDetailQuery(null, "club").queryKey).toEqual(["community", "hub", "detail", "club"]);
  });

  it("should give the search its own entry under the list", () => {
    const list = hubCommunitiesQuery(member).queryKey;
    const search = hubCommunitiesQuery(member, "hiking").queryKey;
    expect(search).not.toEqual(list);
    expect(search.slice(0, list.length)).toEqual(list);
    expect(hubCommunitiesQuery(member, "music").queryKey).not.toEqual(search);
  });

  it("should put the latest announcement under the community's posts", () => {
    expect(hubKeys.latestPost("0xabc", "club").slice(0, hubKeys.posts("0xabc", "club").length)).toEqual(
      hubKeys.posts("0xabc", "club"),
    );
  });
});

describe("the hub's disabled reads", () => {
  it("should not read requests for a visitor", () => {
    expect(typeof ownRequestsQuery(null).queryFn).not.toBe("function");
    expect(typeof ownRequestsQuery(member).queryFn).toBe("function");
  });

  it("should not read announcements the reader may not see", () => {
    expect(typeof hubLatestPostQuery(member, "club", false).queryFn).not.toBe("function");
    expect(typeof hubLatestPostQuery(member, "club", true).queryFn).toBe("function");
  });
});
