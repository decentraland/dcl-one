import { describe, expect, it } from "vitest";

import { eventToDraft, toCreateEventBody, type EditableHangout } from "./submit-hangout";

const OWNER = "0x1111111111111111111111111111111111111111";
const EDITOR = "0x2222222222222222222222222222222222222222";

function hangout(overrides: Partial<EditableHangout> = {}): EditableHangout {
  return {
    id: "5f0c6a52-6b1d-4c1e-9a77-0e8f5b2c4d10", name: "Pool party", image: "https://example.com/cover.png", image_vertical: null,
    description: "Bring a towel", start_at: "2099-06-01T18:00:00Z", finish_at: "2099-06-01T20:00:00Z", next_start_at: null, next_finish_at: null,
    duration: 7_200_000, all_day: false, x: 10, y: 20, server: null, url: null, user: OWNER, user_name: null,
    estate_id: null, estate_name: null, scene_name: null, approved: false, rejected: false, highlighted: false, trending: false, world: false,
    recurrent: false, recurrent_frequency: null, recurrent_weekday_mask: 0, recurrent_month_mask: 0, recurrent_interval: 1,
    recurrent_setpos: null, recurrent_monthday: null, recurrent_count: null, recurrent_until: null, recurrent_dates: [], categories: [], schedules: [],
    total_attendees: 0, latest_attendees: [], coordinates: [10, 20], position: [10, 20], live: false, attending: false, place_id: null, community_id: null,
    featured_item: null, created_at: null, updated_at: null, approved_by: null, rejected_by: null, rejection_reason: null,
    deleted_by_user: false, deleted_by_admin: false, deleted_by: null, deleted_at: null, deleted_reason: null, previous_place_id: null,
    ...overrides,
  } as EditableHangout;
}

describe("when an edited event is saved", () => {
  describe("and it had been rejected", () => {
    const original = hangout({ rejected: true, rejection_reason: "Invalid image" });

    describe("and its owner saves it", () => {
      it("should ask to send it back to review, as decentraland.org's editor does", () => {
        expect(toCreateEventBody(eventToDraft(original), original, OWNER)).toMatchObject({ rejected: false });
      });
    });

    describe("and someone who may edit any event saves it", () => {
      it("should leave its review state out of the change", () => {
        expect(toCreateEventBody(eventToDraft(original), original, EDITOR, true)).not.toHaveProperty("rejected");
      });
    });

    describe("and the signer is unknown", () => {
      it("should leave its review state out of the change", () => {
        expect(toCreateEventBody(eventToDraft(original), original)).not.toHaveProperty("rejected");
      });
    });
  });

  describe("and it was pending or approved", () => {
    it("should leave its review state out of the change", () => {
      for (const original of [hangout(), hangout({ approved: true })]) {
        expect(toCreateEventBody(eventToDraft(original), original, OWNER)).not.toHaveProperty("rejected");
      }
    });
  });

  describe("and it is a new event", () => {
    it("should not send a review state", () => {
      expect(toCreateEventBody(eventToDraft(hangout()), undefined, OWNER)).not.toHaveProperty("rejected");
    });
  });
});
