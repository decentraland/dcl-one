// The blog's link allowlist (source RichText.helpers.ts): the one list the post renderer
// (BlogRichText) and the post markdown for agents (apps/sites data/lib/agent/blog-richtext.ts) use.
// Pure and React-free, so server-side code can import it.

const SAFE_PROTOCOLS = new Set(['http:', 'https:', 'mailto:'])

/** http, https and mailto, plus #anchors and site paths; javascript:, data:, vbscript: and the rest are inert. */
export function isSafeHyperlinkUri(uri: unknown): boolean {
  if (typeof uri !== 'string' || uri.length === 0) return false
  if (uri.startsWith('#') || uri.startsWith('/')) return true
  try {
    return SAFE_PROTOCOLS.has(new URL(uri).protocol)
  } catch {
    return false
  }
}
