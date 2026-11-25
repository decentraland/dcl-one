import { beforeEach, describe, expect, it } from "vitest";

import type { EventsPage } from "../catalyst/social/community-detail";
import { communityKeys, eventsQuery, membersQuery } from "./queries";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const page = (n: number, total: number, received = n): EventsPage => ({
  events: Array.from({ length: n }, (_, i) => ({ id: String(i) }) as never),
  total,
  received,
});

describe("community queries", () => {
  describe("when paging events", () => {
    let next: NonNullable<ReturnType<typeof eventsQuery>["getNextPageParam"]>;

    beforeEach(() => {
      next = eventsQuery(ID).getNextPageParam!;
    });

    describe("and the total has not been reached", () => {
      it("should ask for the next page", () => {
        expect(next(page(12, 30), [page(12, 30)], 0, [0])).toBe(12);
      });
    });

    describe("and the total has been reached", () => {
      it("should stop paging", () => {
        expect(next(page(6, 18), [page(12, 18), page(6, 18)], 12, [0, 12])).toBeUndefined();
      });
    });

    describe("and the last page is empty", () => {
      it("should stop paging", () => {
        expect(next(page(0, 50), [page(12, 50), page(0, 50)], 12, [0, 12])).toBeUndefined();
      });
    });

    describe("and every row of the page failed validation", () => {
      it("should still ask for the next rows", () => {
        // 12 rows arrived, none valid: the next 12 are still asked for.
        expect(next(page(0, 30, 12), [page(0, 30, 12)], 0, [0])).toBe(12);
      });
    });

    describe("and the API sent nothing", () => {
      it("should stop paging even though the total says more", () => {
        expect(next(page(0, 30, 0), [page(12, 30), page(0, 30, 0)], 12, [0, 12])).toBeUndefined();
      });
    });
  });

  describe("when reading members", () => {
    describe("and the read is not signed", () => {
      it("should use the public members key", () => {
        expect(membersQuery(ID).queryKey).toEqual(communityKeys.members(ID));
      });
    });

    describe("and the read is signed", () => {
      it("should key it under the member's account", () => {
        expect(membersQuery(ID, { signer: "0xABC" } as never).queryKey).toEqual(["private", "0xabc", "community", "members", ID]);
      });
    });
  });
});
