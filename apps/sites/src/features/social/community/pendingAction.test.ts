// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { PENDING_ACTION_TTL_MS, rememberAction, takeAction } from "./pendingAction";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const NOW = 1_800_000_000_000;

afterEach(() => sessionStorage.clear());

describe("pending community action", () => {
  describe("when a marker is remembered", () => {
    let nonce: string | null;

    beforeEach(() => {
      nonce = rememberAction(ID, "join", sessionStorage, NOW);
    });

    describe("and it is taken with the matching nonce", () => {
      it("should accept the marker the first time", () => {
        expect(takeAction(ID, "join", nonce, sessionStorage, NOW + 1000)).toBe(true);
      });

      describe("and it is taken again", () => {
        beforeEach(() => {
          takeAction(ID, "join", nonce, sessionStorage, NOW + 1000);
        });

        it("should refuse the marker", () => {
          expect(takeAction(ID, "join", nonce, sessionStorage, NOW + 2000)).toBe(false);
        });
      });
    });

    describe("and it is taken without a nonce", () => {
      it("should refuse the marker", () => {
        expect(takeAction(ID, "join", null, sessionStorage, NOW)).toBe(false);
      });
    });

    describe("and it is taken with a different nonce", () => {
      it("should refuse the marker", () => {
        expect(takeAction(ID, "join", "other", sessionStorage, NOW)).toBe(false);
      });

      describe("and the marker was refused", () => {
        beforeEach(() => {
          takeAction(ID, "join", null, sessionStorage, NOW);
          takeAction(ID, "join", "other", sessionStorage, NOW);
        });

        it("should consume the marker", () => {
          expect(sessionStorage.length).toBe(0);
        });
      });
    });

    describe("and it is taken for a different action", () => {
      it("should refuse the marker", () => {
        expect(takeAction(ID, "requestToJoin", nonce, sessionStorage, NOW)).toBe(false);
      });
    });

    describe("and it is taken for a different community", () => {
      it("should refuse the marker", () => {
        expect(takeAction("another", "join", nonce, sessionStorage, NOW)).toBe(false);
      });
    });

    describe("and it is taken after its TTL", () => {
      it("should refuse the expired marker", () => {
        expect(takeAction(ID, "join", nonce, sessionStorage, NOW + PENDING_ACTION_TTL_MS + 1)).toBe(false);
      });
    });
  });

  describe("when a marker is remembered in the future", () => {
    let future: string | null;

    beforeEach(() => {
      future = rememberAction(ID, "join", sessionStorage, NOW + 60_000);
    });

    it("should refuse the marker", () => {
      expect(takeAction(ID, "join", future, sessionStorage, NOW)).toBe(false);
    });
  });

  describe("when the stored marker is corrupted", () => {
    beforeEach(() => {
      sessionStorage.setItem("dcl:community-action", "{not json");
    });

    it("should refuse the marker", () => {
      expect(takeAction(ID, "join", "n", sessionStorage, NOW)).toBe(false);
    });
  });

  describe("when there is no storage", () => {
    it("should record nothing", () => {
      expect(rememberAction(ID, "join", null, NOW)).toBeNull();
    });

    it("should accept nothing", () => {
      expect(takeAction(ID, "join", "n", null, NOW)).toBe(false);
    });
  });

  describe("when writing to storage throws", () => {
    let throwing: Storage;

    beforeEach(() => {
      throwing = {
        setItem: () => {
          throw new Error("QuotaExceededError");
        },
      } as unknown as Storage;
    });

    it("should record nothing", () => {
      expect(rememberAction(ID, "join", throwing, NOW)).toBeNull();
    });
  });
});
