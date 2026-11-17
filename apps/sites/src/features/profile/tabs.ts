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

export function profileTabs(own: boolean): readonly TabEntry[] {
  return own ? OWN_TABS : MEMBER_TABS;
}

/** The tab a URL segment names for this viewer, or `null` when this profile does not show it. */
export function readProfileTab(segment: string | undefined, own: boolean): ProfileTab | null {
  const raw = (segment ?? "").trim().toLowerCase();
  if (raw === "") return "overview";
  return profileTabs(own).some((tab) => tab.id === raw) ? (raw as ProfileTab) : null;
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
