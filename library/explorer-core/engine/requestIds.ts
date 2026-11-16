// One sequence for the ids page requests carry and their answers echo, so two features asking the
// same kind of question (the Events page and the map both send `getEvents`) never share an id.

let last = 0

export function nextRequestId(): number {
  return ++last
}
