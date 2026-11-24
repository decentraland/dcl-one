import { describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

import { DEFAULT_SOCIAL_IMAGE, SOCIAL_LOGO } from "../lib/social-meta";
import { canonicalHrefs, rootMatches, tagContent } from "../lib/social-meta.test-utils";
import { meta as createMeta } from "./create._index";
import { meta as downloadMeta } from "./download";
import { meta as creatorHubMeta } from "./download_.creator-hub";
import { meta as creatorHubSuccessMeta } from "./download_.creator-hub-success";
import { meta as downloadSuccessMeta } from "./download_success";
import { meta as helpMeta } from "./help";
import { meta as pressMeta } from "./press";
import { meta as reportMeta } from "./report._index";
import { meta as reportSuccessMeta } from "./report.success";

type MetaFn = (args: never) => ReturnType<typeof downloadMeta>;
const head = (meta: MetaFn, base?: string) => meta({ matches: rootMatches(base) } as never);

describe.each([
  [
    "/download",
    downloadMeta,
    "Download Decentraland | Decentraland",
    "Download Decentraland and start exploring the metaverse — events, communities, voice chat and more.",
  ],
  [
    "/download/creator-hub",
    creatorHubMeta,
    "Download Decentraland Creator Hub | Decentraland",
    "Download the Creator Hub to start building immersive scenes and interactive experiences in Decentraland.",
  ],
  [
    "/download/creator-hub-success",
    creatorHubSuccessMeta,
    "Creator Hub installed | Decentraland",
    "Open the Creator Hub and start building for Decentraland.",
  ],
  ["/help", helpMeta, "Help & Support | Decentraland", "Find answers, troubleshoot issues, and get the most out of Decentraland."],
  ["/report", reportMeta, "Report a player | Decentraland", "Report disruptive behaviour and help keep Decentraland a safe community."],
  [
    "/report/success",
    reportSuccessMeta,
    "Report submitted | Decentraland",
    "Your report has been submitted. Thanks for helping keep Decentraland safe.",
  ],
] as const)("when rendering the %s head", (path, meta, title, description) => {
  describe("and the root loader answered the public page base", () => {
    it("should carry decentraland.org's card and the canonical path", () => {
      const tags = head(meta as MetaFn, "https://public.test");
      expect(tags[0]).toEqual({ title });
      expect(tagContent(tags, "og:title")).toEqual([title]);
      expect(tagContent(tags, "description")).toEqual([description]);
      expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(tagContent(tags, "twitter:card")).toEqual(["summary_large_image"]);
      expect(tagContent(tags, "og:type")).toEqual(["website"]);
      expect(tagContent(tags, "og:url")).toEqual([`https://public.test${path}`]);
      expect(canonicalHrefs(tags)).toEqual([`https://public.test${path}`]);
    });
  });

  describe("and there is no public page base", () => {
    it("should keep the card and drop the canonical", () => {
      const tags = head(meta as MetaFn);
      expect(tagContent(tags, "og:title")).toEqual([title]);
      expect(tagContent(tags, "og:url")).toEqual([]);
      expect(canonicalHrefs(tags)).toEqual([]);
    });
  });
});

describe("when rendering the /download_success head", () => {
  it("should carry decentraland.org's card and keep the page out of the index", () => {
    const tags = head(downloadSuccessMeta as MetaFn, "https://public.test");
    expect(tags[0]).toEqual({ title: "You're almost done — finish installing Decentraland | Decentraland" });
    expect(tagContent(tags, "description")).toEqual([
      "Open the file you just downloaded to finish installing Decentraland and start exploring.",
    ]);
    expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
    expect(tagContent(tags, "twitter:card")).toEqual(["summary_large_image"]);
    expect(tagContent(tags, "robots")).toEqual(["noindex"]);
    expect(canonicalHrefs(tags)).toEqual([]);
  });
});

describe("when rendering the /create head", () => {
  const title = "Create in Decentraland | Wearables, Emotes, Scenes & Worlds";

  describe("and the root loader answered the public page base", () => {
    it("should carry the creators-website card and the canonical path", () => {
      const tags = head(createMeta as MetaFn, "https://public.test");
      expect(tags[0]).toEqual({ title });
      expect(tagContent(tags, "og:title")).toEqual([title]);
      expect(tagContent(tags, "description")).toEqual([
        "Everything you need to create in Decentraland: build Wearables, Emotes, Scenes and Worlds, manage your collections and join the creator community.",
      ]);
      expect(tagContent(tags, "og:image")).toEqual([
        "https://images.ctfassets.net/ea2ybdmmn1kv/3fZB29mbYNJdp7sv1y2SJ2/108e55d6d3e23cd11e7d75563d81caf5/hero.webp?fm=jpg&q=85",
      ]);
      expect(tagContent(tags, "twitter:card")).toEqual(["summary_large_image"]);
      expect(tagContent(tags, "og:url")).toEqual(["https://public.test/create"]);
      expect(canonicalHrefs(tags)).toEqual(["https://public.test/create"]);
    });
  });

  describe("and there is no public page base", () => {
    it("should drop the canonical", () => {
      expect(canonicalHrefs(head(createMeta as MetaFn))).toEqual([]);
    });
  });
});

describe("when rendering the /press head", () => {
  const title = "Press Kit | Decentraland";
  const description = "Decentraland brand assets, screenshots and media resources for press and partners.";

  describe("and the root loader answered the public page base", () => {
    it("should carry the card, the logo and the canonical path, leaving the title to the page", () => {
      const tags = head(pressMeta as MetaFn, "https://public.test");
      expect(tags.some((tag) => "title" in tag)).toBe(false);
      expect(tagContent(tags, "og:title")).toEqual([title]);
      expect(tagContent(tags, "twitter:title")).toEqual([title]);
      expect(tagContent(tags, "description")).toEqual([description]);
      expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(tagContent(tags, "og:logo")).toEqual([SOCIAL_LOGO]);
      expect(tagContent(tags, "og:type")).toEqual(["website"]);
      expect(tagContent(tags, "og:url")).toEqual(["https://public.test/press"]);
      expect(canonicalHrefs(tags)).toEqual(["https://public.test/press"]);
    });
  });

  describe("and there is no public page base", () => {
    it("should keep the card and drop the canonical", () => {
      const tags = head(pressMeta as MetaFn);
      expect(tagContent(tags, "og:title")).toEqual([title]);
      expect(tagContent(tags, "og:url")).toEqual([]);
      expect(canonicalHrefs(tags)).toEqual([]);
    });
  });
});
