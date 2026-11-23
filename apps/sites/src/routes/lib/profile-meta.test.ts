import { describe, expect, it } from "vitest";

import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";

import { profileCard, type ProfileLoaderData } from "./profile-loader.server";
import { PROFILE_DESCRIPTION, profileMeta } from "./profile-route";
import { DEFAULT_SOCIAL_IMAGE } from "./social-meta";

const A = "0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda";
const matches = [{ id: "root", loaderData: { publicPageBase: "https://dcl.test" } }];

type Tags = ReturnType<typeof profileMeta>;
const content = (tags: Tags, key: string) =>
  tags.filter((t) => ("property" in t && t.property === key) || ("name" in t && t.name === key)).map((t) => (t as { content: string }).content);

const profile = (over: Partial<ProfileVM>): ProfileVM => ({ ...emptyProfile(A), ...over });
const loaded = (card: ProfileLoaderData["card"]) => ({ address: A, card }) as ProfileLoaderData;

describe("profileCard", () => {
  it("names a claimed name as it is and an unclaimed one with the address's last four characters", () => {
    expect(profileCard(profile({ name: "Alice", hasClaimedName: true })).name).toBe("Alice");
    expect(profileCard(profile({ name: "Alice", hasClaimedName: false })).name).toBe("Alice#7eda");
  });

  it("leaves the name out when the profile is only named after its address, or does not exist", () => {
    expect(profileCard(profile({ name: A })).name).toBeUndefined();
    expect(profileCard(profile({ name: "  " })).name).toBeUndefined();
    expect(profileCard(null)).toEqual({});
  });

  it("puts the description on one line and leaves a blank one out", () => {
    expect(profileCard(profile({ bio: "Builder.\n\nLikes  parties." })).description).toBe("Builder. Likes parties.");
    expect(profileCard(profile({ bio: " \n " })).description).toBeUndefined();
  });

  it("keeps the face only as a path of this site's profile image cache", () => {
    expect(profileCard(profile({ avatarUrl: "https://peer.test/profile-images/entities/bafy1/face.png" })).image).toBe(
      "/profile-images/entities/bafy1/face.png",
    );
    expect(profileCard(profile({ avatarUrl: "https://evil.test/x.png" })).image).toBeUndefined();
    expect(profileCard(profile({ avatarUrl: undefined })).image).toBeUndefined();
  });
});

describe("profileMeta", () => {
  it("a found profile is a small profile card with its name, description and face", () => {
    const tags = profileMeta({
      loaderData: loaded({ name: "Alice", description: "Builder.", image: "/profile-images/entities/bafy1/face.png" }),
      params: { address: A },
      matches,
    });
    expect(tags[0]).toEqual({ title: "Alice - Decentraland Profile" });
    expect(content(tags, "og:title")).toEqual(["Alice - Decentraland Profile"]);
    expect(content(tags, "description")).toEqual(["Builder."]);
    expect(content(tags, "og:image")).toEqual(["https://dcl.test/profile-images/entities/bafy1/face.png"]);
    expect(content(tags, "twitter:card")).toEqual(["summary"]);
    expect(content(tags, "og:type")).toEqual(["profile"]);
    expect(content(tags, "og:site_name")).toEqual(["Decentraland"]);
    expect(content(tags, "og:url")).toEqual([`https://dcl.test/profile/${A}`]);
    expect(tags).toContainEqual({ tagName: "link", rel: "canonical", href: `https://dcl.test/profile/${A}` });
  });

  it("a profile without a description or a face falls back to the generic text and the shared large image", () => {
    const tags = profileMeta({ loaderData: loaded({ name: "Alice" }), params: { address: A }, matches });
    expect(content(tags, "description")).toEqual([PROFILE_DESCRIPTION]);
    expect(content(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
    expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
  });

  it("a profile without a name of its own is the generic profile card", () => {
    const tags = profileMeta({ loaderData: loaded({}), params: { address: A }, matches });
    expect(tags[0]).toEqual({ title: "Decentraland Profile" });
    expect(content(tags, "og:type")).toEqual(["profile"]);
  });

  it("every tab, and an address in another case, is canonical to the profile itself", () => {
    const tab = profileMeta({ loaderData: loaded({ name: "Alice" }), params: { address: A, tab: "photos" } as never, matches });
    expect(content(tab, "og:url")).toEqual([`https://dcl.test/profile/${A}`]);
    // A client navigation carries no loader data: the address comes from the URL.
    const navigated = profileMeta({ loaderData: null, params: { address: A.toUpperCase().replace("0X", "0x") }, matches });
    expect(content(navigated, "og:url")).toEqual([`https://dcl.test/profile/${A}`]);
    expect(navigated[0]).toEqual({ title: "Decentraland Profile" });
  });

  it("has no canonical without a public base", () => {
    const tags = profileMeta({ loaderData: loaded({ name: "Alice", image: "/profile-images/entities/bafy1/face.png" }), params: { address: A }, matches: [] });
    expect(tags.some((t) => "rel" in t)).toBe(false);
    expect(content(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
  });

  it("an address that cannot be a wallet is a missing page; a failed read keeps the generic card", () => {
    const notFound = profileMeta({ error: errorResponse(404, "not-found"), params: { address: "nope" }, matches });
    expect(notFound[0]).toEqual({ title: "Page not found | Decentraland" });
    expect(content(notFound, "robots")).toEqual(["noindex"]);
    expect(content(notFound, "og:title")).toEqual([]);
    const unavailable = profileMeta({ error: errorResponse(503, "unavailable"), params: { address: A }, matches });
    expect(unavailable[0]).toEqual({ title: "Decentraland Profile" });
    expect(content(unavailable, "robots")).toEqual([]);
  });
});

/** The status-bearing response the loader throws, as the route's meta receives it. */
function errorResponse(status: number, reason: string) {
  return { status, statusText: "", internal: false, data: { address: "x", reason } };
}
