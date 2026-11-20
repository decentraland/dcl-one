import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEventCategories } from "@/data/lib/catalyst/places/events";
import { loader as editLoader } from "./events_.edit-event.$id";
import { loader as newLoader } from "./events_.new-event";

vi.mock("@/data/lib/catalyst/places/events", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/events")>()),
  fetchEventCategories: vi.fn(),
}));

const EVENT_ID = "5f0c6a52-6b1d-4c1e-9a77-0e8f5b2c4d10";

const args = (path: string, params: Record<string, string> = {}) =>
  ({ request: new Request(`https://sites.test${path}`), params, context: new RouterContextProvider() }) as never;

type EditorData = Awaited<ReturnType<typeof newLoader>>["data"];

describe("when the hangout editor loads", () => {
  let data: EditorData;

  beforeEach(() => {
    vi.mocked(fetchEventCategories).mockResolvedValue([]);
  });
  afterEach(() => vi.resetAllMocks());

  describe("and the visitor opens /events/new-event", () => {
    describe("and the query names a community", () => {
      beforeEach(async () => {
        ({ data } = await newLoader(args("/events/new-event?community_id=c-1")));
      });

      it("should open the editor in create mode with the community from the query", () => {
        expect(data).toMatchObject({ mode: "create", eventId: null, communityId: "c-1" });
      });
    });

    describe("and the query carries an id", () => {
      beforeEach(async () => {
        ({ data } = await newLoader(args("/events/new-event?mode=edit&id=ev-1")));
      });

      it("should ignore the id and open in create mode", () => {
        expect(data).toMatchObject({ mode: "create", eventId: null });
      });
    });
  });

  describe("and the visitor opens /events/edit-event/:id", () => {
    describe("and the path id names an event", () => {
      beforeEach(async () => {
        ({ data } = await editLoader(args(`/events/edit-event/${EVENT_ID}?community_id=c-1`, { id: EVENT_ID })));
      });

      it("should open the editor in edit mode for the event in the path", () => {
        expect(data).toMatchObject({ mode: "edit", eventId: EVENT_ID, communityId: "" });
      });
    });

    describe("and the path id is not a UUID", () => {
      beforeEach(async () => {
        ({ data } = await editLoader(args("/events/edit-event/not-an-id", { id: "not-an-id" })));
      });

      it("should treat it as no event", () => {
        expect(data).toMatchObject({ mode: "edit", eventId: null });
      });
    });

    describe("and the path id is blank", () => {
      beforeEach(async () => {
        ({ data } = await editLoader(args("/events/edit-event/%20", { id: " " })));
      });

      it("should ask for an event", () => {
        expect(data).toMatchObject({ mode: "edit", eventId: null });
      });
    });
  });

  describe("and a category has multiple translations", () => {
    beforeEach(async () => {
      vi.mocked(fetchEventCategories).mockResolvedValueOnce([
        { name: "music", active: true, i18n: { en: "Music", es: "Música" } },
      ]);
      ({ data } = await newLoader(args("/events/new-event")));
    });

    it("should retain the translations for the locale selected on the client", () => {
      expect(data.categories).toEqual([
        { name: "music", label: "Music", i18n: { en: "Music", es: "Música" } },
      ]);
    });
  });

  describe("and the categories cannot be read", () => {
    beforeEach(async () => {
      vi.mocked(fetchEventCategories).mockRejectedValueOnce(new Error("down"));
      ({ data } = await newLoader(args("/events/new-event")));
    });

    it("should still open the editor and flag the categories", () => {
      expect(data).toMatchObject({ mode: "create", categories: [], categoriesError: true });
    });
  });
});
