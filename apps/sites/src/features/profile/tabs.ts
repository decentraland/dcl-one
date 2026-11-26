import { normalizeAddress } from "@/data/lib/catalyst/social/profile";

export type ProfileTab =
  | "overview"
  | "creations"
  | "assets"
  | "communities"
  | "places"
  | "photos"
  | "referral-rewards";

type TabEntry = { id: ProfileTab; label: string };

/** Another member's profile, in the order the tab bar shows it. */
const MEMBER_TABS: readonly TabEntry[] = [
  { id: "overview", label: "Overview" },
  { id: "creations", label: "Creations" },
  { id: "communities", label: "Communities" },
  { id: "places", label: "Places" },
  { id: "photos", label: "Photos" },
];

/** The signed-in member's own profile. */
const OWN_TABS: readonly TabEntry[] = [
  { id: "overview", label: "Overview" },
  { id: "assets", label: "My Assets" },
  { id: "communities", label: "My Communities" },
  { id: "places", label: "My Places" },
  { id: "photos", label: "My Photos" },
  { id: "referral-rewards", label: "Referral Rewards" },
];

/** What decides a tab beyond whose profile it is: Referral Rewards is shown only while its rollout flag is on. */
export type ProfileTabOptions = { referral?: boolean };

export function profileTabs(own: boolean, { referral = false }: ProfileTabOptions = {}): readonly TabEntry[] {
  if (!own) return MEMBER_TABS;
  return referral ? OWN_TABS : OWN_TABS.filter((tab) => tab.id !== "referral-rewards");
}

/** Tab ids of the profile site's old links, and the tab each one is now. */
const LEGACY_TAB_IDS: Readonly<Record<string, ProfileTab>> = { referral: "referral-rewards" };

function tabSegment(segment: string | undefined): string {
  return (segment ?? "").trim().toLowerCase();
}

/** Whether a URL segment is an old tab id, so the URL moves to the current one. */
export function isLegacyTabSegment(segment: string | undefined): boolean {
  return Object.hasOwn(LEGACY_TAB_IDS, tabSegment(segment));
}

/** The tab a URL segment names for this viewer, or `null` when this profile does not show it. */
export function readProfileTab(
  segment: string | undefined,
  own: boolean,
  options: ProfileTabOptions = {},
): ProfileTab | null {
  const raw = tabSegment(segment);
  if (raw === "") return "overview";
  const id = Object.hasOwn(LEGACY_TAB_IDS, raw) ? LEGACY_TAB_IDS[raw] : raw;
  return profileTabs(own, options).some((tab) => tab.id === id) ? (id as ProfileTab) : null;
}

/** Overview lives at the profile's root; every other tab is a path segment. */
export function profileHref(address: string, tab: ProfileTab = "overview"): string {
  const base = `/profile/${encodeURIComponent(normalizeAddress(address))}`;
  return tab === "overview" ? base : `${base}/${tab}`;
}

/** Whether the signed-in wallet is the profile's own address. */
export function isOwnProfile(address: string, wallet: string | null | undefined): boolean {
  const me = normalizeAddress(wallet);
  return me !== "" && me === normalizeAddress(address);
}
