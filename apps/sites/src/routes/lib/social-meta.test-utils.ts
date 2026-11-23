import type { MetaDescriptor } from "react-router";

/** The `content` of every tag named `key` (by `property` or `name`), in order. */
export const tagContent = (tags: readonly MetaDescriptor[], key: string) =>
  tags.filter((t) => ("property" in t && t.property === key) || ("name" in t && t.name === key)).map((t) => (t as { content: string }).content);

/** The `href` of every canonical link. */
export const canonicalHrefs = (tags: readonly MetaDescriptor[]) =>
  tags.filter((t) => "rel" in t && t.rel === "canonical").map((t) => (t as { href: string }).href);

/** Meta args whose root match carries the public page base. */
export const rootMatches = (publicPageBase?: string) => [{ id: "root", loaderData: publicPageBase ? { publicPageBase } : {} }];
