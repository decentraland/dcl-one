import { describe, expect, it } from "vitest";

import { readRejectionReason, rejectionReason } from "@dcl/one-ui/apps/sites";

import { rejectedNotice } from "./review-notice";

const LEAD_TWO =
  "This event has been rejected because of Invalid image and Invalid event name. Please update the information and submit again";
const PLAIN = "This event has been rejected. Please update the information and submit again";

describe("when an event's rejection is shown to its owner", () => {
  describe("and moderation stored known reasons with a note", () => {
    it("should list the reasons with 'and' and put the moderator's note on its own line", () => {
      expect(rejectedNotice("Invalid image, Invalid event name. test", "en")).toBe(`${LEAD_TWO}\ntest`);
    });

    it("should read back exactly what moderation stores", () => {
      const stored = rejectionReason(["invalid_image", "invalid_duration", "invalid_location"], "  Fix the time  ");
      expect(rejectedNotice(stored, "en")).toBe(
        "This event has been rejected because of Invalid image, Invalid duration, and Invalid location. Please update the information and submit again\nFix the time",
      );
    });
  });

  describe("and moderation stored known reasons only", () => {
    it("should omit the note line", () => {
      expect(rejectedNotice("Invalid image, Invalid event name", "en")).toBe(LEAD_TWO);
    });

    it("should join the reasons the way the reader's language does", () => {
      expect(rejectedNotice("Invalid image, Invalid event name", "es")).toContain("Invalid image e Invalid event name");
    });
  });

  describe("and moderation stored a note without reasons", () => {
    it("should show the note under the lead without a reason", () => {
      expect(rejectedNotice(rejectionReason([], "Fix the time"), "en")).toBe(`${PLAIN}\nFix the time`);
    });
  });

  describe("and the stored reason is free text written elsewhere", () => {
    it("should show text ending in a period as the note without doubling the period", () => {
      const notice = rejectedNotice("Spam.", "en");
      expect(notice).toBe(`${PLAIN}\nSpam.`);
      expect(notice).not.toContain("..");
    });

    it("should show text with several sentences as the note, not as a reason", () => {
      expect(rejectedNotice("Spam. Repeated listings", "en")).toBe(`${PLAIN}\nSpam. Repeated listings`);
    });
  });

  describe("and nothing was stored", () => {
    it("should say it was rejected without a reason or a note", () => {
      expect(rejectedNotice(null, "en")).toBe(PLAIN);
      expect(rejectedNotice("   ", "en")).toBe(PLAIN);
    });
  });
});

describe("when a stored rejection reason is read back", () => {
  it("should treat a known title followed by unknown text as the note, not as reasons", () => {
    expect(readRejectionReason("Invalid image, Something else")).toEqual({ reasons: [], note: "Invalid image, Something else" });
  });

  it("should keep a note that has its own sentences", () => {
    expect(readRejectionReason("Invalid image. First. Second.")).toEqual({ reasons: ["Invalid image"], note: "First. Second." });
  });

  it("should read a known title with a trailing period as that reason", () => {
    expect(readRejectionReason("Invalid image.")).toEqual({ reasons: ["Invalid image"], note: "" });
  });
});
