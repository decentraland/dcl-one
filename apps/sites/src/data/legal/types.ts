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

/** A link exactly as the source renders it: `target` and `rel` only when the source sets them. */
export type LegalLink = {
  type: "link";
  href: string;
  target?: string;
  rel?: string;
  children: LegalInline[];
};

export type LegalInline =
  string | LegalLink | { type: "strong" | "em"; children: LegalInline[] };

export type LegalBlock =
  | { type: "h2" | "h3" | "h4"; id?: string; content: LegalInline[] }
  | { type: "p"; content: LegalInline[] }
  | { type: "ul"; items: LegalInline[][] };

/** A titled part of a document; `id` is its anchor. */
export type LegalSection = { id: string; blocks: LegalBlock[] };

export type LegalTocEntry = { id: string; label: string; depth?: number };

export type LegalDocument = {
  key: LegalKey;
  /** The path segment the document is served at, e.g. "rewards-terms". */
  slug: string;
  /** The document's h1. */
  title: string;
  /** The page title, as production sets it. */
  pageTitle: string;
  toc: LegalTocEntry[];
  /** Blocks between the table of contents and the first section. */
  intro: LegalBlock[];
  sections: LegalSection[];
  /** A call to action after the body (the /security mailto link). */
  cta?: { href: string; target?: string; rel?: string; label: string };
};
