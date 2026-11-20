import { renderToString } from "react-dom/server";
import { beforeEach, describe, expect, it } from "vitest";
import { emptyDraft, type HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import HangoutAssociations from "./HangoutAssociations";

describe("when the actual association controls render", () => {
  let draft: HangoutDraft;
  let options: HangoutOptions;
  let html: string;
  beforeEach(() => {
    draft = { ...emptyDraft(), communityId: "mine", scheduleIds: ["removed"] };
    options = { profile: null, profileError: null, communities: [{ id: "mine", name: "My managed community" }], communitiesError: null, schedules: [], schedulesError: null, worlds: [], worldsError: null };
  });

  it("should offer loaded communities and real asset search, with no schedule or join link controls", () => {
    html = renderToString(<HangoutAssociations draft={draft} options={options} canChangeCommunity onEdit={() => {}} />);
    expect(html).toContain("My managed community");
    expect(html).toContain("Search featured items or collections");
    expect(html).not.toContain("Custom join link");
    expect(html).not.toContain("Schedules");
  });

  describe("and the wallet may not change the community", () => {
    it("should lock the community select", () => {
      html = renderToString(<HangoutAssociations draft={draft} options={options} canChangeCommunity={false} onEdit={() => {}} />);
      expect(html).toMatch(/<select[^>]*disabled/);
    });
  });

  describe("and the communities read failed", () => {
    beforeEach(() => {
      options = { ...options, communitiesError: "Social down", schedulesError: "Schedules down" };
      html = renderToString(<HangoutAssociations draft={draft} options={options} canChangeCommunity onEdit={() => {}} />);
    });

    it("should report it in an error notice, and nothing about schedules", () => {
      expect(html).toMatch(/<p[^>]*data-tone="error"[^>]*role="alert"[^>]*>Could not load your communities: Social down<\/p>/);
      expect(html).not.toContain("Could not load schedules");
    });
  });

  describe("and the options are still loading", () => {
    it("should keep a plain loading line, not a notice", () => {
      html = renderToString(<HangoutAssociations draft={draft} options={null} canChangeCommunity onEdit={() => {}} />);
      expect(html).toContain('<p role="status">Loading communities and schedules…</p>');
      expect(html).not.toContain("data-tone");
    });
  });
});
