import { describe, expect, it } from "vitest";

import type { EventsPage } from "../catalyst/social/community-detail";
import { communityKeys, eventsQuery, membersQuery } from "./queries";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const page = (n: number, total: number): EventsPage => ({ events: Array.from({ length: n }, (_, i) => ({ id: String(i) }) as never), total });

describe("community queries", () => {
  it("pages events until the total, and stops on an empty page", () => {
    const next = eventsQuery(ID).getNextPageParam!;
    expect(next(page(12, 30), [page(12, 30)], 0, [0])).toBe(12);
    expect(next(page(6, 18), [page(12, 18), page(6, 18)], 12, [0, 12])).toBeUndefined();
    expect(next(page(0, 50), [page(12, 50), page(0, 50)], 12, [0, 12])).toBeUndefined();
  });

  it("keys a signed members read under the member's account", () => {
    expect(membersQuery(ID).queryKey).toEqual(communityKeys.members(ID));
    expect(membersQuery(ID, { signer: "0xABC" } as never).queryKey).toEqual(["private", "0xabc", "community", "members", ID]);
  });
});
