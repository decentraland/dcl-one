// Chat line ids must rise along the list: the chat counts a line as new when its id is above the
// last one it has seen, so a row placed above a higher id would be counted as unread and faded in
// again. A line keeps its id unless it is out of order (or repeats an earlier id, as two lines with
// the same message id do); those get a new id from `next`, which must return ids above every id
// already handed out.
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
