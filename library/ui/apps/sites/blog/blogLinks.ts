// The blog's link rules, shared by the post renderer (BlogRichText) and the post markdown for
// agents (apps/sites data/lib/agent/markdown.ts). Pure and React-free, so server code can import it.

const SAFE_PROTOCOLS = new Set(['http:', 'https:', 'mailto:'])
const PROBE = 'https://blog.invalid'

export type BlogHref =
  /** A path of this site or an #anchor: followed in place. */
  | { kind: 'internal'; href: string }
  /** An http(s) or mailto URL: opened in a new tab. */
  | { kind: 'external'; href: string }

/**
 * Where a CMS hyperlink may go: #anchors and site paths stay internal, http, https and mailto are
 * external, and anything else (javascript:, data:, vbscript:, protocol-relative `//host`) is null,
 * so the link renders as its text.
 */
export function blogHref(uri: unknown): BlogHref | null {
  if (typeof uri !== 'string') return null
  const value = uri.trim()
  if (!value) return null
  if (value.startsWith('#')) return { kind: 'internal', href: value }
  if (value.startsWith('/')) {
    try {
      const parsed = new URL(value, PROBE)
      return parsed.origin === PROBE && !value.startsWith('//') && !value.startsWith('/\\')
        ? { kind: 'internal', href: value }
        : null
    } catch {
      return null
    }
  }
  try {
    return SAFE_PROTOCOLS.has(new URL(value).protocol) ? { kind: 'external', href: value } : null
  } catch {
    return null
  }
}
