import type { LegalIconName } from "@dcl/one-ui/apps/sites/legal/LegalIcon";

import type { LegalKey } from "./types";

/**
 * The sidebar, in source order, with source's labels (English only, as source), and each
 * document's served path segment. It holds no document text, so a page that imports it does not
 * bundle the documents.
 */
export const LEGAL_SIDEBAR: readonly {
  key: LegalKey;
  slug: string;
  label: string;
  icon: LegalIconName;
}[] = [
  { key: "brand", slug: "brand", label: "Logo and Name", icon: "favorite" },
  { key: "content", slug: "content", label: "Content Policy", icon: "park" },
  { key: "ethics", slug: "ethics", label: "Code of Ethics", icon: "balance" },
  { key: "rewards", slug: "rewards-terms", label: "Rewards Program", icon: "cardGiftcard" },
  { key: "credits", slug: "credits-terms", label: "Credits", icon: "token" },
  { key: "security", slug: "security", label: "Security", icon: "shield" },
  { key: "privacy", slug: "privacy", label: "Privacy Policy", icon: "vpnKey" },
  { key: "referral", slug: "referral-terms", label: "Referral", icon: "redeem" },
  { key: "terms", slug: "terms", label: "Terms of Use", icon: "explore" },
];

const SLUGS = Object.fromEntries(
  LEGAL_SIDEBAR.map(({ key, slug }) => [key, slug]),
) as Record<LegalKey, string>;

// Paths that name a document by neither its slug nor its key.
const LEGAL_ALIASES: Readonly<Record<string, LegalKey>> = {
  "content-policy": "content",
};

export function isLegalKey(value: string): value is LegalKey {
  return Object.hasOwn(SLUGS, value);
}

/** The document a path segment names: its slug, an alias or its key. */
export function resolveLegalKey(segment: string): LegalKey | null {
  const bySlug = LEGAL_SIDEBAR.find(({ slug }) => slug === segment)?.key;
  if (bySlug) return bySlug;
  if (Object.hasOwn(LEGAL_ALIASES, segment)) return LEGAL_ALIASES[segment];
  return isLegalKey(segment) ? segment : null;
}

/** The path a document is served at, e.g. "/rewards-terms". */
export const legalPath = (key: LegalKey) => `/${SLUGS[key]}`;
