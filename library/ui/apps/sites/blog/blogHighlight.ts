/** A piece of text and whether it matches the search. */
export type BlogHighlightRun = { text: string; match: boolean }

const WORD = /[\p{L}\p{N}]+/gu

/**
 * Splits `text` into runs, marking every word that starts with one of the query's words (ignoring
 * case), as the search dropdown highlights them: "decen" marks the whole of "Decentraland".
 */
export function blogHighlightRuns(text: string, query: string): BlogHighlightRun[] {
  const terms = (query.toLocaleLowerCase().match(WORD) ?? []).filter(Boolean)
  if (!text) return []
  if (terms.length === 0) return [{ text, match: false }]

  const runs: BlogHighlightRun[] = []
  const push = (piece: string, match: boolean) => {
    if (!piece) return
    const last = runs[runs.length - 1]
    if (last && last.match === match) last.text += piece
    else runs.push({ text: piece, match })
  }

  let at = 0
  for (const found of text.matchAll(WORD)) {
    const word = found[0]
    const start = found.index
    push(text.slice(at, start), false)
    const lower = word.toLocaleLowerCase()
    push(
      word,
      terms.some((term) => lower.startsWith(term))
    )
    at = start + word.length
  }
  push(text.slice(at), false)
  return runs
}
