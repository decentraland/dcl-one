import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

import type { ReferrerVM } from "@/data/lib/catalyst/landings/referral";

import { DEFAULT_SOCIAL_DESCRIPTION, DEFAULT_SOCIAL_IMAGE } from "../lib/social-meta";
import { canonicalHrefs, rootMatches, tagContent } from "../lib/social-meta.test-utils";
import { meta } from "./invite.($referrer)";

const ADDRESS = "0xAbCdEf0123456789aBcDeF0123456789AbCdEf01";
const FACE = "https://profile-images.decentraland.org/entities/bafkreiexample/face.png";

const referrer = (overrides: Partial<ReferrerVM> = {}): ReferrerVM => ({
  name: "Jane",
  address: ADDRESS.toLowerCase(),
  hasClaimedName: true,
  resolved: true,
  face256: FACE,
  ...overrides,
});

const BASE = "https://public.test";

const head = (referrerParam: string, value: ReferrerVM | null, base: string | undefined = BASE) =>
  meta({
    loaderData: { sid: "s1", step: "view", referrerParam, referrer: value, referral: null },
    matches: rootMatches(base),
    location: { pathname: referrerParam ? `/invite/${referrerParam}` : "/invite" },
  } as never);

describe("when rendering the /invite head", () => {
  let tags: ReturnType<typeof meta>;

  describe("and the referrer is an address whose profile resolved with a face", () => {
    beforeEach(() => {
      tags = head(ADDRESS, referrer());
    });

    it("should name the referrer without the site suffix", () => {
      expect(tags[0]).toEqual({ title: "Jane invited you to join Decentraland" });
      expect(tagContent(tags, "og:title")).toEqual(["Jane invited you to join Decentraland"]);
    });

    it("should carry the hang-out description", () => {
      expect(tagContent(tags, "description")).toEqual([DEFAULT_SOCIAL_DESCRIPTION]);
    });

    it("should show the face as a small card", () => {
      expect(tagContent(tags, "og:image")).toEqual([FACE]);
      expect(tagContent(tags, "twitter:card")).toEqual(["summary"]);
    });

    it("should point the canonical at the lowercased address", () => {
      expect(tagContent(tags, "og:url")).toEqual([`https://public.test/invite/${ADDRESS.toLowerCase()}`]);
      expect(canonicalHrefs(tags)).toEqual([`https://public.test/invite/${ADDRESS.toLowerCase()}`]);
    });
  });

  describe("and the referrer is a NAME whose profile has no face", () => {
    beforeEach(() => {
      tags = head("Jane", referrer({ face256: null }));
    });

    it("should name the referrer with the default image as a large card", () => {
      expect(tags[0]).toEqual({ title: "Jane invited you to join Decentraland" });
      expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(tagContent(tags, "twitter:card")).toEqual(["summary_large_image"]);
    });

    it("should point the canonical at the requested path", () => {
      expect(canonicalHrefs(tags)).toEqual(["https://public.test/invite/Jane"]);
    });
  });

  describe("and the referrer is unknown", () => {
    beforeEach(() => {
      tags = head("nobody", referrer({ name: null, address: null, resolved: false, face256: null }));
    });

    it("should carry the hang-out card", () => {
      expect(tags[0]).toEqual({ title: "Decentraland — Come Hang Out" });
      expect(tagContent(tags, "description")).toEqual([DEFAULT_SOCIAL_DESCRIPTION]);
      expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(tagContent(tags, "twitter:card")).toEqual(["summary_large_image"]);
    });
  });

  describe("and the loader returned no data", () => {
    it("should carry the hang-out card at the requested path", () => {
      tags = meta({ loaderData: undefined, matches: rootMatches("https://public.test"), location: { pathname: "/invite" } } as never);
      expect(tags[0]).toEqual({ title: "Decentraland — Come Hang Out" });
      expect(tagContent(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(canonicalHrefs(tags)).toEqual(["https://public.test/invite"]);
    });
  });

  describe("and there is no public page base", () => {
    it("should keep the card and drop the canonical", () => {
      tags = head(ADDRESS, referrer(), "");
      expect(tagContent(tags, "og:title")).toEqual(["Jane invited you to join Decentraland"]);
      expect(tagContent(tags, "og:url")).toEqual([]);
      expect(canonicalHrefs(tags)).toEqual([]);
    });
  });
});
