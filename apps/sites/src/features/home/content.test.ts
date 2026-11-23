import { beforeEach, describe, expect, it } from "vitest";

import { discoverToMarkdown } from "@/data/lib/agent/markdown";
import type { HomeContent } from "@/data/lib/catalyst/landings/home";
import { homeAgentContent } from "./content";

describe("homeAgentContent", () => {
  describe("when the CMS content carries outdated copy and a rituals rail", () => {
    let content: HomeContent;
    let markdown: string;

    beforeEach(() => {
      content = {
        hero: {
          kicker: "",
          title: "Outdated title",
          mobileTitle: undefined,
          subtitle: "Outdated subtitle",
          downloads: null,
          downloadsLabel: "",
          cta: {
            desktop: { label: "", href: "" },
            epic: { label: "", href: "" },
          },
          platforms: [],
        },
        rails: [
          {
            id: "rituals",
            kind: "rituals",
            title: "Rituals",
            viewAll: null,
            items: [],
          },
        ],
        comeHangOut: {
          title: "Come Hang Out",
          downloads: null,
          downloadsLabel: "",
        },
      };
      markdown = discoverToMarkdown(homeAgentContent(content));
    });

    it("should keep the current desktop title", () => {
      expect(markdown).toContain("# Close the Feed. Come Hang Out.");
    });

    it("should keep the current mobile copy", () => {
      expect(markdown).toContain("Mobile title: Hang Out From Anywhere");
      expect(markdown).toContain("Decentraland now on mobile.");
    });

    it("should list the weekly rituals with their event links", () => {
      expect(markdown).toContain("## Rituals");
      expect(markdown).toContain(
        "[Career Mondays](/events?id=0795dec0-4862-4f19-a4d5-d0f3c3c3b559)",
      );
      expect(markdown).toContain(
        "[Takeover Tuesdays](/events?id=efc2bf0f-bf0d-447a-82a2-fe7a5650ca19)",
      );
      expect(markdown).toContain(
        "[Watch Party Wednesdays](/events?id=e22333e4-4277-4b59-b99b-ba3174e1dd29)",
      );
    });
  });
});
