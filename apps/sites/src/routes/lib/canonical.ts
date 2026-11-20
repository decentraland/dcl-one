/** The root resolves the deployment's public base for both document and client navigations. */
export function canonicalLink(base: string | undefined, path: string) {
  return base ? [{ tagName: "link" as const, rel: "canonical", href: base + path }] : [];
}
