import { BRAND } from "./brand";
import { CONTENT } from "./content";
import { CREDITS } from "./credits";
import { ETHICS } from "./ethics";
import { PRIVACY } from "./privacy";
import { REFERRAL } from "./referral";
import { REWARDS } from "./rewards";
import { SECURITY } from "./security";
import { TERMS } from "./terms";
import type { LegalDocument, LegalKey } from "./types";

export type * from "./types";

export const LEGAL_DOCS: Record<LegalKey, LegalDocument> = {
  brand: BRAND,
  content: CONTENT,
  ethics: ETHICS,
  rewards: REWARDS,
  credits: CREDITS,
  security: SECURITY,
  privacy: PRIVACY,
  referral: REFERRAL,
  terms: TERMS,
};

/** The sidebar, in source order, with source's labels (English only, as source). */
export const LEGAL_SIDEBAR: readonly { key: LegalKey; label: string }[] = [
  { key: "brand", label: "Logo and Name" },
  { key: "content", label: "Content Policy" },
  { key: "ethics", label: "Code of Ethics" },
  { key: "rewards", label: "Rewards Program" },
  { key: "credits", label: "Credits" },
  { key: "security", label: "Security" },
  { key: "privacy", label: "Privacy Policy" },
  { key: "referral", label: "Referral" },
  { key: "terms", label: "Terms of Use" },
];

// Paths that name a document by another segment than its key.
const LEGAL_ALIASES: Record<string, LegalKey> = {
  "rewards-terms": "rewards",
  "referral-terms": "referral",
  "content-policy": "content",
  "credits-terms": "credits",
};

export function resolveLegalKey(slug: string): LegalKey | null {
  const key = LEGAL_ALIASES[slug] ?? slug;
  return Object.prototype.hasOwnProperty.call(LEGAL_DOCS, key)
    ? (key as LegalKey)
    : null;
}

/** The path a document is served at, e.g. "/rewards-terms". */
export const legalPath = (key: LegalKey) => `/${LEGAL_DOCS[key].slug}`;
