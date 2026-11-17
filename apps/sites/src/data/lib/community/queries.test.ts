import { describe, expect, it } from "vitest";

import type { EventsPage } from "../catalyst/social/community-detail";
import { communityKeys, eventsQuery, membersQuery } from "./queries";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const page = (n: number, total: number, received = n): EventsPage => ({
  events: Array.from({ length: n }, (_, i) => ({ id: String(i) }) as never),
  total,
  received,
});

describe("community queries", () => {
  it("pages events until the total, and stops on an empty page", () => {
    const next = eventsQuery(ID).getNextPageParam!;
    expect(next(page(12, 30), [page(12, 30)], 0, [0])).toBe(12);
    expect(next(page(6, 18), [page(12, 18), page(6, 18)], 12, [0, 12])).toBeUndefined();
    expect(next(page(0, 50), [page(12, 50), page(0, 50)], 12, [0, 12])).toBeUndefined();
  });

  it("keeps paging past a page whose rows all failed validation", () => {
    const next = eventsQuery(ID).getNextPageParam!;
    // 12 rows arrived, none valid: the next 12 are still asked for.
    expect(next(page(0, 30, 12), [page(0, 30, 12)], 0, [0])).toBe(12);
    // The API sent nothing: paging ends even though the total says more.
    expect(next(page(0, 30, 0), [page(12, 30), page(0, 30, 0)], 12, [0, 12])).toBeUndefined();
  });

  it("keys a signed members read under the member's account", () => {
    expect(membersQuery(ID).queryKey).toEqual(communityKeys.members(ID));
    expect(membersQuery(ID, { signer: "0xABC" } as never).queryKey).toEqual(["private", "0xabc", "community", "members", ID]);
  });
});
