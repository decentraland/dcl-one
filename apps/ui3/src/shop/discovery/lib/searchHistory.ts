// from: shop@94584a5c:app/src/lib/searchHistory.ts
/**
 * Whether running a search should add a history entry or replace the current one.
 *
 * A new destination, another query, or a search from a page that is not the results page, is pushed,
 * so "back" returns to the previous search or to the page the reader searched from. Repeating the very
 * same destination (same path and the same query string, filters included) replaces, so pressing Enter
 * twice does not stack two identical entries. A destination is only "the same" when the whole query
 * string matches: the same `q` with a different filter is a different page.
 */
export function searchHistoryMode(current: { pathname: string; search: string }, target: string): "push" | "replace" {
  const [targetPath, targetQuery = ""] = target.split("?")
  const currentQuery = current.search.replace(/^\?/, "")
  const same = current.pathname === targetPath && normalize(currentQuery) === normalize(targetQuery)
  return same ? "replace" : "push"
}

// Sorted params, so `a=1&b=2` and `b=2&a=1` are the same destination.
function normalize(query: string): string {
  const params = new URLSearchParams(query)
  params.sort()
  return params.toString()
}
