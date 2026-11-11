import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";

import { profileFaceUrl, profileSnapshotUrl } from "../../auth/profile-face-url";
import { siteUrl } from "../bases";
import { getJSON } from "../client";
import type { GetOptions } from "../client";
import { z } from "zod";
import { absentWhenNull } from "../absent";
import { NameColorSchema } from "../generated-schemas/communities";
import { ETH_ADDRESS_RE } from "../format/address";

export function normalizeAddress(addr: string | null | undefined): string {
  return (addr ?? "").trim().toLowerCase();
}

export function isEthAddress(addr: string): boolean {
  return ETH_ADDRESS_RE.test(addr.trim());
}

const Color3Schema = NameColorSchema.partial().passthrough();

const LinkSchema = z.object({
  title: z.string(),
  url: z.string(),
});

const AvatarInfoSchema = z
  .object({
    wearables: z.array(z.string()).optional(),
    snapshots: absentWhenNull(
      z.object({ face256: z.string().optional(), body: z.string().optional() }).passthrough(),
    ),
  })
  .passthrough();

const AvatarSchema = z
  .object({
    name: z.string(),
    hasClaimedName: z.boolean(),
    nameColor: absentWhenNull(z.union([Color3Schema, z.string()])),
    description: z.string(),
    links: absentWhenNull(z.array(LinkSchema)),
    country: absentWhenNull(z.string()),
    gender: absentWhenNull(z.string()),
    pronouns: absentWhenNull(z.string()),
    relationshipStatus: absentWhenNull(z.string()),
    sexualOrientation: absentWhenNull(z.string()),
    language: absentWhenNull(z.string()),
    profession: absentWhenNull(z.string()),
    birthdate: absentWhenNull(z.number()),
    realName: absentWhenNull(z.string()),
    hobbies: absentWhenNull(z.string()),
    ethAddress: z.string().optional(),
    userId: z.string().optional(),
    avatar: AvatarInfoSchema.optional(),
  })
  .passthrough();
type Avatar = z.infer<typeof AvatarSchema>;

const ProfileEnvelopeSchema = z
  .object({
    avatars: z.array(AvatarSchema),
    timestamp: z.number().optional(),
  })
  .passthrough();
type ProfileEnvelope = z.infer<typeof ProfileEnvelopeSchema>;

function parseProfileEnvelope(raw: unknown): ProfileEnvelope {
  return ProfileEnvelopeSchema.parse(raw);
}

export async function fetchProfile(
  address: string,
  opts: GetOptions = {},
): Promise<Avatar | null> {
  const raw = await getJSON<unknown>(
    `/lambdas/profile/${encodeURIComponent(normalizeAddress(address))}`,
    opts,
  );
  const env = parseProfileEnvelope(raw);
  return env.avatars[0] ?? null;
}

type ProfileInfoField = { key: string; label: string; value: string; icon: string };
type ProfileLink = { title: string; url: string };
export type ProfileVM = {
  address: string;
  name: string;
  hasClaimedName: boolean;
  nameColor: string;
  avatarUrl?: string;
  /** The full-body snapshot, an http(s) URL or empty. */
  bodyUrl: string;
  mutualCount: number;
  bio: string;
  accountUrl: string;
  info: ProfileInfoField[];
  links: ProfileLink[];
  equipped: unknown[];
};


const INFO_FIELDS: { src: keyof Avatar; key: string; label: string; icon: string }[] = [
  { src: "country", key: "country", label: "Country", icon: "globe" },
  { src: "language", key: "language", label: "Language", icon: "translate" },
  { src: "pronouns", key: "pronouns", label: "Pronouns", icon: "pronouns" },
  { src: "relationshipStatus", key: "relationship_status", label: "Relationship status", icon: "heart" },
  { src: "gender", key: "gender", label: "Gender", icon: "gender" },
  { src: "profession", key: "profession", label: "Profession", icon: "games" },
  { src: "realName", key: "real_name", label: "Real name", icon: "at" },
  { src: "hobbies", key: "favorite_hobby", label: "Favorite hobby", icon: "heart" },
];

export function emptyProfile(address: string): ProfileVM {
  const addr = normalizeAddress(address);
  return {
    address: addr,
    name: addr,
    hasClaimedName: false,
    // The same derived color the page showed before the profile loaded, from the address alone.
    nameColor: getAvatarBackgroundColor(getDisplayName(addr, false, addr)),
    bodyUrl: "",
    mutualCount: 0,
    bio: "",
    accountUrl: addr ? siteUrl("/shop") : "",
    info: [],
    links: [],
    equipped: [],
  };
}

/** The site name a link points to, for a link saved without a title. */
function linkSite(url: string): string {
  const host = new URL(url).hostname.replace(/^www\./, "");
  const first = host.split(".")[0] ?? host;
  return first.charAt(0).toUpperCase() + first.slice(1);
}

/** Only http(s) links reach the page: a profile's links are written by its owner. */
function httpUrl(url: string): string | null {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:" ? parsed.href : null;
  } catch {
    return null;
  }
}

export function mapProfile(avatar: Avatar, address: string): ProfileVM {
  const addr = normalizeAddress(address) || avatar.ethAddress || address;
  const name = avatar.name || addr;
  // Every Decentraland client colors a name from the display name (the explorer's NameColorHelper),
  // so every tab of a profile shows the same color.
  const nameColor = getAvatarBackgroundColor(getDisplayName(name, avatar.hasClaimedName, addr));

  const info: ProfileInfoField[] = [];
  for (const f of INFO_FIELDS) {
    const value = avatar[f.src];
    if (typeof value === "string" && value.trim()) {
      info.push({ key: f.key, label: f.label, value, icon: f.icon });
    }
  }

  // The one place a link's URL and shown title are decided: untitled links show their site's name.
  const links: ProfileLink[] = (avatar.links ?? []).flatMap((l) => {
    const url = httpUrl(l.url);
    if (!url) return [];
    const title = l.title.trim();
    return [{ title: title && title !== l.url ? title : linkSite(url), url }];
  });

  return {
    address: addr,
    name,
    hasClaimedName: Boolean(avatar.hasClaimedName),
    nameColor,
    avatarUrl: profileFaceUrl(avatar.avatar?.snapshots?.face256),
    bodyUrl: profileSnapshotUrl(avatar.avatar?.snapshots?.body, "body.png"),
    mutualCount: 0,
    bio: avatar.description ?? "",
    accountUrl: siteUrl("/shop"),
    info,
    links,
    equipped: [],
  };
}
