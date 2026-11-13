import { profileFaceUrl } from "../../auth/profile-face-url";
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

const DEFAULT_NAME_COLOR = "#FF8362";

function color3ToHex(c: { r?: number; g?: number; b?: number }): string {
  const to255 = (n: number | undefined) =>
    Math.max(0, Math.min(255, Math.round((n ?? 0) * 255)));
  const hex = (n: number) => n.toString(16).padStart(2, "0");
  return `#${hex(to255(c.r))}${hex(to255(c.g))}${hex(to255(c.b))}`;
}

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
    nameColor: DEFAULT_NAME_COLOR,
    bodyUrl: "",
    mutualCount: 0,
    bio: "",
    accountUrl: addr ? siteUrl("/shop") : "",
    info: [],
    links: [],
    equipped: [],
  };
}

export function mapProfile(avatar: Avatar, address: string): ProfileVM {
  const nameColor =
    typeof avatar.nameColor === "string"
      ? avatar.nameColor
      : avatar.nameColor
        ? color3ToHex(avatar.nameColor)
        : DEFAULT_NAME_COLOR;

  const info: ProfileInfoField[] = [];
  for (const f of INFO_FIELDS) {
    const value = avatar[f.src];
    if (typeof value === "string" && value.trim()) {
      info.push({ key: f.key, label: f.label, value, icon: f.icon });
    }
  }

  const links: ProfileLink[] = (avatar.links ?? [])
    .filter((l) => /^https?:\/\//i.test(l.url))
    .map((l) => ({ title: l.title || l.url, url: l.url }));

  const addr = normalizeAddress(address) || avatar.ethAddress || address;

  return {
    address: addr,
    name: avatar.name || addr,
    hasClaimedName: Boolean(avatar.hasClaimedName),
    nameColor,
    avatarUrl: profileFaceUrl(avatar.avatar?.snapshots?.face256),
    bodyUrl: /^https?:\/\//i.test(avatar.avatar?.snapshots?.body ?? "") ? (avatar.avatar?.snapshots?.body as string) : "",
    mutualCount: 0,
    bio: avatar.description ?? "",
    accountUrl: siteUrl("/shop"),
    info,
    links,
    equipped: [],
  };
}
