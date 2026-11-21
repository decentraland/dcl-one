import { describe, expect, it } from "vitest";

import type { CommunityLoaderData } from "../lib/community-loader.server";
import { DEFAULT_SOCIAL_IMAGE } from "../lib/social-meta";
import { meta } from "./social_.communities.$id";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const matches = [{ id: "root", loaderData: { publicPageBase: "https://dcl.test" } }];

type Tags = ReturnType<typeof meta>;
const content = (tags: Tags, key: string) =>
  (tags ?? [])
    .filter((t) => ("property" in t && t.property === key) || ("name" in t && t.name === key))
    .map((t) => (t as { content: string }).content);

const card = (over: Partial<CommunityLoaderData> | null, error?: unknown) =>
  meta({
    loaderData: over && ({ title: "Decentraland Foundation", description: "", ownerName: null, image: null, ...over } as CommunityLoaderData),
    error,
    params: { id: ID },
    matches,
  } as never);

describe("a community's link-preview card", () => {
  it("names the community, describes it and shows its image, canonical to the community", () => {
    const tags = card({ description: "Official\n\nupdates.", image: "/assets-cdn/social/communities/" + ID + "/raw-thumbnail.png" });
    expect(tags[0]).toEqual({ title: "Decentraland Foundation | Decentraland" });
    expect(content(tags, "og:title")).toEqual(["Decentraland Foundation | Decentraland"]);
    expect(content(tags, "description")).toEqual(["Official updates."]);
    expect(content(tags, "og:image")).toEqual([`https://dcl.test/assets-cdn/social/communities/${ID}/raw-thumbnail.png`]);
    expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
    expect(content(tags, "og:type")).toEqual(["website"]);
    expect(content(tags, "og:url")).toEqual([`https://dcl.test/social/communities/${ID}`]);
    expect(tags).toContainEqual({ tagName: "link", rel: "canonical", href: `https://dcl.test/social/communities/${ID}` });
  });

  it("without a description, is described by its owner, else invites to join; without an image shows the shared one", () => {
    const owned = card({ ownerName: "DCLOfficial" });
    expect(content(owned, "description")).toEqual(["A Decentraland community by DCLOfficial."]);
    expect(content(owned, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
    expect(content(card({}), "description")).toEqual(["Join this Decentraland community."]);
  });

  it("a community that does not exist is a missing page", () => {
    const tags = card(null, { status: 404, statusText: "", internal: false, data: { reason: "not-found" } });
    expect(tags[0]).toEqual({ title: "Page not found | Decentraland" });
    expect(content(tags, "robots")).toEqual(["noindex"]);
    expect(content(tags, "og:title")).toEqual([]);
  });

  it("a community that could not be read here is the generic communities card", () => {
    const tags = card(null, { status: 503, statusText: "", internal: false, data: { reason: "unavailable" } });
    expect(tags[0]).toEqual({ title: "Communities in Decentraland | Decentraland" });
    expect(content(tags, "description")).toEqual(["Connect with players, join voice chats, and discover what the Decentraland community is up to."]);
    expect(content(tags, "og:url")).toEqual([`https://dcl.test/social/communities/${ID}`]);
  });
});
