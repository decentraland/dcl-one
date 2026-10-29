// The committed docs tree (content/docs/manifest.json, written by scripts/gen-docs-content.mts).
export type DocsSpaceIcon = "home" | "laptop" | "document" | "api" | "spark";

export interface DocsSpace {
  id: string;
  title: string;
  blurb: string;
  icon: DocsSpaceIcon;
  header?: boolean;
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
}

export interface DocsManifest {
  generator: string;
  source: { repo: string; commit: string; committedAt: string };
  spaces: DocsSpace[];
  nav: Record<string, DocsNavNode>;
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
  spaces: DocsSpace[];
  nav: DocsNavNode;
  navUrl: string;
  breadcrumb: Array<{ title: string; url: string | null }>;
  prev: DocsLink | null;
  next: DocsLink | null;
  html: string;
  toc: DocsTocEntry[];
}
