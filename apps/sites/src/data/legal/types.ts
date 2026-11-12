import type {
  LegalBlock,
  LegalSection,
  LegalTocEntry,
} from "@dcl/one-ui/apps/sites/legal/LegalPageLayout";

export type {
  LegalBlock,
  LegalInline,
  LegalLink,
  LegalSection,
  LegalTocEntry,
} from "@dcl/one-ui/apps/sites/legal/LegalPageLayout";

/** The legal documents, by the key the route, telemetry and agent markdown name them. */
export type LegalKey =
  | "brand"
  | "content"
  | "ethics"
  | "rewards"
  | "credits"
  | "security"
  | "privacy"
  | "referral"
  | "terms";

export type LegalDocument = {
  key: LegalKey;
  /** The path segment the document is served at, e.g. "rewards-terms". */
  slug: string;
  /** The document's h1. */
  title: string;
  /** The page title, as production sets it. */
  pageTitle: string;
  toc: readonly LegalTocEntry[];
  /** Blocks between the table of contents and the first section. */
  intro: readonly LegalBlock[];
  sections: readonly LegalSection[];
  /** A call to action after the body (the /security mailto link). */
  cta?: { href: string; target?: string; rel?: string; label: string };
};
