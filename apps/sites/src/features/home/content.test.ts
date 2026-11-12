import { describe, expect, it } from "vitest";

import { discoverToMarkdown } from "@/data/lib/agent/markdown";
import type { HomeContent } from "@/data/lib/catalyst/landings/home";
import { homeAgentContent } from "./content";

describe("homeAgentContent", () => {
  it("keeps current desktop and mobile copy with the weekly rituals in agent Markdown", () => {
    const content: HomeContent = {
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

    const markdown = discoverToMarkdown(homeAgentContent(content));

    expect(markdown).toContain("# Close the Feed. Come Hang Out.");
    expect(markdown).toContain("Mobile title: Hang Out From Anywhere");
    expect(markdown).toContain("Decentraland now on mobile.");
    expect(markdown).toContain("## Rituals");
    expect(markdown).toContain(
      "[Career Mondays](/whats-on/0795dec0-4862-4f19-a4d5-d0f3c3c3b559)",
    );
    expect(markdown).toContain(
      "[Takeover Tuesdays](/whats-on/efc2bf0f-bf0d-447a-82a2-fe7a5650ca19)",
    );
    expect(markdown).toContain(
      "[Watch Party Wednesdays](/whats-on/e22333e4-4277-4b59-b99b-ba3174e1dd29)",
    );
  });
});
