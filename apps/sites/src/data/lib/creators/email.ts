// Stricter than builder-server's ToS schema (stray dots, dotless domains), so anything that passes here is accepted there.
const EMAIL_PATTERN =
  /^[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+)*@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*\.[a-zA-Z][a-zA-Z0-9-]{0,61}[a-zA-Z0-9]$/

export const EMAIL_MAX_LENGTH = 254

export function isValidEmail(value: string): boolean {
  const trimmed = value.trim()
  return trimmed.length <= EMAIL_MAX_LENGTH && EMAIL_PATTERN.test(trimmed)
}
