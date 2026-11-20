/** Up to 5 page numbers centred on the current page. */
export function pageWindow(current: number, pages: number): number[] {
  const size = Math.min(5, pages)
  const start = Math.min(Math.max(1, current - 2), pages - size + 1)
  return Array.from({ length: size }, (_, i) => start + i)
}

/** "1-20" style label for the items visible on `page`, given the page size and how many were returned. */
export function pageRangeLabel(page: number, limit: number, shown: number): string {
  if (shown <= 0) return '0'
  const from = (page - 1) * limit + 1
  const to = from + shown - 1
  return from === to ? String(from) : `${from}-${to}`
}
