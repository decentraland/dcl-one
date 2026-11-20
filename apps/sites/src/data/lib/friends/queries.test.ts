import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

import type { AuthIdentity } from "../auth/types";
import { friendProfilesQuery, friendsKeys, invalidateFriendship } from "./queries";

const ME = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";
const OTHER = "0x1111111111111111111111111111111111111111";
const THIRD = "0x2222222222222222222222222222222222222222";

describe("friends keys", () => {
  it("sit under the member's private prefix, lowercased, so sign-out and an account switch remove them", () => {
    const prefix = ["private", ME.toLowerCase(), "friends"];
    expect(friendsKeys.all(ME)).toEqual(prefix);
    for (const key of [
      friendsKeys.friendship(ME, OTHER),
      friendsKeys.count(ME),
      friendsKeys.list(ME),
      friendsKeys.mutualCount(ME, OTHER),
      friendsKeys.blocking(ME),
    ]) {
      expect(key.slice(0, 3)).toEqual(prefix);
    }
  });

  it("keeps the batched friend summaries private and deduplicates their addresses", () => {
    const owner = { signer: ME } as AuthIdentity;
    const another = { signer: THIRD } as AuthIdentity;
    const addresses = [THIRD, OTHER, THIRD];

    expect(friendProfilesQuery(owner, addresses).queryKey).toEqual([
      ...friendsKeys.list(ME), "profiles", [OTHER, THIRD],
    ]);
    expect(friendProfilesQuery(another, addresses).queryKey).not.toEqual(friendProfilesQuery(owner, addresses).queryKey);
    expect(friendProfilesQuery(owner, []).enabled).toBe(false);
  });

  it("name the other member, so two profiles never share an entry", () => {
    expect(friendsKeys.friendship(ME, OTHER)).not.toEqual(friendsKeys.friendship(ME, THIRD));
    expect(friendsKeys.mutualCount(ME, OTHER)).not.toEqual(friendsKeys.mutualCount(ME, THIRD));
    expect(friendsKeys.friendship(ME, OTHER)).not.toEqual(friendsKeys.friendship(THIRD, OTHER));
  });
});

describe("invalidateFriendship", () => {
  it("marks every read a friendship or block write changes as stale, and no other member's", async () => {
    const client = new QueryClient();
    const changed = [
      friendsKeys.friendship(ME, OTHER),
      friendsKeys.count(ME),
      friendsKeys.list(ME),
      friendsKeys.mutualCount(ME, OTHER),
      friendsKeys.blocking(ME),
    ];
    const untouched = [friendsKeys.friendship(ME, THIRD), friendsKeys.count(THIRD)];
    for (const key of [...changed, ...untouched]) client.setQueryData(key, 1);

    await invalidateFriendship(client, ME, OTHER);

    for (const key of changed) expect(client.getQueryState(key)?.isInvalidated).toBe(true);
    for (const key of untouched) expect(client.getQueryState(key)?.isInvalidated).toBe(false);
  });
});
