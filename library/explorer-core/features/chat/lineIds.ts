// The chat counts a line as new when its id is above the last one it has seen, so ids must rise
// along the list; a row placed above a higher id would be counted as unread and faded in again.
// `next` must return ids above every id already handed out.
export function risingIds<T extends { id: number }>(lines: T[], next: () => number): T[] {
  let last = -Infinity
  return lines.map((line) => {
    if (line.id > last) {
      last = line.id
      return line
    }
    last = next()
    return { ...line, id: last }
  })
}

// Rows can repeat a message id (one sender, one timestamp). A row without one is a DM stored
// before ids, whose store is never trimmed, so its position is stable.
export function rowKeys(messageIds: string[]): string[] {
  const seen = new Map<string, number>()
  return messageIds.map((messageId, i) => {
    if (messageId === '') return `#${i}`
    const n = seen.get(messageId) ?? 0
    seen.set(messageId, n + 1)
    return `${messageId}#${n}`
  })
}
