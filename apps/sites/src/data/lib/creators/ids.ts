// builder-server ids are UUIDs. Anything else coming from the URL is refused before it can reach a
// signed request path (React Router decodes params, so `..%2F` would otherwise rewrite the path).
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export function isUuid(value: string | null | undefined): value is string {
  return typeof value === 'string' && UUID_PATTERN.test(value)
}

/** A route/query id as the queries take it: the UUID itself, or undefined for anything absent or malformed. */
export function parseUuidParam(value: string | null | undefined): string | undefined {
  return isUuid(value) ? value : undefined
}
