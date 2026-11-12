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
export { LEGAL_SIDEBAR, isLegalKey, legalPath, resolveLegalKey } from "./nav";

/** Every document's text. Import it from loaders and server code only: it is ~200 KB of source. */
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
