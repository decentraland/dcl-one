// The committed docs tree (content/docs/manifest.json, written by scripts/gen-docs-content.mts).
export type DocsSpaceIcon = "home" | "laptop" | "document" | "api" | "spark";

export interface DocsSpace {
  id: string;
  title: string;
  blurb: string;
  icon: DocsSpaceIcon;
  header?: boolean;
  /** A locale variant's folder under /docs (`player/player-pt`); English spaces have none. */
  path?: string;
  /** The English space a locale variant translates. */
  section?: string;
  language?: string;
  /** The language switcher label: EN, ES, PT, KO, ZH. */
  label?: string;
  /** Where the space comes from: the decentraland/docs mirror or a dated live capture. */
  source?: "mirror" | "capture";
  revision?: string;
  capturedAt?: string;
  sitemap?: string | null;
}

/** A space's landing URL: the Player space is the site root, a locale variant sits at its live path. */
export function docsSpaceHome(space: DocsSpace): string {
  if (space.path) return `/docs/${space.path}/`;
  return space.id === "player" ? "/docs/" : `/docs/${space.id}/`;
}

export interface DocsNavNode {
  type?: "section" | "item";
  title?: string;
  url?: string | null;
  icon?: string | null;
  children: DocsNavNode[];
}

export interface DocsPageEntry {
  url: string;
  file: string;
  title: string;
  description: string | null;
  icon: string | null;
  space: string;
  source: string | null;
  updatedAt: string | null;
  navUrl?: string;
  tokens?: number;
  /** The same page in the other languages of its section, by label. */
  alternates?: Record<string, string>;
}

export interface DocsManifest {
  generator: string;
  source: { repo: string; commit: string; committedAt: string };
  spaces: DocsSpace[];
  nav: Record<string, DocsNavNode>;
  /** The moved paths each space's .gitbook.yaml redirects, by source URL under /docs; every target is a page URL. */
  redirects?: Record<string, string>;
  pages: DocsPageEntry[];
}

export interface DocsTocEntry {
  depth: 2 | 3;
  id: string;
  text: string;
}

export interface DocsLink {
  title: string;
  url: string;
}

/** What the /docs route renders: one page, its space's navigation and the rendered body. */
export interface DocsPageView {
  url: string;
  title: string;
  description: string | null;
  mdUrl: string;
  editUrl: string | null;
  updatedAt: string | null;
  space: DocsSpace;
  /** The header's sections, each in the page's language where that variant exists. */
  spaces: DocsSpace[];
  /** The language choice: every language of the page's section, empty when it has one. */
  languages: Array<{ label: string; url: string; current: boolean }>;
  nav: DocsNavNode;
  navUrl: string;
  breadcrumb: Array<{ title: string; url: string | null }>;
  prev: DocsLink | null;
  next: DocsLink | null;
  html: string;
  toc: DocsTocEntry[];
}
